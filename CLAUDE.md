# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

`c4` is a Rust library + CLI that loads config sources (folders, single
files, in-code strings) and deep-merges them into one value. Formats
(jsonc, yaml, json, toml, ini, env, csv, excel, ods) are individually
feature-gated; `default = ["jsonc"]` (kept deliberately light — one
parser dependency). Edition 2024,
`rust-version = 1.85` — keep the toolchain current (`rustup update
stable`); cargo 1.81 cannot even parse the lockfile's edition-2024
dependencies.

**Division of documents (no duplication):** README.md is the landing
page only — badges, the "why c4" positioning statement (see the design
principle below), purpose + install, the simplest `load` usage, the
CLI, a pointer to the examples, and contributing — and it points at the
crate docs for everything else. The **examples index** (the per-example
table, how to run one, and the fixtures note) lives in
`examples/README.md`, not in the root README (owner, 2026-08-12).
The **crate-level rustdoc** (`src/lib.rs`
`//!`) is the full user reference: sources & options, Cargo features,
file formats (so named to keep them apart from the table formats),
merge rules, tree mode, table formats, custom formats, provenance.
**Readability rules for the crate docs (owner, 2026-07-19):** they focus
on *usage* — the intro carries no "why c4" pitch, it just links to the
README for the quick start and positioning; prose stays lean where code
comments in an example already explain the forms (the sources/options
example); per-mode/per-option detail lives on the `Options` field
docs, with the lib-level section reduced to a short intro + a rust
example + a ref (folder keying and the spreadsheet section are the
models: the full rules sit on `Options::filename_as_key` /
`dirname_as_key` / `sheetname_as_key` /
`Options::ignore_commented_sheetnames`/`ignore_hidden_sheets`); and **every
`Options` field doc carries a rust code block** — runnable when it
needs no files (value sources, `parse_table`), `no_run` otherwise. Its examples are real doctests and must run under the whole
feature matrix, so runnable ones use only feature-agnostic pieces
(custom formats via `parse_table`, 1-tuple value sources, always-on type
ids); only filesystem-reading `load("config")` snippets are `no_run`.
CONTRIBUTING.md carries the contributor conventions in short form.
CLAUDE.md is the complete, rigorous spec: every rule below is normative,
and a behavior change updates this file, the rustdoc/README as
appropriate, and the tests together. TDD order is unchanged: spec first,
tests second, implementation third.

## Design principles (from discussion with the owner)

- **Positioning (the "why c4" statement, decided 2026-07-12):** in the
  AI era, code that just reads a simple config file does not need a
  library at all — generated bespoke loading logic is lighter than any
  dependency. What a config *library* is still for is the **convention**:
  like node-config, whose real contribution was not convenience but a
  shared rule set (later-overrides-earlier, multi-file merge), c4's
  value is a fixed set of rules teams can point at — deterministic
  override order, deep merge, and above all the **table conventions**
  for csv/excel/ods (`key,value[,format]` rows, and the db layout
  below), which give planners (especially game designers) a
  spreadsheet-native format they can own without touching code. The
  escape-hatch examples (custom formats/layouts) are part of the pitch:
  when the convention doesn't fit, customizing is a documented few-line
  job, not a fork. README carries this statement; the crate docs do
  **not** repeat it — their intro links to the README instead (owner,
  2026-07-19: crate docs focus on usage). Wording note (owner,
  2026-07-12): the pitch is
  *"ask your AI for bespoke loading code with **zero dependencies**"* —
  the point is no-library beats any library for trivial cases, not that
  the generated code is short.
- **Options are plain data, and they carry everything** — the sources
  too (`Options.sources`, default `["config"]` folder). One struct,
  public fields, no setter per option; new capabilities become new
  fields. `Loader` has exactly `new(options)`, `load` and `trace` —
  there is no separate builder step. **Doc rule (owner, 2026-07-12):**
  every option that only applies in a particular mode/format must say
  so in the first words of its doc comment (`Tree mode only …`,
  `Merge mode only …`, `Spreadsheet formats (excel/ods) only …`) —
  scope first, behavior second.
- **One generic `load<T>`**, no `load_as`: `Value` implements
  `Deserialize`, so the annotated target type decides between dynamic
  and typed access. One free function: `c4::load(path)` — the path is
  required; an existing file loads as a file source, anything else as a
  folder (same detection as the CLI). Default options; anything more
  takes a `Loader`. `Value` also has direct accessors: `Index<&str>` /
  `Index<usize>` (missing → `Value::Null`, chainable), `get`,
  `get_index`, `is_null`, `as_bool/i64/u64/i128/u128/f64/str/array/object`
  (`as_str` also returns the text of the textual typed variants), and
  `format_id`.
- **No dot-path getter (`.get("a.b.c")`) — decided 2026-07-12.** Safe
  chained indexing (`value["a"]["b"]["c"]`, missing keys yield
  `Value::Null` at every step) already covers the need completely, so a
  string-path API would be a second way to do the same thing. The
  node-config-style `config.get('a.b.c')` exists in JS for historical
  reasons only: `config.a.b.c` used to throw on an undefined
  intermediate and optional chaining (`config?.a?.b?.c`) did not exist
  yet — Rust's `Index` impls give us the safe form natively, so there is
  nothing to work around. (`dot_key` is unrelated: it is about dotted
  keys in *input* formats, not about reading values.)
- **Capability-gating features only, no option-flipping.** (`opt-*`
  features existed once and were removed: feature unification would let
  any dependency silently flip the app's defaults.) Format features add
  formats; value-parser features add table type ids; `numeric` adds
  literal syntax. None of them change what an existing valid input means.
  **The one deliberate exception (owner, 2026-07-22): `tree`** — it is a
  pure default-preset that flips `Options::default()`'s folder/file/sheet
  keying on (`dir_depth: -1`); the keying fields themselves are always
  present and work without it. Accepted because keying is a first-class
  part of the plain-data `Options`, not a hidden capability; the
  footgun (a dependency enabling `tree` flips your default) is mitigated
  by `cli` *not* enabling it and the CLI pinning a flat baseline. See the
  "Folder keying" section for the full rationale and consequences.
- **`Options.formats` carries no override-order semantics.** It only
  maps extensions to parsers — the last claimer of an extension owns it
  (this is also how extensions are reassigned across formats, e.g.
  `[yaml, (jsonc, ["yml"])]` makes jsonc read `.yml`). Override order
  between files is decided by filename alone. Entries convert from
  `Format`, a string id (`"yaml"`), a `(format, [exts])` tuple, a
  `(format, [exts], layout)` tuple, or a `CustomFormat`. The optional
  **layout** (2026-07-12) is a `TableLayout`/`CustomLayout`/layout-id
  string (a string must be a valid layout id here — there is no sheet
  meaning in `formats`) and applies to **every file that entry claims**,
  in merge mode, tree mode and single-file path sources alike — e.g.
  `(Format::Csv, ["csv"], "db")` makes a whole folder of csv files parse
  as record grids. Table sources override it (they never consult
  `formats`); string sources stay `Kv`. A layout on a **non-table**
  format (`(Format::Yaml, ["yml"], "db")`) **panics at conversion** —
  config-time code, like unknown ids (owner request, 2026-07-12).
  Default: **the format's default layout** — `kv` for csv, **`db` for
  excel/ods** (`Format::default_layout`, decided 2026-07-12:
  spreadsheets are grids, so sheets parse as record grids unless told
  otherwise; csv keeps kv for settings files).
- **Custom formats live in `Options.formats`**, not a separate option:
  `FormatSpec.format` is `FormatKind::Builtin(Format) |
  Custom(CustomFormat)`. A `CustomFormat` is an id + extensions +
  `Arc<dyn Fn(&str, &Path, &Options) -> Result<Value>>`; a `(custom, text)`
  string source may name one directly without any
  `formats` registration. The canonical example (README + doc test +
  `custom_md` fixture) is a markdown pipe-table lowered to rows for
  `parse_table`; it drops the header row in the parser so the data rows
  stay positional. Header rows / renamed / reordered columns for csv are
  handled the same way (the `csv-header` example), not by any option.
- **Extension matching is always case-insensitive**;
  `Options.case_sensitive` governs key merging only. A hidden file `.X`
  counts as having extension `X` (general rule; that is how `.env`
  matches `env`). Files whose extension no active format claims are
  ignored in merge mode.
- **Table subsystem is generic, and parsing is two-staged.** The format
  module (`src/format/csv.rs`) only lowers the file into a plain row
  table (`Vec<Vec<String>>`, i.e. `[[key, value, format], …]`); the
  generic stage (`src/format/table.rs`, always compiled, public as
  `c4::parse_table`) interprets the rows **positionally** (col 0/1/2 =
  key/value/format), applying the type ids, dot_key and row-order
  merging. It has **no options** — no header handling, no column
  renaming; those are the caller's job in a `CustomFormat` (drop/map the
  header, emit positional rows), which is why there is no `TableOptions`.
  Spreadsheet formats will produce rows for that same second stage.
