# CLAUDE.md

## Project

`c4` is a Rust library + CLI that loads config sources (folders, single
files, in-code strings and values) and deep-merges them into one value.
Edition 2024, `rust-version = 1.88` (raised from 1.85 on 2026-10-12 for
let-chains and calamine 0.36) — keep the toolchain current
(`rustup update stable`).

## Where things are written

Behavior lives in exactly one place, and this file refs it:

- **Crate rustdoc** (`src/lib.rs` `//!` + the item docs it links) — the
  user reference and the behavioral spec: sources, options, formats,
  merge rules, folder keying, table formats and type ids, spreadsheets,
  cnf, provenance and warnings, typed loading.
- **README.md** — landing page only: badges, the "why c4" positioning,
  install, the simplest `load`, the CLI, a pointer to the examples. The
  examples index lives in `examples/README.md` (owner, 2026-08-12).
- **`src/main.rs` `USAGE`** — the CLI flag reference (`c4 --help`).
- **CONTRIBUTING.md** — contributor conventions, short form.
- **This file** — decisions and their reasons, invisible constraints,
  internals. A behavior change updates the rustdoc, the tests and (for a
  decision) this file together.

Crate-doc rules (owner, 2026-07-19): docs focus on *usage* — the intro
links to the README instead of repeating the pitch; per-option detail
lives on the `Options` field docs, the lib-level section is a short
intro + one example + a ref (folder keying and spreadsheets are the
models); **every `Options` field doc carries a rust code block**. Doc
examples are real doctests that must pass under the whole feature
matrix: runnable ones use feature-agnostic pieces (custom formats via
`parse_table`, 1-tuple value sources, always-on type ids), only
filesystem-reading snippets are `no_run`.

Every option scoped to a mode or format says so in the first words of
its doc (`Tree mode only …`, `Spreadsheet formats (excel/ods) only …`) —
scope first, behavior second (owner, 2026-07-12).

## Decisions

### Positioning (2026-07-12)

In the AI era, code that reads one simple config file needs no library —
the pitch is *"ask your AI for bespoke loading code with **zero
dependencies**"* (wording note: no-library beats any library for trivial
cases, not "the generated code is short"). A config library is for the
**convention**, like node-config: deterministic override order, deep
merge, and above all the table conventions for csv/excel/ods that let
planners (game designers) own config in a spreadsheet. The escape-hatch
examples (custom formats/layouts) are part of the pitch. README carries
the statement; the crate docs do not repeat it.

### API shape

- **Options are plain data and carry everything**, sources included. No
  setter per option, no builder: `Loader` has exactly `new`, `load`,
  `trace`; new capabilities become new fields.
- **One generic `load<T>`**, no `load_as` — the annotated type picks
  dynamic `Value` or typed config. One free function, `c4::load(path)`.
- **`Value::into_typed` is the one way to type part of a loaded value**
  (owner, 2026-10-12): a single consuming method, `.clone()` to take a
  section — no borrowing twin. `.into()` cannot do it (`From` is
  infallible) and `try_into()` cannot either: a blanket `TryFrom<Value>`
  for every serde type breaks the orphan rule.
- **No dot-path getter** (`get("a.b.c")`, decided 2026-07-12): chained
  `Index` already yields `Value::Null` at any missing step. node-config's
  `get` exists only because JS lacked optional chaining.
- **Users never name `Source`.** Every source is a `From` conversion
  (forms on `Source`). There is no `sources!` macro and no `c4::value`;
  the typed override is a 1-tuple because a blanket
  `From<impl Serialize>` would overlap the path and tuple conversions.
- **Tuple arity has one meaning each** (2026-07-12): the 3-tuple's third
  element is always the layout, naming a sheet is always the 4-tuple. A
  same-day DWIM rule (3-tuple strings doubling as sheet names) was
  reverted by the owner.
- A table source whose path is not a file is `Error::Parse`, not
  `NotFound`: the common mistake is passing csv *text* as the path, and
  the message points at string sources.
- Unknown ids (format, layout, order) and a layout on a non-table format
  **panic at conversion** — config-time code (owner, 2026-07-12).
