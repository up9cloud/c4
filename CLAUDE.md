# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

`c4` is a Rust library + CLI that loads config sources (folders, single
files, in-code strings) and deep-merges them into one value. Formats
(jsonc, yaml, json, toml, ini, env, csv) are individually feature-gated;
`default = ["jsonc", "yaml", "numeric"]`. Edition 2024,
`rust-version = 1.85` — keep the toolchain current (`rustup update
stable`); cargo 1.81 cannot even parse the lockfile's edition-2024
dependencies.

**Division of documents:** README.md is the user-facing overview — keep
it simple and readable (tables over prose, runnable examples).
CONTRIBUTING.md carries the contributor-facing conventions in short
form. CLAUDE.md is the complete, rigorous spec: every rule below is
normative, and behavior changes update this file, the README (in
simplified form), and the tests together. TDD order is unchanged: spec
first, tests second, implementation third.

## Design principles (from discussion with the owner)

- **Options are plain data, and they carry everything** — the sources
  too (`Options.sources`, default `["config"]` folder). One struct,
  public fields, no setter per option; new capabilities become new
  fields. `Loader` has exactly `new(options)`, `load` and `trace` —
  there is no separate builder step.
- **One generic `load<T>`**, no `load_as`: `Value` implements
  `Deserialize`, so the annotated target type decides between dynamic
  and typed access. One free function: `c4::load(path)` — the path is
  required; an existing file loads as a file source, anything else as a
  folder (same detection as the CLI). Default options; anything more
  takes a `Loader`. `Value` also has direct accessors: `Index<&str>` /
  `Index<usize>` (missing → `Value::Null`, chainable), `get`,
  `get_index`, `is_null`, `as_bool/i64/u64/f64/str/array/object`
  (`as_str` also returns the text of the textual typed variants), and
  `format_id`.