- **Sources are ordered; later overrides earlier — and users never name
  `Source`.** `Options.sources` is `Vec<Source>`, but every source is
  built by conversion, not by a constructor: a path-like value
  (`&str`/`String`/`&Path`/`PathBuf`) becomes `Source::Path` (one
  variant — folder vs single file is detected at load time, the same
  `is_dir`/`is_file`/`NotFound` rule as `c4::load`); a `(format, text)`
  tuple (`format` = `Format`, a format-id `&str`, or a `CustomFormat`)
  becomes `Source::String`; and a **1-tuple** `(value,)` where `value:
  impl Serialize` becomes a typed override (`Source::Value`). The
  `sources` list is a plain `Vec<Source>` written with `.into()` per
  element — there is **no** `sources!` macro and no `c4::value` function.
  The typed override is a 1-tuple, not a bare value, on purpose: a
  blanket `From<impl Serialize>` would overlap the path/tuple conversions
  (`&str`/`String`/2-tuples are all `Serialize`), but a single-element
  tuple `(T,)` is a distinct type that overlaps nothing. Its serializer
  lives in `src/ser.rs` (no format feature involved; map keys must be
  strings; unit variants → strings, data variants → `{variant: …}`; a
  serialization failure surfaces at load time as `Error::Parse` with path
  `value:<i>`). Null roots contribute nothing, from any source kind.
  `Source`'s variants stay public (it is the element type of the public
  field), but it has no public constructors — the `From` impls (including
  the 1-tuple) are the whole surface. **Table sources** (2026-07-12): a
  3-tuple `(format, path, layout)` and a 4-tuple
  `(format, path, sheet, layout)` convert to `Source::Table { path,
  format: Format, sheet: Option<String>, layout: TableLayout }` —
  `format` is a `Format` or a format-id `&str` (never a `CustomFormat`,
  which already controls everything itself), `path` is path-like,
  `sheet` is `Into<String>`, `layout` is a `TableLayout`, a
  `CustomLayout`, or a layout-id `&str`. Arity disambiguates against the
  `(format, text)` string source, and each arity has one meaning
  (2026-07-12; an earlier same-day DWIM rule — 3-tuple strings doubling
  as sheet names — was reverted by the owner): the **3-tuple's third
  element is always the layout** (it suits csv, where the file is the
  table; an unknown layout-id string panics at conversion, listing the
  valid ids), and **naming a sheet is always the 4-tuple**, which names
  the layout explicitly too. On csv, a 4-tuple errors at load with the
  sheet name echoed ("csv sources cannot name a sheet (got '…')"). A table source must resolve to
  a single **file** of a table format — csv, excel, ods; a path that is
  not a file is `Error::Parse` with a hint ("table sources read exactly
  one file; for in-code text use a (format, text) string source" —
  deliberately not `NotFound`, because the common mistake is passing csv
  *text* as the path); any other format is
  `Error::Parse` ("not a table format"), a compiled-out table format is
  the usual "not compiled" parse error, and `(csv, path, sheet, layout)`
  is `Error::Parse` (csv has no sheets). Naming a sheet reads **exactly
  that sheet** — the prefix/hidden ignore options do *not* apply
  (explicit wins), and a missing sheet is `Error::Parse` — and the
  parsed value merges **under the sheet name as key** (mirroring tree
  mode's sheet keying; two db-layout sheets would otherwise clobber
  each other at the root). Without a sheet, the workbook follows the
  normal config-sheet/tree selection with the given layout applied to
  whatever parses. The trace label stays `SourceRef::File(path)`.
- **Hot reload is intentionally not a feature.** Watching is
  policy-heavy (debounce, error handling, threading model) and trivially
  composable by users; `examples/hot-reload` shows the canonical pattern
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
byte-wise name comparison; converts from an id string via
`Order::from_id` / `From<&str>` — `"alphabetic"`, `"reverse_alphabetic"`
or `"reverse"`, `"folders_first_alphabetic"`/`"folders_first"`/`"default"`,
`-`/`_` interchangeable — so `Options { order: "alphabetic".into(), .. }`
and the CLI `--order` both work) → deep merge (objects recurse, arrays
and scalars replace). Same-basename files are just two filenames sorted.
Empty parse results (`Value::Null` roots) contribute nothing. A missing
source path is `Error::NotFound`; a `Source::Path` that resolves to a
single file whose extension no active format claims is `Error::Parse`
(in a folder such files are skipped).

**Scan depth (`Options.dir_depth`, default `1`, replaced
`recursive`/`flat`/`tree` on 2026-07-22).** `dir_depth` is how many
subdirectory levels below a folder source to scan: `1` = the folder's
own files plus one level (`config/*` and `config/a/*`, not
`config/a/b/*`); `0` = the folder only; `-1` = unlimited (fully
recursive). Whether the scanned folders/files become keys is the keying
options below — with all keying off (the default), every scanned file
just deep-merges into the root, folder names carrying no meaning. Files
and subfolders whose name starts with a comment prefix (`#`/`_`) are
skipped while scanning — see "Commented names and keys".

### Folder keying (`tree` is a default-preset, not a gate — owner, 2026-07-22)

The keying options — all `bool`, all default `false`, and **all always
available on any build** (no feature gate, no `Error::Unsupported`):

- **`filename_as_key`** — folder sources: each **file** becomes a key
  named after the file with its extension stripped (a dotfile whose whole
  name is its extension, like `.env`, keeps the full name), holding that
  file's parsed content, instead of merging into the parent.
- **`dirname_as_key`** — folder sources: each **subfolder** that is
  scanned (per `dir_depth`) becomes a key and everything under it merges
  below it.