- **`Error`, `Format`, `Value` and `SourceRef` are `#[non_exhaustive]`**
  (owner, 2026-10-12, in 0.6.0): new formats, typed values and errors
  are expected, and each used to break exhaustive matches (`Format::Cnf`
  did). `Options` must not be: `..Default::default()` literals need an
  exhaustive struct.
- `Error::Unsupported` is kept although nothing constructs it today
  (owner, 2026-10-12: expected to be needed again).
- **Hot reload is not a feature** (2026-07): watching is policy-heavy and
  composes trivially; `examples/hot-reload` is the pattern.
- **Visibility:** `pub` only for the documented API; everything else is
  `pub(crate)` or private, including every `format::*::parse` and
  `src/valid.rs`.
- **Crate naming** (2026-07-05): package `c4-config` (`c4` is taken), lib
  and binary keep the name `c4`. A crate depending directly on both this
  and the real `c4` crate must rename one.

### Features

- **Capability-gating only, no option-flipping**: features add formats,
  type ids or literal syntax, never change what valid input means
  (`opt-*` features existed once and were removed: feature unification
  would let any dependency flip the app's defaults).
- **The one exception: `tree`** (owner, 2026-07-22) — a default-preset
  that flips `Options::default()`'s keying on via `cfg!` in the `Default`
  impl; the fields work without it. Accepted because keying is
  first-class plain data. Mitigations: `cli` does not enable `tree`,
  `src/main.rs` pins an explicit flat baseline, and tests build on the
  flat `common::base()` so they hold under `--all-features`.
- `default = ["jsonc"]`, deliberately one parser dependency. `cli`
  implies every format + `datetime`, `inet`, `macaddr`, `uuid`,
  `numeric` — not `tree`. No format feature at all is a deliberate
  `compile_error!`.
- `ods` was renamed from `opendocument` (2026-07-12) so feature, format id
  and extension share one name.

### Formats and merging

- `Options.formats` only maps extensions to parsers (last claimer wins);
  override order between files is decided by filename alone.
- **Custom formats live in `Options.formats`** (`FormatKind::Custom`),
  not in a separate option. A header row or renamed/reordered csv columns
  are a `CustomFormat` job (the `csv-header` example), never an option —
  which is why the table stage has no `TableOptions`.
- **Each format has a default layout** (2026-07-12): `db` for excel/ods —
  spreadsheets are grids — and `kv` for csv, which keeps settings files
  simple.
- **Typed overrides skip `None`** (owner, 2026-10-12): an unset
  `Option` field in a `(value,)` source (a CLI flag not given) must not
  erase what config files set. `src/ser.rs` tells `None` from an explicit
  null with a thread-local flag set by `serialize_none`. Null written in
  a file still overrides.
- **Strict json keeps erroring on an empty file** (owner, 2026-10-12) —
  it is strict on purpose — but the message points at `jsonc`, which
  treats an empty document as empty.
- **No leniency for hand-written csv** (owner, 2026-10-12): value cells
  are not trimmed (`port, 8080` is the string `" 8080"`). csv here is a
  spreadsheet export; people writing config by hand use jsonc or yaml.
- **dot_key applies to sheet names but never to file or folder names**
  (owner, 2026-07-27): file names carry dots for unrelated reasons
  (extensions, `app.local.yml`); sheets have no such convention.
- **Commented names** (owner, 2026-07-27): one prefix set, `#` and `_`,
  written only in `COMMENT_PREFIXES` (`src/options.rs`). `.` was dropped
  when the rule generalized from sheets — `.env` and `.hidden.yml` are
  ordinary names. The data-key filter runs on each parse result before
  any name key wraps it, so it never touches keys c4 builds from names:
  turning a name option off really loads the prefixed name. Naming a
  commented thing yourself always wins; a named commented folder still
  filters below itself.

### Table stage

- **Two stages:** a format module only lowers its input to rows
  (`Vec<Vec<String>>`); `src/format/table.rs` (always compiled, public as
  `c4::parse_table`) interprets them positionally. It has **no options**
  — the layout arrives per call. One entry point with an explicit layout
  (owner, 2026-07-12: the `Kv`-defaulting variant and `parse_table_as`
  were merged).
- **The db type row is positional** (2026-07-12): the row right after the
  keys is the type row even when entirely blank. Spreadsheet writers do
  not materialize an all-empty row, so "the second non-blank row" read
  the first record as type ids (found via mg2x's `levels` sheet). Blank
  rows skip only before the key row and between records.
- **The type row is validated eagerly** (2026-07-12): a kv-shaped sheet
  under the db default would otherwise parse silently to an empty array;
  the error hints at the `kv` layout. There is no typeless db variant —
  `DbAuto` existed briefly and was removed (owner, 2026-07-12); the
  insert-an-`auto`-row `CustomLayout` is the pattern.
- **Cell ids mirror file formats one-to-one** (owner, 2026-07-12): `json`
  needs the `json` feature and `jsonc` the `jsonc` feature — the old
  "json works under either" fallback was removed. A compiled-out id is
  unknown and fails the row; only `auto` degrades.
- `array<sep><format>` (always compiled, 2026-07-17) has no multi-line
  semantics and no trimming — a newline is an ordinary element character,
  which is the whole difference from a `csv` cell; an empty cell is `[]`.
  A `csv` cell that fails to parse errors at the **outer** cell's row.
- **Relaxed numeric fallback** (feature `numeric`, explicit numeric ids
  only, never `auto`; owner, 2026-07-22): after the literal parse fails,
  map fullwidth `０-９` to ASCII, then keep only ASCII digits, the last
  `.` and the first `-`, and parse again under the cell's type —
  `1,000.1`, `10 234.5`, `$123.12` load. Locale-naive on purpose
  (`1.000,50` is a `CustomFormat` job); radix/BigInt forms belong to the
  primary parse only.
- **dot_key array segments** are inserted by `format::insert_key`, which
  walks the tree built so far — `[]` append must see the existing array,
  so expand-then-merge cannot express it. Two limits (owner, 2026-10-12),
  because every skipped slot is allocated — one `a[30000000]` cell cost
  300 MB: one index may skip at most `MAX_ARRAY_GAP` (10 000) unset slots
  past the array's current end — counted per jump, so a sparse array may
  still reach far (`a[1]`, `a[100]`, `a[10002]`) — and no index may
  exceed `MAX_ARRAY_INDEX` (1 000 000), a last line of defense.
- Non-finite floats (`inf`, `NaN`, `1e999`) load as they parse; JSON
  output turns them into `null` like any JSON serializer (owner,
  2026-10-12: expected, not rejected).
- PostgreSQL-shaped ids (`inet`, `cidr`, `macaddr*`, `dt`/`date`/`time`,
  `uuid`) validate in `src/valid.rs`, pure std with exhaustive
  accept/reject unit tests.

### Typed loading (owner, 2026-10-12)

`load` hands a `Value` over without a serde round trip, which would
flatten typed variants (`DateTime`, `Ipv4`, …) into strings — the bug
until 2026-10-12, caught by `tests/common`'s load-vs-trace check. Owner,
2026-10-12: no `T: 'static` bound on `load` (users with generic wrappers
would have to add it), so instead `Value::deserialize` asks for a
private newtype name that `ValueDeserializer` answers by passing the
owned value through a thread-local (`src/value.rs`, `src/de.rs`); any
other deserializer sees an ordinary newtype and builds the value as
usual. **Every typed value keeps text** (owner, 2026-10-12): `Ipv4`/`Ipv6`
store their canonical text instead of `Ipv4Addr`/`Ipv6Addr`, because
`as_str() -> Option<&str>` can only lend text that exists inside the
value; `as_ip()` gives the address.

A string reaching a bool or number field parses with that table type
id's rules (`format::table::scalar`, shared with explicit cells), so ini,
env and cnf — all strings — load into typed structs; a number or bool
reaching a string field becomes its text (floats via `{:?}`, so `1.0`
keeps its `.0`). Only typed targets coerce; `deserialize_any` keeps the
value. Deserialize errors carry the key path, built in `src/de.rs` as
errors bubble out of maps and sequences. An empty string cannot become
`None` for `Option<u16>` only — serde's `deserialize_option` decides
None/Some before the inner type is known — so it stays an error whose
message says to remove the line (owner, 2026-10-12).

### Spreadsheets

- **Office lock files are not special-cased** (owner, 2026-10-12): Excel's
  `~$book.xlsx` fails the load while the workbook is open, and that is
  correct — workbooks are edited elsewhere, uploaded, then reloaded.
- Each sheet is treated like a file (owner, 2026-07-22): without
  `sheetname_as_key`, every remaining sheet merges in `order` by name —
  this replaced the old "only the sheet named `config`" rule.
- Binary formats never go through the text pipeline: the loader calls
  `format::parse_binary` when `format::is_binary`, and a string source
  naming one is an error.
- The excel reader is picked by the real file extension (`xls`, `xlsb`,
  else xlsx), so a remapped extension parses as xlsx. Cell lowering
  (serial datetimes without chrono, durations, formula results) is
  documented and unit-tested in `src/format/sheet.rs`.
- **Binary fixtures are generated**, never hand-edited:
  `cargo run --manifest-path tools/gen-sheets/Cargo.toml` (zero-dependency,
  hand-written stored zips with a fixed timestamp — byte-deterministic).
  CI regenerates and `git diff --exit-code`s.

### cnf (owner, 2026-10-12)

`.ini` is read strictly (the common subset — see "env and ini");
`.cnf` has many dialects, so its parser is lenient: every `.cnf` commonly found online must parse without error.
Where dialects conflict, **my.cnf wins** (the bulk of real files), with
no per-dialect exceptions:

- No line continuation (openssl has it; MySQL lets a value end in `\`).
- `;` and `%` (texmf.cnf) comment only at line start — `;` sits inside
  real MySQL values (`SET a=1; SET b=2`, generated passwords).
- Value handling is MySQL's order: quote-aware comment cut → trim →
  unquote only when one quote pair wraps the whole value → MySQL escapes
  only. Mid-value quotes are literal: a generated password like
  `d>$_E;"%5…"!a` must survive byte for byte. An unclosed quote is text,
  never an error.
- Variables (`$dir`, `$ENV::HOME`) stay literal, like env.
- Directives are skipped with a warning, never followed.

### env and ini (owner, 2026-10-12)

- env follows the common dotenv rules (python-dotenv, docker compose):
  a quoted value ends at its closing quote; unquoted, `#` starts a
  comment only after whitespace (`C=#fff` keeps its value); double
  quotes take `\n \r \t \" \\`.
- **ini has no formal spec.** The de facto references disagree (Windows
  `GetPrivateProfileString` strips quotes; Python configparser continues
  values on indented lines), so c4 implements only their common subset:
  `[section]` one level deep, `key=value`, `;`/`#` whole-line comments,
  no quoting, no escapes, no multi-line values.
- **CLI env/ini output must read back with c4's own parsers.** env quotes
  and escapes as above; ini writes root keys, then one `[section]` per
  top-level object, and errors on what the subset cannot hold (a newline
  in a value, `=` in a key, leading/trailing whitespace, nesting below a
  section). env errors on keys it cannot hold (`=`, whitespace, empty,
  leading `#`).
- yaml is read with `yaml_serde` (the YAML organization's maintained
  serde_yaml fork; serde_yaml is deprecated), and `<<` merge keys are
  applied.

### Warnings (owner, 2026-10-12)

- A `Warning` reports input in an **enabled** format/option that was
  accepted but not acted on (so far: skipped cnf directives). A
  compiled-out format or an option the user chose is their decision and
  gets no warning — `.yml` on a jsonc-only build is skipped silently.
- They live in `Trace.warnings`; the library never prints and `load()`
  drops them. The CLI prints them to stderr in every mode
  (`c4: warning: …`) so a CLI user needs no `--trace` to see them. No
  `log` crate.
- Parsers report a `&mut Vec<String>` of `line N: …` messages; the
  loader attaches the `SourceRef`. An `Options` warning callback is
  deferred until a second warning source exists.

### CLI

`--help` (`USAGE` in `src/main.rs`) is the flag reference; every
`Options` field is a flag with a `--no-` form, except `formats`, table
sources and layouts, which are library-only. A boolean flag given a
value (`--dot-key=false`) is an error — it used to be ignored, silently
turning the option *on* (owner, 2026-10-12). Output-format resolution:
`-f` (ids + `debug`; `jsonc` = `json`) → `-o` extension → `debug`. With
`-o` nothing goes to stdout. Output is deterministic: sorted keys; json
is two-space pretty + trailing newline; env is sorted dotted `key=value`
lines and ini root keys + `[section]`s (arrays as JSON strings; both
must read back — see "env and ini"); csv
is `key,value,format` rows with concrete ids (arrays and null as one
`json` cell; typed text values as `str` — the CLI works from the
serialized tree); toml omits null keys (toml has no null) and errors,
naming the key, on a null inside an array. Errors go to stderr with exit 1.

## Development workflow (TDD — strictly in this order)

1. Spec the behavior in the rustdoc (and a decision here).
2. Write the expected-usage tests (fixture folder + integration test,
   doc tests) — before implementation.
3. Implement until the tests pass.

## Tests

- **Fixtures:** one folder per scenario under `tests/fixtures/<case>/`:
  `config/` (the input), `expect.value.json` (what `load()` returns) and
  `expect.debug.log` (`{:#?}` of the whole `Trace`, warnings included —
  exactly `c4 --trace -f debug`; owner, 2026-10-12, replacing
  `expect.debug.json` and `expect.warnings.json`). Option variants get
  their own case folder with copied config files — never share one
  config between variant expectations. Error cases have `config/` only.
- **Helpers** (`tests/common/mod.rs`): `check(case, options)`,
  `assert_trace(case, &loader)` for cases that are not one folder, and
  `base()` — the explicit flat `Options` every case builds on instead of
  `Options::default()` (see the `tree` decision).
- **Feature gating:** fixture tests are `#[cfg]`-gated on the features
  their expectations need, since a compiled-out type id changes the
  variant in `expect.debug.log`. Watch implications: `inet` implies
  `ipv4`+`ipv6`+`cidr`, so "without ipv4" needs `not(inet)` too.
- **CLI** (`tests/cli.rs`): each `tests/cli/<case>/` holds `args.txt` +
  one `result.*` compared byte-for-byte with stdout; stderr, `-o` and
  exit-code behaviors are plain tests in the file.
- **Examples** are standalone crates (`examples/<name>/`, depending on
  `c4-config = { path = "../.." }`; root `autoexamples = false` keeps
  their dependencies out of the library's tree), each with a committed
  `output.log` — regenerate it after changing an example. Paths inside
  are relative, so logs are machine-independent.

## Commands

```sh
cargo test                            # default features
cargo test --all-features
cargo test --features cli             # binary + CLI stdout tests
cargo test --no-default-features --features yaml   # single-format build
cargo test --doc
cargo +1.88 check --all-features --all-targets     # MSRV
```

When touching feature-gated behavior, run the matrix: default,
`--all-features`, `--features cli`, every single format via
`--no-default-features` (`excel` and `ods` included), and partial
value-parser sets (`csv`, `csv,inet`, `csv,datetime`, `csv,macaddr`,
`csv,numeric`, `excel,tree,datetime`). `csv` alone exercises the
`json`/`jsonc` cell ids with both features off.

CI (`.github/workflows/main.yml`): fmt, clippy (`--all-targets
--all-features -D warnings`), MSRV, stable/beta(/nightly) tests, the
feature matrix, and an examples job diffing each `output.log`;
`RUSTFLAGS=-D warnings` globally. A `v*` tag publishes through crates.io
Trusted Publishing (no token secret); `TELEGRAM_BOT_TOKEN` /
`TELEGRAM_CHAT_ID` feed the notify job.