- **Capability-gating features only, no option-flipping.** (`opt-*`
  features existed once and were removed: feature unification would let
  any dependency silently flip the app's defaults.) Format features add
  formats; value-parser features add table type ids; `numeric` adds
  literal syntax; `tree` adds a mode. None of them change what an
  existing valid input means.
- **`Options.formats` carries no override-order semantics.** It only
  maps extensions to parsers — the last claimer of an extension owns it
  (this is also how extensions are reassigned across formats, e.g.
  `[yaml, (jsonc, ["yml"])]` makes jsonc read `.yml`). Override order
  between files is decided by filename alone. Entries convert from
  `Format`, a string id (`"yaml"`), a `(format, [exts])` tuple, or a
  `CustomFormat`.
- **Custom formats live in `Options.formats`**, not a separate option:
  `FormatSpec.format` is `FormatKind::Builtin(Format) |
  Custom(CustomFormat)`. A `CustomFormat` is an id + extensions +
  `Arc<dyn Fn(&str, &Path, &Options) -> Result<Value>>`; string sources
  may also name one directly (`Source::string(md, "…")`) without any
  `formats` registration. The canonical example (README + doc test +
  `custom_md` fixture) is a markdown pipe-table lowered to rows for
  `parse_table`; it drops the header row in the parser so the data rows
  stay positional and `Options.table` stays untouched.
- **Extension matching is always case-insensitive**;
  `Options.case_sensitive` governs key merging only. A hidden file `.X`
  counts as having extension `X` (general rule; that is how `.env`
  matches `env`). Files whose extension no active format claims are
  ignored in merge mode.
- **Table subsystem is generic, and parsing is two-staged.** The format
  module (`src/format/csv.rs`) only lowers the file into a plain row
  table (`Vec<Vec<String>>`, i.e. `[[key, value, format], …]`); the
  generic stage (`src/format/table.rs`, always compiled, public as
  `c4::parse_table`) interprets the rows — header matching, the format
  column, dot_key, row-order merging. Spreadsheet formats will produce
  rows for that same second stage. Name shared things `Table*`
  (`TableOptions`, `TableTypes`, `TableColumns`), not `Csv*`.
- **Sources are ordered; later overrides earlier.** In-code overrides:
  `Source::string(format_or_custom, text)` and
  `Source::value(impl Serialize)` — the latter converts through the
  crate-private serializer in `src/ser.rs` (no format feature involved;
  map keys must be strings; unit variants → strings, data variants →
  `{variant: …}`; a serialization failure surfaces at load time as
  `Error::Parse` with path `value:<i>`). Null roots contribute nothing,
  from any source kind. Path arguments take `impl Into<PathBuf>`.
- **Hot reload is intentionally not a feature.** Watching is
  policy-heavy (debounce, error handling, threading model) and trivially
  composable by users; `examples/watch` shows the canonical pattern
  (`notify` + re-run `load()`). Revisit only if a compelling API shape
  appears (decided 2026-07).
- **Provenance is a typed API, and a debug/test aid only.**
  `Loader::trace()` returns `TracedValue` (Object nodes /
  `Leaf { value, source: SourceRef }`), which serializes to
  `{"$id": "Leaf", value, source, format}` JSON — used by CLI `--trace`
  and the fixtures. `$id` tags the node kind so leaves are unambiguous
  (a config object could itself have value/source/format keys). `format`
  is `Value::format_id()`, derived from the stored value — an `i8` cell
  reports `i64`, `inet` reports `ipv4`/`ipv6` for bare addresses — not
  recorded parser state. Source labels (`SourceRef::File`,
  `SourceRef::String(index)` → `"string:<i>"`) carry no config
  semantics.

## Spec details

### Merge and scanning

Source order → folder entry order (`Options.order`:
`FoldersFirstAlphabetic` default, `Alphabetic`, `ReverseAlphabetic`;
byte-wise name comparison) → deep merge (objects recurse, arrays and
scalars replace). Same-basename files are just two filenames sorted.
`flat` defaults to `true`: recursive merge ignores subfolder paths;
`flat: false` nests them as keys (folder-shaped config is tree mode's
job). Empty parse results (`Value::Null` roots) contribute nothing. A
missing source path is `Error::NotFound`; an explicit `Source::file`
with an unclaimed extension is `Error::Parse`.

### Tree mode (`Options.tree`, feature `tree`)

Folder sources only (file/string sources merge into the root as
always). Every subfolder is a key; every file is a key named after the
file with its extension stripped (a dotfile whose whole name is its
extension, like `.env`, keeps the full name). Always recursive —
`recursive`/`flat` do not apply, but `order` does: entries load in
order and key collisions (`a.yml` vs folder `a/`) deep-merge, so order
decides the winner. Extension handling: claimed → parse normally;
extension-less and `auto_files: true` (named to avoid confusion with
the table `auto` id it reuses) → the trimmed content goes through the
table `auto` detection; otherwise unclaimed → skipped when
`ignore_unknown_ext` (default), else `Error::Parse`. `tree: true`
without the feature is `Error::Unsupported`. The CLI exposes tree mode
as `--tree`.

### Table stage

Rows are `key,value[,format]` positionally; `header: true` locates
columns by the names in `TableColumns` (default `key`/`value`/`format`;
the field is `format`, not `type_`). Row numbers in `Error::Table` are
1-based; the header row, when enabled, is row 1. Blank rows are
skipped; `dot_key` expands dotted keys; rows deep-merge in order.

Type ids ("formats" in the column): `i8`–`i64`, `u8`–`u64`, `f32`/`f64`
(f32 rounds through `f32` precision), `bool`, `str`, `auto` always
exist. Aliases: `int`/`integer` → `i64`, `uint` → `u64`,
`float`/`double`/`number` → `f64`, `string`/`text` → `str`,
`boolean` → `bool`, `datetime` → `dt`. Feature-gated ids: `dt` (by
`datetime`), `date`, `time`, `ipv4`, `ipv6`, `inet`, `cidr`, `macaddr`,
`macaddr8`, `uuid` (same-named features). Without its feature an id is
an *unknown format* → `Error::Table`; only `auto` (and toml datetime
literals) degrade to strings, because they merely stop guessing.
Extended, opt-in via `TableTypes` flags: `null` (value cell ignored),
`arr:<scalar>` (splits on `table.delimiter`, default `;`; element must
be scalar), `json` (parses the cell as a JSON document; additionally
needs the `json` format feature). Failures use one shared `bad` closure:
`'{value}' is not a valid {ty}`.

PostgreSQL-definition types (shape rules in crate-private `src/valid.rs`,
each with exhaustive accept/reject unit tests):

- `inet`: host address, optional `/mask` ≤ family bits, host bits below
  the mask allowed. Bare → `Value::Ipv4`/`Ipv6`; masked → `Value::Inet`
  (text).
- `cidr`: network; mask optional (defaults to full length, so a bare
  address is a host network); host bits below the mask must be zero →
  `Value::Cidr` (text).
- `macaddr`: exactly the PostgreSQL groupings — six `:`/`-` pairs,
  `6+6` hex halves (`:`/`-`), three 4-hex groups (`.`/`-`), bare 12
  hex; one uniform separator. `macaddr8`: eight pairs, `6+10` or `8+8`
  halves, four 4-hex groups, bare 16 hex.
- `dt`: `YYYY-MM-DD`, optionally `T`/space + `hh:mm:ss[.frac]` +
  optional `Z`/`±hh:mm`. `date`: exactly `YYYY-MM-DD`. `time`: exactly
  `hh:mm:ss[.frac]`. All fixed-width.
- `uuid`: hyphenated 8-4-4-4-12 hex or bare 32 hex, case-insensitive
  (the bare form is explicit-only, like the loose MAC groupings).

`auto` order (cheap fixed-shape scans first, parser-backed after,
strict before loose): bool → `date` → `time` → `dt` → `uuid`
(hyphenated only, `valid::uuid_hyphenated`) → `macaddr`/`macaddr8`
(pair spellings only, `valid::mac*_pairs`) → `ipv4` → `ipv6` → `cidr`
(only with a `/`) → `inet` (masked only — bare addresses are already
covered because `inet` implies `ipv4`+`ipv6`) → i64 → u64 → f64 →
string. Leading-zero decimals (`007`) stay strings; bare-hex
UUID/MAC spellings never auto-convert.

Numeric literals (feature `numeric`, default on) apply wherever a
string becomes a number (auto + explicit numeric ids): `0x`/`0b`/`0o`
radix prefixes (sign allowed), ES2021 `_` separators (each `_` must sit
between two hex digits), BigInt-style trailing `n` (integer-only; a
bigint beyond u64 is not representable → auto falls back to string,
explicit ids error). Radix/bigint forms convert to floats through
integers.

### Value and format ids

`Value`: Null / Bool / Int(i64) / Uint(u64) / Float(f64) / String /
DateTime / Date / Time / Ipv4(`Ipv4Addr`) / Ipv6(`Ipv6Addr`) / Inet /
Cidr / MacAddr / MacAddr8 / Uuid / Array / Object(`BTreeMap`) — in
`src/value.rs` with the serde bridges. Textual variants keep their
validated input text; everything serializes as (canonical) strings, so
fixtures stay valid whether or not parser features are on — but
`format_id` differs, so fixture checks whose formats depend on a
feature are gated on it. A serialize→deserialize round trip through a
format without the corresponding type yields plain strings/numbers.
`format_id()`: variant-derived — `null`, `bool`, `i64`, `u64`, `f64`,
`str`, `dt`, `date`, `time`, `ipv4`, `ipv6`, `inet`, `cidr`, `macaddr`,
`macaddr8`, `uuid`, `object`, and `arr:<t>` for non-empty homogeneous
scalar arrays else `arr`.

### CLI

`c4 [sources…] [-f fmt] [-o path] [--trace] [--tree]`. Existing files are file
sources; everything else is a folder source; default source is
`./config`. Output format resolution ("auto"): `-f` (ids + `debug`;
`jsonc` = `json`) → `-o` extension → `debug` (Rust `{:#?}` of
`Value`/`TracedValue`). With `-o` nothing goes to stdout. Deterministic
output: sorted keys everywhere; json = two-space pretty + trailing
newline; env/ini = sorted dotted `key=value` lines (arrays embedded as
JSON strings, quoting only when needed); csv = `key,value,format` rows
with concrete ids (`arr:<t>` joined by `;` for homogeneous scalar
arrays, `json` otherwise; typed text values emit as `str` because the
CLI works from the serialized tree). Errors → stderr, exit 1.

## Development workflow (TDD — strictly in this order)

1. Spec the behavior here (and simplified in README.md).
2. Write the expected-usage tests (fixture folder + integration test,
   plus doc tests) against the spec — before implementation.
3. Implement until the tests pass.

## Test layout

One fixture folder per scenario under `tests/fixtures/<case>/`:
`config/` (the folder the test loads) + `expect.json` (the plain merged
result) + `expect.debug.json` (serialized trace:
`$id`/value/source/format leaves). Option variants get their own
case folder with copied config files — never share one config between
variant expectations. Error cases (`csv_bad`) have `config/` only.
`multi_sources/` is the one non-standard case. Shared helpers in
`tests/common/mod.rs`: `check(case, options)` asserts
`serde_json::to_value(trace())` against expect.debug.json and `load()`
against expect.json. Fixture-dependent tests are
`#[cfg]`-gated on the features their fixtures need so every
feature-combination build stays green (watch interactions: `inet`
implies `ipv4`+`ipv6`+`cidr`, so "without ipv4" gates need
`not(inet)` too).

- `tests/basic.rs` — simple, merge_order, precedence (filename-order
  override), jsonc, empty, missing-folder error, struct
  deserialization, typed `TracedValue` assertions, `Value`
  accessors/indexing, `format_id` derivation.
- `tests/options.rs` — recursive_off/nest/flat, order_folders_first,
  order_alphabetic, merge_order_reverse, case_insensitive, csv_dot_key,
  csv_flat_key, tree mode (tree_basic / tree_auto / tree_unknown /
  tree_order / tree_order_reverse, typed ipv4 assertion, strict
  unknown-ext error, `Unsupported` without the feature).
- `tests/formats.rs` — csv table schema (scalars + aliases, auto +
  leading-zero rule, numeric literals on/off, extended types incl. json
  gating, header + custom columns, dt/date/time/ip/inet/cidr/mac/uuid
  typed + auto-guess + unknown-type + bad-value row assertions,
  PostgreSQL mac/inet/cidr forms), strict json, ext_override,
  formats_tuple, custom_md, toml/ini/env basics, toml datetime
  following the `datetime` feature — each behind its format's
  `#[cfg(feature)]`.
- `tests/sources.rs` — single file (Path form), multi_sources
  precedence, string-source override (trace label `string:<index>`),
  `Source::value` overrides (label `value:<index>`, serde shapes,
  non-string-key error at load time).
- `examples/<name>/` — each example is a **standalone crate**: its own
  `Cargo.toml` (`c4 = { path = "../.." }`), `src/main.rs`, config files
  and a committed `output.log` with the exact expected output. Run with
  `cd examples/<name> && cargo run`; paths inside are relative to the
  example folder so the logs are machine-independent. The root crate
  sets `autoexamples = false`, so they are not cargo example targets
  and their dependencies (e.g. `notify` in `watch`) stay out of the
  library's tree. `readme-simple` / `readme-advanced` mirror the
  README; `watch` is the DIY hot-reload pattern (self-driving demo:
  scripted edits against a temp copy keep the log reproducible). After
  changing an example, regenerate its `output.log`.
- `tests/cli.rs` (`#![cfg(feature = "cli")]`) — every folder under
  `tests/cli/<case>/` holds `args.txt` + one `result.*` file compared
  byte-for-byte against the binary's stdout (via
  `env!("CARGO_BIN_EXE_c4")`); non-stdout behaviors (cwd default, `-o`
  writing, `-f` beats `-o`, unknown-extension→debug fallback, error
  exits) are regular tests in the file.