- **`sheetname_as_key`** — spreadsheet formats only (applies to folder
  and single-file sources alike): a workbook parses to an object keyed by
  sheet name — see the spreadsheet section. Sheet-name keys are the one
  keying that goes through `dot_key` (owner, 2026-07-27) — see below.

**`dot_key` and the keying options (owner, 2026-07-27).** A **sheet-name**
key is inserted with the same `format::insert_key` every data key uses,
so with `dot_key: true` a sheet named `a.b` nests (`{a: {b: …}}`) and the
array suffixes apply (`items[]`); with `dot_key: false` the name is one
literal key. This holds wherever a sheet becomes a key — `sheetname_as_key`
and the sheet-name key of an explicit-sheet table source alike.
**File and folder keys stay literal**: `a.b.yml` is the key `a.b` and a
folder `x.y` is the key `x.y`, dot_key or not (file names carry dots for
reasons unrelated to nesting — extensions, `app.local.yml` — so splitting
them would be surprising; sheets have no such convention).

Single-file and string path sources ignore `filename_as_key`/
`dirname_as_key` (they merge into the root as always); `sheetname_as_key`
still applies to a single-file workbook. Keys deep-merge on collision
(`a.yml` vs a folder `a/`), so `order` decides the winner. When
`filename_as_key` is on, extension handling per file is: claimed → parse
normally; extension-less and `auto_no_ext_files: true` (named to avoid
confusion with the table `auto` id it reuses) → the trimmed content goes
through the table `auto` detection; otherwise unclaimed → skipped when
`ignore_unknown_ext` (default), else `Error::Parse`. When
`filename_as_key` is off, extension-less/unknown files are always skipped
(no key to contribute under). `auto_no_ext_files`/`ignore_unknown_ext`
are meaningful only with `filename_as_key`.

**The `tree` Cargo feature is only a default-preset.** It compiles no new
capability — the keying fields above work with or without it. What it
does is flip `Options::default()`: with `tree` on, the default becomes
`filename_as_key: true, dirname_as_key: true, sheetname_as_key: true,
dir_depth: -1` (tree-shaped loading); with it off, the flat default
(`false`/`false`/`false`/`1`). So an app that wants tree-by-default
enables the feature and keeps using `Options::default()`; anyone else
sets the fields explicitly. There is **no** `Options::tree()` constructor.
Implemented as `cfg!(feature = "tree")` in the `Default` impl. This is
the **one deliberate exception** to the "no option-flipping" principle
(see design principles) — accepted because the keying is a first-class
part of the plain-data surface, not a silently-unioned capability. Two
consequences the code handles: (1) `cli` does **not** enable `tree`, so
the binary stays flat-by-default, and `src/main.rs` additionally pins an
explicit flat baseline so even a `--features cli,tree` build's CLI is
flat unless `--tree`/`--*-as-key` is passed; (2) tests build fixture
cases from an explicit flat `common::base()` (not `Options::default()`)
so they hold under `--all-features` (which turns `tree` on). The CLI
`--tree` sets the four fields (and `--no-tree` resets them); the four are
also individually exposed as `--filename-as-key`, `--dirname-as-key`,
`--sheetname-as-key` and `--dir-depth <n>` (an integer; `--no-` forms on
the booleans).

### Commented names and keys (owner, 2026-07-27)

A name or key that starts with one of the **comment prefixes — `#` or
`_`** reads as "commented out". One shared set for every rule below
(crate-private `COMMENT_PREFIXES` in `src/options.rs`, the single place
the characters are written); `.` is deliberately **not** a prefix (it was
one while the rule only covered sheets, and was dropped when the rule
generalized on 2026-07-27 — `.env`, `.hidden.yml` and a `.y` sheet are
ordinary names). Four independent `bool` options, **all default `true`**
and **all always available on any build**, say where the rule applies:

- **`ignore_commented_filenames`** — folder scanning: a file whose name
  starts with a prefix is skipped (`config/_draft.yml`,
  `config/#old.json`), keyed or not.
- **`ignore_commented_dirnames`** — folder scanning: a subfolder whose
  name starts with a prefix is not descended into, so nothing under
  `config/_wip/` loads (and, with `dirname_as_key`, no key appears).
- **`ignore_commented_sheetnames`** — spreadsheet formats only: a sheet
  whose name starts with a prefix is skipped (this is the old
  `ignore_commented_sheets`, renamed and minus `.`).
- **`ignore_commented_data_keys`** — every **data key**: after a source
  parses (and after `dot_key` expansion), object keys starting with a
  prefix are dropped from that parse result, **recursively** — nested
  objects and objects inside arrays included, so a `#memo` column of a db
  grid vanishes from every record. It applies to every source kind
  (files, sheets, `(format, text)` strings, `(value,)` typed overrides).

Scope rules:

- The three **name** options filter **scanning/selection**, by name,
  whether or not that name becomes a key: a `#tmp` sheet is skipped in
  merge mode too (where sheet names are not keys), and `_draft.yml` is
  skipped with `filename_as_key` off.
- **Naming a thing yourself always wins** (explicit beats the filter,
  exactly as with `ignore_hidden_sheets`) — three cases, all tested:
  1. a **file** path source (or `c4::load("config/_draft.yml")`) loads
     that file with `ignore_commented_filenames: true`;
  2. a **folder** path source (`c4::load("config/_wip")`) scans that
     folder with `ignore_commented_dirnames: true` — but **only that
     folder is exempt**: the scan below it filters as usual, so
     `config/_wip/#deep/` is still skipped and its files still follow
     `ignore_commented_filenames`;
  3. a **table source naming a sheet** (`(format, path, sheet, layout)`)
     reads that sheet with `ignore_commented_sheetnames: true` and keys
     it by the sheet name.
- **`ignore_commented_data_keys` never touches the structural keys** c4
  builds from names (`filename_as_key`/`dirname_as_key`/`sheetname_as_key`,
  and the sheet-name key of an explicit-sheet table source) — that
  data/name split is exactly why the option says **data keys** (owner,
  2026-07-27): those keys are governed by the three name options alone,
  so turning one of them off really does make the prefixed name loadable
  as a key. Implementation consequence: the filter runs at the **data**
  boundary — on the parse result of one file/sheet/string/value, *before*
  any name key wraps it — not on the merged tree.
- A dropped key takes its whole subtree with it; an object left empty
  stays an empty object (which merges as nothing).
- `c4::parse_table` never filters by itself — it is a building block, and
  the filter runs on what a **source** finally parsed to, a
  `CustomFormat`/`CustomLayout` result included.

### Table stage

The stage interprets a plain row table under a **`TableLayout`**
(2026-07-12) — public enum in `src/options.rs`, id-convertible like
`Order` (`TableLayout::from_id` / `From<&str>`, panics on unknown id):

- **`Kv`** (id `kv`, alias `kvf`) — the default for csv (each format
  has a default layout: `Format::default_layout` = kv for csv, db for
  excel/ods). Rows are `key,value[,format]` **positionally** — col 0 the
  key, col 1 the value, col 2 the optional type id. Blank rows are
  skipped; `dot_key` expands dotted keys; rows deep-merge in order.