- `src/valid.rs` — unit tests for every shape validator, accept and
  reject cases.

## Commands

```sh
cargo test                            # default features
cargo test --all-features             # every format + value parser
cargo test --features cli             # builds the binary + CLI stdout tests
cargo test --features csv,datetime,inet   # typed value parsers on
cargo test --no-default-features --features yaml   # single-format build must pass
cargo test --test basic simple_mixed_formats       # one test by name
cargo test --doc                      # doc tests only
cargo build --features cli            # build the c4 binary
```

Building with no format feature at all is a deliberate `compile_error!`.

CI (`.github/workflows/main.yml`): fmt + clippy (`--all-targets
--all-features -D warnings`), MSRV 1.85 check, tests on
stable/beta(/nightly experimental), the feature matrix, and an examples
job that diffs each example's stdout against its committed
`output.log`. `RUSTFLAGS=-D warnings` globally — keep the tree
warning-free. Publishing: pushing a `v*` tag runs `cargo publish` via crates.io
Trusted Publishing (`rust-lang/crates-io-auth-action` + job-level
`id-token: write`; configured on crates.io for this repo + main.yml —
no long-lived token secret). `secrets.TELEGRAM_BOT_TOKEN` /
`TELEGRAM_CHAT_ID` feed the notify job.
When touching feature-gated behavior, run the matrix: default,
`--all-features`, `--features cli`, each single format via
`--no-default-features`, and partial value-parser combinations
(`csv,inet`, `csv,datetime`, `csv,macaddr`, …).

## Architecture notes

- `src/lib.rs` — facade only: crate docs, the `compile_error!` guard,
  module declarations, re-exports, `load(path)` and `parse_table`.
- `src/error.rs` — `Error` / `Result`.
- `src/options.rs` — the plain-data surface: `Options`, `Order`,
  `Table*`, `Format`, `FormatKind`, `FormatSpec`, `CustomFormat`
  (`parser` field is crate-private; construct via `CustomFormat::new`).
- `src/source.rs` — `Source` and its constructors.
- `src/trace.rs` — `SourceRef`, `TracedValue` and the `$id` serialization.
- `src/loader.rs` — `Loader` plus all scanning/merging internals (scan →
  parse → merge with provenance; `load()` = `trace()` minus labels, one
  code path).
- `src/value.rs` — `Value`, accessors, `format_id`, serde bridges.
- `src/de.rs` — deserialize any serde type out of an owned `Value`
  (crate-private types).
- `src/ser.rs` — serialize any serde type into a `Value`
  (`Source::value`; crate-private).
- Visibility rule: `pub` only for the documented API (the crate-root
  re-exports and their methods/fields); everything internal is
  `pub(crate)` or private, including every `format::*::parse`.
- `src/valid.rs` — crate-private shape validators (pure std, always
  compiled, unit-tested; not user API — kept private on purpose).