- **`Db`** (id `db`) — a database-style grid: the first non-blank row
  holds the **keys**, and the row **immediately after the key row** is
  **always** the **type ids** (a cell may be empty = `auto`; there is
  deliberately no typeless variant — one less API, and the convention
  stays uniform: planners always see the type row), every following
  row is one record. The type row is **positional, not "next
  non-blank"** (2026-07-12): an all-blank row right after the keys is
  a type row of all `auto`s, not a blank row to skip. Spreadsheet
  writers usually don't materialize an all-empty row, so under the
  earlier "second non-blank row" reading a legally all-`auto` type row
  vanished and the first record was consumed as the type ids (an
  all-numeric record then failed eager validation with the misleading
  kv hint — found via mg2x's `levels` sheet). Blank rows are skipped
  only **before the key row** and **between records**. The type row is **validated eagerly** (2026-07-12): every
  non-empty cell must be a known (compiled-in) type id, else
  `Error::Table` at the type row's real row number with a hint
  ("unknown or disabled type id '…' in the type row — if this sheet is
  key,value rows, use the \"kv\" layout (spreadsheets default to db)").
  This makes a kv-shaped sheet under the db default fail loudly — a
  two-row kv sheet would otherwise silently parse to an empty array —
  and catches typo'd/disabled ids even in columns that never carry
  data. The result is a `Value::Array` of one object per record:
  `[[a,b],[i8,ipv4],[4,1.1.1.1],[5,2.2.2.2]]` →
  `[{a:4,b:1.1.1.1},{a:5,b:2.2.2.2}]`. Empty cells (and cells beyond
  the key row's width, and columns whose key cell is empty) are
  **omitted** from that record's object — sparse tables give sparse
  objects, which plays well with `#[serde(default)]`/`Option`.
  `dot_key` applies to the keys (a `stats.atk` column nests per
  record). A grid with no non-blank rows is `Value::Null` (contributes
  nothing); a key row with no records is an empty array. Two db
  sources do **not** concatenate — the arrays replace each other like
  any array (merge rule 2). A grid
  **without** a type row is the canonical `CustomLayout` pattern:
  insert a row of `auto` cells after the header and delegate to the
  `Db` layout (shown in the `xlsx-sheets` example and the
  `csv_db_no_types` fixture) — `DbAuto` existed briefly and was
  removed on owner request (2026-07-12).
- **`Custom(CustomLayout)`** — the rows escape hatch:
  `CustomLayout::new(id, |rows, path, options| …)` receives the lowered
  `Vec<Vec<String>>` and returns any `Value`. This is how one sheet of
  a workbook gets fully custom treatment (binary formats can't use
  `CustomFormat`, which parses text).

**`dot_key` array segments (2026-07-19).** With `dot_key: true`, a key
**segment** (between dots) may carry one or more trailing array
suffixes — `[]` (append) or `[<int>]` (index), chained for nested
arrays (`a[1][2]`, `g[][]`, `h[0][]` — each suffix is one nesting
level). The shape rules: the base name before the first `[` must be
non-empty, every group must be exactly `[]` or `[<digits>]` with the
digits parsing as `usize` (leading zeros accepted), and the groups must
run back-to-back to the segment's end. Any violation anywhere makes the
**whole segment** a literal key (`[]`/`[3]`/`[0][1]` alone — no base,
`a[x]`, `a[-1]`, `a[]b`, `a[1]x[2]`, `a[[1]]`). The suffixes apply
everywhere
`dot_key` applies — env keys, kv layout keys, db key-row columns — also
on keys without any dot (`ports[]`), and chains through the path
(`a[0].b[].c`); `dot_key: false` keeps everything literal. Semantics,
per parse (one file/sheet/string source — across sources arrays still
replace, merge rule 2):

- `name[]` **appends** one new element per occurrence: each kv row / env
  line with `a[].b` pushes a new element; in db each `a[].…` **column**
  pushes per record (`[[a[].b, a[].c], [auto, auto], [1, 2]]` →
  `[{a: [{b:1}, {c:2}]}]`).
- `name[<i>]` addresses element `i`, growing the array with `Null`s as
  needed; the same index deep-merges into the same element
  (`a[0].b` + `a[0].c` → one object), out-of-order indexes land sorted
  by index (kv rows `a[0].b`, `a[2].c`, `a[1].c` →
  `{a: [{b:…}, {c:…}, {c:…}]}`). **Skipped indexes leave `Null` gaps**:
  `a[1]=1` + `a[4]=4` → `{a: [null, 1, null, null, 4]}` (length 5) —
  deserialize such an array as `Vec<Option<T>>`.
- Chained suffixes nest: `m[1][2]=9` → `{m: [null, [null, null, 9]]}`;
  `g[][]` appends a new inner array per occurrence (`[[1], [2]]`),
  `h[0][]` appends inside element 0 (`[[1, 2]]`).
- Kind collisions follow merge rule 2 — the later row wins: an array
  suffix over a non-array replaces it with an array; a plain segment
  descending into an array replaces it with an object.

Implementation note: `expand_key` was replaced by
`insert_key(root, key, value, dot_key)` (`src/format/mod.rs`), which
walks the existing tree — `[]` append has to see the array built so far,
so expand-then-merge cannot express it. Its sibling
`strip_commented_data_keys(value, options)` (same module) is the
`ignore_commented_data_keys` pass, called on every data-parse result —
`format::parse` (all text formats, so file *and* string sources), the csv
branch of `parse_table_file`, `sheet::parse_sheet` (per sheet, before any
sheet-name key) and, in the loader, custom-format results and
`Source::Value`.

Cell typing is shared across layouts: the type ids below, the `bad`
message shape, and `Error::Table` row numbers (1-based, and real
spreadsheet rows for sheets) all behave identically. The stage still
has **no Options fields**: the layout arrives per call, chosen per
source via table-source tuples. **One** public entry point:
`parse_table(rows, &TableLayout, path, options)` — the layout is always
explicit (the `Kv`-shorthand `parse_table(rows, path, options)` and the
separate `parse_table_as` were merged on owner request, 2026-07-12: one
function, no hidden default). Header
rows/renamed/reordered columns for the **kv** layout remain a
`CustomFormat` job (the `csv-header` example and the `custom_md`
markdown format both drop/map the header and stay positional).

Type ids ("formats" in the column): `i8`–`i64`, `u8`–`u64`, `i128`,
`u128`, `f32`/`f64` (f32 rounds through `f32` precision), `bool`, `str`,
`auto` always exist. Aliases: `int`/`integer` → `i64`, `uint` → `u64`,
`float`/`double`/`number` → `f64`, `string`/`text` → `str`,
`boolean` → `bool`, `datetime` → `dt`. `bool` parses two ways: `auto`
accepts only the words `true`/`false` (case-insensitive), while an
explicit `bool` cell also accepts (case-insensitive) `t`/`f`,
`yes`/`no`, `y`/`n`, `on`/`off`, `1`/`0`. 128-bit ids are
**explicit-only** — `auto` never widens past `i64`/`u64`/`f64`.
Feature-gated ids: `dt` (by `datetime`), `date`, `time`, `ipv4`, `ipv6`,
`inet`, `cidr`, `macaddr`, `macaddr8`, `uuid` (same-named features), and
`json` / `jsonc` — a whole document as the cell (array/object/null/
scalar). The cell format ids mirror the file formats **one-to-one**:
`json` parses with the strict json parser (feature `json`), `jsonc` with
the jsonc parser (feature `jsonc`) — the old "`json` works under either
feature, parsed by whichever is present" fallback was removed on owner
request (2026-07-12; a format id names its parser, exactly like file
extensions do). Without its feature an
id is an *unknown format* → `Error::Table`; only `auto` (and toml
datetime literals) degrade to strings, because they merely stop
guessing. Failures use one shared `bad` closure: `'{value}' is not a
valid {ty}`.

**List cell type ids (2026-07-17): `array` and `csv`.** Two explicit-only
ids (like `json`/`jsonc`, `auto` never produces them) that expand one
cell into a list; each carries an optional **single-character
separator** (default `,`):

- **`array<sep><format>`** — a **native** split (no parser dependency, so
  **always compiled**, like `str`). Splits the cell into a **flat** list
  by the separator and runs **each** piece through the shared cell
  converter under the per-element `format` (so `array|` on `1|2|3` →
  `[1,2,3]`, `array` on `a,b` → `["a","b"]`). Grammar
  `array<sep><format>`, both parts optional and **positional** (exactly
  like `csv<sep><layout>`): after the literal `array` the first character
  (if any) is the separator (default `,`) and the rest (if any) is a
  per-element type id applied to **every** element (default empty =
  `auto`). So `array|u8` on `1|2|3` → `[1,2,3]` typed `u8`, `array,str`
  keeps the pieces as strings; because the separator is positional,
  naming a format means writing the separator too (`array,i8`). The
  `format` reuses the whole cell-converter (any id `convert` handles,
  including feature-gated ones and even a nested list id), is validated
  recursively by `known_type_id` (so `array,i8` is caught in a db type
  row), and an element that does not fit its format fails the row
  (`'{value}' is not a valid {ty}` at the cell's row). One `format`
  types **every** element the same; a list whose elements need
  **different** formats is a **`csv`** cell (`a,1,i8` / `b,2,i16` rows).
  There are **no multi-line semantics** — a newline is an ordinary
  character inside an element (that is the whole difference from `csv`);
  no trimming (pieces reach the converter verbatim); an empty cell →
  `[]` (empty array).
- **`csv<sep><layout>`** — gated on the **`csv`** feature (it reuses the
  csv parser; without the feature `csv` is an unknown id). Parses the
  **whole cell as a CSV document** (csv crate, `delimiter = sep`,
  headerless, flexible) into rows, then runs the standard table stage
  under the named `TableLayout` — so the cell's shape is **whatever that
  layout produces** (`csv,kv` → an object, `csv,db` → an array of
  objects); there is no raw `[[…]]` form. Grammar `csv<sep><layout>`,
  both parts optional and **positional**: after the literal `csv` the
  first character (if any) is the separator, the rest (if any) is the
  layout id; defaults are separator `,` and the csv format default layout
  (`kv`). So `csv` = `csv,kv`, `csv;` = sep `;` layout `kv`, `csv,db` =
  sep `,` layout `db`, `csv;db` = sep `;` layout `db`. Because the
  separator is positional, naming a layout means writing the separator
  too — `csvdb` reads as separator `d` + layout `b` (unknown id). The
  separator must be a single **ASCII** byte (the csv delimiter is one
  byte); a non-ASCII separator, an unknown layout id, or a compiled-out
  layout all make the id unknown. Only built-in layout ids are nameable
  (`kv`/`db`) — a `CustomLayout` has no id form here. Nested tables
  recurse (a `csv,db` cell whose content has a `csv` column parses the
  inner cell too). A cell whose content fails to parse errors at the
  **outer** cell's row (`… row N: invalid csv cell: …`).

Both ids are recognised by `known_type_id` (honoring the `csv` feature,
the ASCII-separator rule and the layout id), so an `array`/`csv` id in a
`db` type row is validated eagerly like any other, and error rows are the
usual real 1-based (spreadsheet) row numbers.

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
strict before loose): bool (case-insensitive `true`/`false`) → `date` → `time` → `dt` → `uuid`
(hyphenated only, `valid::uuid_hyphenated`) → `macaddr`/`macaddr8`
(pair spellings only, `valid::mac*_pairs`) → `ipv4` → `ipv6` → `cidr`
(only with a `/`) → `inet` (masked only — bare addresses are already
covered because `inet` implies `ipv4`+`ipv6`) → i64 → u64 → f64 →
string. Leading-zero decimals (`007`) stay strings; bare-hex
UUID/MAC spellings never auto-convert.

Numeric literals (feature `numeric`, **off by default** — a table-format
capability) apply wherever a **table cell** becomes a number (auto +
explicit numeric ids, including `i128`/`u128`): `0x`/`0b`/`0o` radix
prefixes (sign allowed), ES2021 `_` separators (each `_` must sit
between two hex digits), BigInt-style trailing `n` (integer-only; a
bigint beyond the target type is not representable → auto falls back to
string, explicit ids error). Radix/bigint forms convert to floats
through integers.

**Relaxed numeric fallback (feature `numeric`, explicit numeric ids only
— never `auto`; owner, 2026-07-22).** For an **explicit** numeric cell
(`i8`–`i64`, `u8`–`u64`, `i128`, `u128`, `f32`/`f64`) that the standard
literal parse above cannot turn into a number, one last recovery pass
runs so human/spreadsheet notation still loads: thousands separators
(`1,000.1`, and spaces `10 234 345.111` → `10234345.111`), a leading
currency symbol (`$123.12`; also `€`/`£`/`¥` — any non-digit is dropped),
and fullwidth digits (`０-９`). The steps are fixed: **(a)** map fullwidth
digits `０-９` to `0-9`; **(b)** keep only ASCII digits, the **last** `.`
(the decimal point) and the **first** `-` (the sign), dropping every
other character (commas, spaces, currency, stray dots/minuses); then
parse the result under the cell's own type (so a fraction still fails an
integer id, and an out-of-range value still errors). `auto` never runs
this pass — `1,000.1`/`$5` stay strings under auto — and it is compiled
out entirely without the `numeric` feature. It runs only after the
literal forms above have failed, and it does **not** interpret radix or
BigInt markers (its digit-only filter would strip an `0x`/`n`), so those
forms are handled solely by the primary parse. (The pass is
intentionally locale-naive: the last dot is always the decimal, so
European `1.000,50` is not special-cased — reach for a `CustomFormat`
when a locale rule is needed.)

### Spreadsheet formats (features `excel`, `ods`)

Both are table-shaped **binary** formats read with `calamine` (pinned
`0.35` — `0.36` needs rustc 1.88, past our MSRV) and lowered to rows
for the standard table stage. Their **default layout is `db`**
(`Format::default_layout`; csv defaults to `kv`) — a sheet is a record
grid unless a table source or a `formats` entry says otherwise; nothing
else about type ids, `dot_key` or merging is spreadsheet-specific.

- `excel` → `Format::Excel`, id `excel`, default extensions `xlsx`,
  `xlsm`, `xlsb`, `xls`. The reader is picked by the actual file
  extension (`xls` → Xls, `xlsb` → Xlsb, anything else → Xlsx), so a
  remapped extension parses as xlsx. `ods` → `Format::Ods`, id `ods`,
  default extension `ods` (the feature was renamed from `opendocument`
  to `ods` on 2026-07-12 — feature, format id and extension now share
  one name). Both are format features (they satisfy the no-format
  `compile_error!` guard) and both are implied by `cli`.
- **File-only.** A `(excel|ods, text)` string source is `Error::Parse`
  ("binary format — file sources only"); the loader parses these formats
  from the path (no `read_to_string`), so binary bytes never hit the
  text pipeline. Without the feature the extension is simply unclaimed
  (and a manually added `Format::Excel` spec errors "not compiled", like
  any other format).
- **Sheet selection.** Non-worksheet sheets (chart/dialog/macro/VBA —
  an Excel-only concept) are always skipped. Then two independent
  `Options` fields, both default `true`:
  - `ignore_hidden_sheets` — skip sheets marked hidden in the workbook
    (xlsx `hidden`/`veryHidden`; ods `table:display="false"`).
  - `ignore_commented_sheetnames` — skip sheets whose name starts with a
    comment prefix (`#` or `_`; **not** `.`) — see "Commented names and
    keys". Each remaining sheet's parsed content then goes through
    `ignore_commented_data_keys` like any other data.
- **`sheetname_as_key: false` (default) — each sheet is a file (owner,
  2026-07-22):** every remaining sheet parses and they all **deep-merge**
  into one value, in `order` applied to the sheet names (sheets have no
  folder/file distinction, so `FoldersFirstAlphabetic` sorts them
  alphabetically), later sheets overriding earlier ones — exactly how
  several files in a folder merge. (This replaced the old "only the sheet
  named `config`" rule.) Two db sheets are two arrays, so the later one
  replaces (merge rule 2); object-producing sheets deep-merge key by key.
  `case_sensitive` folds sheet keys the same way the traced file merge
  does (helper `deep_merge` in `src/format/sheet.rs`, mirroring the
  loader). No sheets left after filtering → Null (contributes nothing).
- **`sheetname_as_key: true`:** every remaining sheet parses and the
  workbook's value is an object keyed by sheet name — a `b.xlsx` with
  sheets `c`, `d` parses to `{c: …, d: …}` (and under
  `filename_as_key`/`dirname_as_key` that object nests below the file and
  folder keys, so `a/b.xlsx` gives `{a: {b: {c: …, d: …}}}`). This is the
  parse result, so it applies wherever the workbook appears (a
  single-file source with `sheetname_as_key: true` merges the sheet-keyed
  object into the root); sheet keys deep-merge like any keys and
  `case_sensitive` applies. Sheet names go in through `insert_key`, so
  `dot_key` expands them (`a.b` → `{a: {b: …}}`, `items[]` appends) —
  unlike file/folder keys, which stay literal (see "Folder keying"). No
  sheets left after filtering → Null (contributes nothing).
- **Explicit sheet (table sources):** a `(format, path, sheet, layout)`
  source reads exactly that sheet with that layout, skips the ignore
  filters, errors when the sheet is missing, and merges under the sheet
  name — see the table-sources bullet in the design principles. Several
  sources may point at the same workbook to give each sheet its own
  layout (the `xlsx-sheets` example).
- **Grid → rows, anchored at A1.** Leading empty rows/columns are padded
  in, so column A is always the key, B the value, C the type id — and
  `Error::Table` row numbers are real spreadsheet row numbers (blank
  rows are skipped by the table stage as usual).
- **Cells lower to text** before the table stage: strings as-is; numbers
  via Rust `Display` (`8080`, `1.5`); booleans `true`/`false`; empty
  cells `""`; ods ISO date/duration text as-is. Excel serial datetimes
  convert without chrono (via calamine's `to_ymd_hms_milli`):
  fraction-only serials (< 1.0, time-formatted cells) → `hh:mm:ss[.mmm]`,
  whole-day serials at midnight → `YYYY-MM-DD`, otherwise
  `YYYY-MM-DD hh:mm:ss[.mmm]` (space separator; `.mmm` only when
  non-zero); duration-formatted cells → `hh:mm:ss[.mmm]` of the total.
  Formula cells contribute their cached result; error cells (`#DIV/0!`
  …) are `Error::Parse`.
- CLI: `--ignore-commented-sheetnames` / `--ignore-hidden-sheets`
  (+ `--no-` forms). Neither format is an output format (`-f excel` stays
  unknown).
- Binary fixtures are generated, not hand-edited: `tools/gen-sheets`
  (a standalone zero-dependency Rust crate — it hand-writes stored-entry
  zips with a fixed timestamp, so output is byte-deterministic; it
  replaced an earlier python script on 2026-07-12) rebuilds every
  `.xlsx`/`.ods` fixture plus the `xlsx-sheets` example workbook:
  `cargo run --manifest-path tools/gen-sheets/Cargo.toml`. CI regenerates
  and `git diff --exit-code`s to prove the checked-in binaries match the
  generator.

### Value and format ids

`Value`: Null / Bool / Int(i64) / Uint(u64) / Int128(i128) /
Uint128(u128) / Float(f64) / String /
DateTime / Date / Time / Ipv4(`Ipv4Addr`) / Ipv6(`Ipv6Addr`) / Inet /
Cidr / MacAddr / MacAddr8 / Uuid / Array / Object(`BTreeMap`) — in
`src/value.rs` with the serde bridges. Textual variants keep their
validated input text; everything serializes as (canonical) strings, so
fixtures stay valid whether or not parser features are on — but
`format_id` differs, so fixture checks whose formats depend on a
feature are gated on it. A serialize→deserialize round trip through a
format without the corresponding type yields plain strings/numbers.
128-bit ints narrow to `i64`/`u64` on serialize when they fit (so
`load` and small-value traces show a JSON number); a value past `u64`
has no JSON number form and serializes as its decimal **string**, while
the trace `format` field still reports `i128`/`u128`.
`format_id()`: variant-derived — `null`, `bool`, `i64`, `u64`, `i128`,
`u128`, `f64`, `str`, `dt`, `date`, `time`, `ipv4`, `ipv6`, `inet`,
`cidr`, `macaddr`, `macaddr8`, `uuid`, `object`, and `arr:<t>` for
non-empty homogeneous scalar arrays else `arr`.

### CLI

`c4 [sources…] [flags]`. Positional arguments are path sources (folder
vs single file detected at load time); default source is `./config`. The
CLI exposes **every `Options` field** as a flag, so `--help` is the
canonical flag reference (README only points at it — and at
`src/main.rs`): output flags `-f`/`--format`, `-o`/`--output`,
`--trace`; `--order <id>` (`Order::from_id`); `--dir-depth <n>` (an
integer); one flag per boolean option (`--filename-as-key`,
`--dirname-as-key`, `--sheetname-as-key`, `--dot-key`,
`--case-sensitive`, `--auto-no-ext-files`, `--ignore-unknown-ext`,
`--ignore-commented-data-keys`, `--ignore-commented-filenames`,
`--ignore-commented-dirnames`, `--ignore-commented-sheetnames`,
`--ignore-hidden-sheets`), each with a
`--no-<name>` counterpart; and the `--tree` preset (sets the three
`*_as_key` flags true + `dir_depth = -1`; `--no-tree` resets them). Value
flags accept `--flag v` or `--flag=v`.
Neither `formats` nor any table setting is exposed (the `cli` feature
enables all formats; csv is positional-only — a header is a
`CustomFormat`, which the CLI cannot register — and table sources /
layouts are library-only: CLI positional sources are always plain
paths). Output format resolution ("auto"): `-f` (ids + `debug`;
`jsonc` = `json`) → `-o` extension → `debug` (Rust `{:#?}` of
`Value`/`TracedValue`). With `-o` nothing goes to stdout. Deterministic
output: sorted keys everywhere; json = two-space pretty + trailing
newline; env/ini = sorted dotted `key=value` lines (arrays embedded as
JSON strings, quoting only when needed); csv = `key,value,format` rows
with concrete ids (arrays/objects emit as a single `json` cell; typed
text values emit as `str` because the CLI works from the serialized
tree). Errors → stderr, exit 1.

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
against expect.json; `base()` is an explicit **flat** `Options` baseline
(keying off, `dir_depth: 1`) that fixture cases build on instead of
`Options::default()`, so their expectations hold even under
`--all-features` (where the `tree` feature flips the default keying on).
`loader(sources)` uses `base()` too. Fixture-dependent tests are
`#[cfg]`-gated on the features their fixtures need so every
feature-combination build stays green (watch interactions: `inet`
implies `ipv4`+`ipv6`+`cidr`, so "without ipv4" gates need
`not(inet)` too).

- `tests/basic.rs` — simple, merge_order, precedence (filename-order
  override), jsonc, empty, missing-folder error, struct
  deserialization, typed `TracedValue` assertions, `Value`
  accessors/indexing, `format_id` derivation.
- `tests/options.rs` — scan depth (`dir_depth: 0` skips subdirs,
  `dir_depth: -1` flat-scans every level), `dirname_as_key` nesting,
  order_folders_first, order_alphabetic, merge_order_reverse,
  case_insensitive, csv_dot_key, csv_flat_key, folder/file keying
  (tree_basic / tree_auto / tree_unknown / tree_order /
  tree_order_reverse, typed ipv4 assertion, strict unknown-ext error) —
  keying needs no feature, so this `mod tree` runs on any build — and the
  `tree`-feature default flip (`tree_feature_flips_defaults` /
  `default_is_flat_without_tree`), and the commented names/keys options
  (`mod commented`: the `commented_names`/`commented_names_off` variant
  pair — `_`/`#` files and folders skipped by default, loaded with the
  three name options off, a `.hidden.json` file proving `.` is not a
  prefix — the `commented_keys`/`commented_keys_off` pair for
  `ignore_commented_data_keys` — nested objects, objects inside arrays and a
  dot_key-expanded `#a.b` — plus non-fixture tests for the three
  "explicit wins" cases (a named commented **file** loads, a named
  commented **folder** scans but still filters its own `_wip/#deep/`
  subfolder, a **table source naming a `_z` sheet** reads it), that the
  keys made from names survive `ignore_commented_data_keys`, and that a
  `(value,)` source's keys are filtered). Fixture cases build on the explicit
  flat `common::base()`, not `Options::default()`, so they hold under
  `--all-features` (which turns `tree` on).
- `tests/formats.rs` — csv table schema (scalars + aliases, auto +
  leading-zero rule, numeric literals on/off, `i128`/`u128` typed cells
  + auto-never-widens, `bool` two-way tokens + strict case-insensitive
  auto, the `json`/`jsonc` cells gated on their own features, the
  `array`/`csv` list cells (array's native split + auto-typed elements +
  custom separator + empty→`[]` + a per-element `array<sep><format>`
  (u8-typed + str-forced elements + an out-of-range element error);
  csv's `csv<sep><layout>` grammar — kv
  and db layouts, custom separator, the positional `csvdb`→unknown and
  non-ASCII-separator and unknown-layout errors; array always compiled
  via `parse_table`, csv unknown without the `csv` feature — plus the
  `csv_list` fixture check), the dot_key array-segment tests (kv
  index/append/gap + path chains and `a[1][2]` suffix chains + the
  literal-shape and kind-collision rules + db `[]` columns, all via
  `parse_table` so they run under every feature set;
  `env_keys_take_array_suffixes` gated on env; the `csv_array_key`
  fixture), the `CustomFormat` table
  escape hatches that replace the old `TableOptions` — a header/renamed/
  reordered-columns format and a transposed (column-oriented) format,
  dt/date/time/ip/inet/cidr/mac/uuid typed +
  auto-guess + unknown-type + bad-value row assertions, PostgreSQL
  mac/inet/cidr forms), strict json, ext_override, formats_tuple,
  custom_md, toml/ini/env basics, toml datetime following the `datetime`
  feature — each behind its format's `#[cfg(feature)]`. Spreadsheet
  cases (all sheet content is db-shaped, the spreadsheet default
  layout): excel_basic (merge mode — `config` is the only non-ignored
  sheet: typed columns, an auto column, a dotted key column, a sparse
  record — + a `_`/`#`-prefixed and a hidden sheet skipped + a second
  workbook whose only sheet is ignored contributing nothing) plus
  `merge_mode_merges_every_sheet_like_a_file` (reuses the excel_tree
  workbook: two visible db sheets merge in name order, the later array
  winning), excel_hidden_config /
  excel_hidden_config_off (the `ignore_hidden_sheets` variant pair),
  excel_tree / excel_tree_prefix_off (`sheetname_as_key` sheet keys —
  including an `e.f` sheet proving `dot_key` expands sheet names — the
  `ignore_commented_sheetnames` variant pair; keying needs no feature),
  excel_dot_sheet (a single `.y` sheet: `.` is not a comment prefix),
  excel_datetime (serial date/time/dt cells typed by a db type row;
  gated on `datetime`),
  excel_bad (config only: bad typed cell on a padded grid asserts real
  spreadsheet row numbers), plus a single-file `.xlsx` path source and
  the string-source `Error::Parse`; ods_basic / ods_tree mirror the
  same rules for `ods`. Table layouts: csv_db (db grid: type row, sparse
  cells omitted, dotted key column) / csv_db_no_types (no type row —
  the insert-an-`auto`-row `CustomLayout` pattern; also the format-id
  tuple form) / csv_db_bad (config
  only: bad cell reports its real row) / csv_db_blank_types (all-blank
  type row = all `auto`; blank rows before the keys and between records
  still skip) / a csv-names-a-sheet error, all
  via table-source tuples; excel_blank_type_row (the same positional
  type row end-to-end: the workbook's row 2 physically absent, keys at
  row 1, records from row 3); excel_sheets (one workbook, five sheet-naming
  sources: kv + db + a no-type-row `CustomLayout` + a transposing
  `CustomLayout` + an
  explicitly named `_`-prefixed sheet, each keyed by sheet name — all
  4-tuples) and a
  missing-sheet error. Type-row validation: csv_db_bad/config/types.csv
  (a kv-shaped file under the db layout errors at the type row with the
  kv hint) and the same assertion against excel_kv_formats without its
  kv override. Formats-level layouts: csv_db_formats (a folder
  of csv claimed by `(csv, ["csv"], "db")`) and excel_kv_formats (a
  kv-shaped config sheet read via `(excel, ["xlsx"], "kv")` — the
  override direction opposite to each format's default). The binary fixtures come from
  `tools/gen-sheets`.
- `tests/sources.rs` — single file (Path form), a unified path source
  (folder vs file), multi_sources precedence, string-source override via
  a `(format, text)` tuple (trace label `string:<index>`), 1-tuple
  `(value,)` overrides (label `value:<index>`, serde shapes,
  non-string-key error at load time). Sources are a plain `vec![…]` with
  `.into()` per element (`order_converts_from_id` covers
  `Order::from_id`/`From<&str>`; `table_layout_converts_from_id`,
  `table_source_requires_a_table_format` and `table_source_must_be_a_file`
  cover the table-source surface).
- `examples/<name>/` — each example is a **standalone crate**: its own
  `Cargo.toml` (`c4 = { path = "../.." }`), `src/main.rs`, config files
  and a committed `output.log` with the exact expected output. Run with
  `cd examples/<name> && cargo run`; paths inside are relative to the
  example folder so the logs are machine-independent. The root crate
  sets `autoexamples = false`, so they are not cargo example targets
  and their dependencies (e.g. `notify` in `hot-reload`) stay out of the
  library's tree. `00_simple` / `01_advanced` (folders numbered to sort
  first; package names `example-00-simple`/`example-01-advanced`, since a
  crate name cannot start with a digit) mirror the
  README; `hot-reload` (renamed from `watch`, 2026-07-12) is the DIY
  hot-reload pattern (self-driving demo:
  scripted edits against a temp copy keep the log reproducible);
  `csv-header` (header + renamed/reordered columns) and `csv-transpose`
  (a column-oriented grid transposed into rows) are the table escape
  hatches — each a `CustomFormat` using the `csv` crate to read and
  `c4::parse_table` to interpret, replacing what `TableOptions` used to
  do. `xlsx-sheets` is the multi-sheet workbook pattern: one
  `config.xlsx` (generated by `tools/gen-sheets`), five sheet-naming table
  sources — kv, db, two `CustomLayout`s (no-type-row db, transpose), and an
  explicitly named `_`-prefixed sheet — plus a typed `Vec<Item>`
  deserialize of a db sheet. `csv-db` is the same db layout on a plain
  csv file: a `(Format::Csv, path, "db")` table source loaded straight
  into a `Vec<Item>`. `csv-list` is the `array`/`csv` list cell type ids
  on a kv csv file: an `array|` flat list, an `array` default-comma list,
  an `array|u8` per-element-format list, and a `csv,db` cell holding a
  whole nested record grid, all deserialized into a `Monster` struct
  (`Vec<String>` + `Vec<u8>` + `Vec<Attack>`). `dot-key` is the dot_key
  tour: kv rows with dotted nesting, out-of-order `[<int>]` indexes and
  `[]` appends into typed structs, a chained-suffix `grid[<i>][<j>]` /
  `grid[1][]` matrix into `Vec<Vec<u8>>`, and skipped `slots[1]` /
  `slots[4]` indexes into `Vec<Option<Slot>>` (gaps are `Null`); a db
  grid with a dotted column, repeated `drops[]` columns and `weak[0].*`
  columns; an env source showing the arrays-replace-across-sources
  caveat; and `dot_key: false` keeping brackets literal.
  After changing an example, regenerate its
  `output.log`.
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
cargo test --features excel,ods,tree,datetime   # spreadsheet formats
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
`--no-default-features` (including `excel` and `ods`), and
partial value-parser combinations (`csv,inet`, `csv,datetime`,
`csv,macaddr`, `csv,numeric`, `csv` alone — the last exercises the
`json`/`jsonc` cell ids with both features off, and `numeric` off —
plus `excel,tree,datetime` for the sheet-key and serial-datetime
paths).

## Architecture notes

- `src/lib.rs` — facade only: crate docs, the `compile_error!` guard,
  module declarations, re-exports, `load(path)` and
  `parse_table(rows, &layout, path, options)` (the single table entry —
  no layout-defaulting variant).
- `src/error.rs` — `Error` / `Result`.
- `src/options.rs` — the plain-data surface: `Options`, `Order`
  (id-convertible via `from_id`/`From<&str>`), `Format`, `FormatKind`,
  `FormatSpec`, `CustomFormat`, plus the crate-private `COMMENT_PREFIXES`
  (`#`, `_`) and `is_commented(name)` shared by the four
  `ignore_commented_*` options. No `TableOptions`/`TableColumns` — the
  table stage is optionless. `FormatSpec`/`CustomFormat` fields are
  crate-private (users construct via `From` / `CustomFormat::new` and
  never read them); `FormatKind` also converts from a format-id `&str`.
- `src/source.rs` — `Source` and its `From` conversions (no public
  constructors; path-like → `Path`, `(format, text)` → `String`,
  1-tuple `(value,)` → `Value` via `src/ser.rs`). There is no `sources!`
  macro and no `c4::value` — the `sources` list is a plain
  `vec![… .into()]`.
- `src/trace.rs` — `SourceRef`, `TracedValue` and the `$id` serialization.
- `src/loader.rs` — `Loader` plus all scanning/merging internals (scan,
  including the commented file/dir name skips → parse → merge with
  provenance; `load()` = `trace()` minus labels, one code path).
- `src/value.rs` — `Value`, accessors, `format_id`, serde bridges.
- `src/de.rs` — deserialize any serde type out of an owned `Value`
  (crate-private types).
- `src/ser.rs` — serialize any serde type into a `Value` (the engine
  behind the 1-tuple `(value,)` source; crate-private).
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
  `table.rs` (always compiled); sheet.rs (gated on
  `any(excel, ods)`, calamine) holds both spreadsheet formats —
  workbook opening, sheet selection, cell-to-text lowering (with unit
  tests for the serial-datetime conversion) — and feeds the same
  `table.rs`. Binary formats parse from the path: `format::parse` (the
  text entry) rejects them, and the loader calls `format::parse_binary`
  instead of `read_to_string` when `format::is_binary` says so.
- `src/main.rs` — the CLI (feature `cli`, `required-features` on the
  bin target).
- Feature map: `datetime = ["date", "time"]` and gates the `dt` id
  (`dt` is a format id, not a feature); `inet = ["ipv4", "ipv6",
  "cidr"]`; `macaddr = ["macaddr8"]`; `numeric` (off by default) gates
  extended table numeric literals; the `json` table cell id is gated on
  `json` and the `jsonc` cell id on `jsonc` (one-to-one with the file
  formats); `tree` is a **default-preset** (no code gate — it flips the
  keying defaults in `Options::default()` via `cfg!`, see "Folder
  keying"); `excel` and `ods` (both `dep:calamine`) are format features
  like any other; `cli` implies all formats + `datetime`, `inet`,
  `macaddr`, `uuid`, `numeric` — **not** `tree` (so the binary is
  flat-by-default). `default = ["jsonc"]`.
- Crate naming (decided 2026-07-05): the crates.io package is
  `c4-config` (verified available; `c4` itself is taken), while the lib
  target and the binary keep the name `c4` — users `cargo add c4-config`
  and write `use c4::…`. Example crates therefore depend on
  `c4-config = { path = "../.." }`. Known edge: the real `c4` crate also
  exposes lib name `c4`, so a crate that directly depends on **both**
  gets an extern-name clash — its author resolves it with a rename key
  (`x = { package = "c4", … }`); indirect coexistence in one dependency
  graph is fine (distinct crate metadata).