- `src/format/` — one module per format behind its feature, each
  pulling in only its own parser dependency: jsonc converts
  jsonc-parser's native value (no serde_json); json needs serde_json;
  yaml/toml use serde_yaml/toml; ini/env are hand-rolled (env:
  `KEY=VALUE`, `#` comments, optional `export`, quote stripping, no
  interpolation, values always strings); csv lowers to rows for
  `table.rs` (always compiled).
- `src/main.rs` — the CLI (feature `cli`, `required-features` on the
  bin target).
- Feature map: `datetime = ["date", "time"]` and gates the `dt` id
  (`dt` is a format id, not a feature); `inet = ["ipv4", "ipv6",
  "cidr"]`; `macaddr = ["macaddr8"]`; `numeric` (default) gates
  extended numeric literals; `tree` gates tree mode; `cli` implies all
  formats + `datetime`, `inet`, `macaddr`, `uuid`, `numeric`, `tree`.
- Crate naming (decided 2026-07-05): the crates.io package is
  `c4-config` (verified available; `c4` itself is taken), while the lib
  target and the binary keep the name `c4` — users `cargo add c4-config`
  and write `use c4::…`. Example crates therefore depend on
  `c4-config = { path = "../.." }`. Known edge: the real `c4` crate also
  exposes lib name `c4`, so a crate that directly depends on **both**
  gets an extern-name clash — its author resolves it with a rename key
  (`x = { package = "c4", … }`); indirect coexistence in one dependency
  graph is fine (distinct crate metadata).
