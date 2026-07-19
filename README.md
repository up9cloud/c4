# c4

[![Documentation](https://img.shields.io/crates/v/c4-config?label=latest)](https://docs.rs/c4-config)
[![build status](https://github.com/up9cloud/c4/actions/workflows/main.yml/badge.svg?branch=master)](https://github.com/up9cloud/c4/actions)
![Downloads](https://img.shields.io/crates/d/c4-config.svg)

Load config from folders, files and in-code strings/values into one
deep-merged value. File formats are individually selectable via Cargo
features (default: just JSONC), with table formats (CSV and
Excel/OpenDocument spreadsheets) whose cells carry typed values,
per-value provenance tracing, and a CLI.

```sh
cargo add c4-config    # the package is c4-config; in code it is `c4`
```

## Why c4

Honestly: in the AI era you may not need a config library at all. If
your app reads one simple settings file, ask your AI assistant to
write bespoke loading code instead — **zero dependencies** is lighter
than any library, including this one. Don't add c4 for that.

What still deserves a library is a **convention**. node-config's real
contribution was never convenience — it was a rule set a whole team
could point at: multiple files, deterministic override order, deep
merge. c4 exists to be that rule set for Rust:

- **Deterministic merging you don't re-invent per project** — later
  sources override earlier, filenames decide order inside a folder,
  objects merge deep, arrays and scalars replace. One sentence,
  always true.
- **Table conventions non-programmers can own** — `key,value[,format]`
  rows for settings (the CSV default), or a `db` record grid (keys /
  types / rows → an array of typed objects; the spreadsheet default)
  for data tables, the same whether they live
  in a CSV file or an Excel/OpenDocument sheet. Planners — game
  designers especially — edit config in the spreadsheet tools they
  already use, with typed cells (integers, bools, dates, IPs, UUIDs, …)
  instead of stringly data, and no programmer in the loop.
- **Escape hatches that are documented, not discovered** — when the
  convention doesn't fit (header rows, transposed grids, a homegrown
  format), a `CustomFormat` or `CustomLayout` is a few lines reusing
  the same table stage. Every hatch ships as a runnable example under
  [`examples/`](examples/).

**Project goals — ease of use first:**

- An API you can remember: a handful of names, and most jobs are a few
  lines.
- Everything optional and changeable: formats, value parsers and modes
  are Cargo features and plain-data `Options` — take only what you use.
- Test-driven: every documented behavior is covered by tests before it
  is implemented.
- An escape hatch: custom formats let you support a file format
  yourself before (or instead of) waiting for a release.

## Usage

One call, one path — a folder whose files deep-merge, or a single file
(the default build reads jsonc; every other format — yaml, toml, ini,
env, csv, excel, ods, strict json — is one Cargo feature away):

```rust
let value: c4::Value = c4::load("config")?;   // a folder …
let one: c4::Value = c4::load("app.json")?;   // … or a single file

// dynamic access: index into objects/arrays — a missing key yields
// Value::Null instead of panicking — and convert with the as_* accessors
let host = value["db"]["host"].as_str().unwrap_or("localhost");
let port = value["db"]["port"].as_u64().unwrap_or(5432);
let first = value["servers"][0].as_str();

// same call — the annotated target type decides what you get:
// a dynamic c4::Value, or any serde type for typed config.
// The recommended pattern is #[serde(default)]: any key no source sets
// falls back to your Default, so partial config files just work.
#[derive(serde::Deserialize)]
#[serde(default)]
struct MyConfig {
    host: String,
    port: u16,
}

impl Default for MyConfig {
    fn default() -> Self {
        Self { host: "localhost".into(), port: 5432 }
    }
}

let cfg: MyConfig = c4::load("config")?;
```

## Documentation

The full reference — mixed sources and every `Options` field, the Cargo
features, format/extension mapping, merge rules, tree mode, typed table
cells, custom formats and provenance tracing — lives in the crate docs,
with runnable examples:

**📖 [docs.rs/c4-config](https://docs.rs/c4-config)**

(`CLAUDE.md` in the repo is the exhaustive, normative spec.)

## CLI

The `cli` feature builds a `c4` binary (all formats and value parsers)
that loads config sources and writes the merged result as one document.
Every `Options` field is a flag (each boolean also has a `--no-<name>`
form), plus output flags `-f`/`-o`/`--trace`:

```sh
c4                        # read ./config, print the Rust Debug form
c4 ./config -f yaml       # explicit sources, choose the output format
c4 --trace -f json        # provenance tree (source + format per value) as JSON
c4 --tree                 # tree mode: folders/files become keys
c4 --help                 # every flag, with examples and notes
```

The full flag list, defaults and examples live in `c4 --help` (and, if
you want the exact behavior, [`src/main.rs`](src/main.rs)). In short:
`-f` wins over the `-o` extension, else output is `debug` (the Rust
`{:#?}` form); output is deterministic (sorted keys, two-space json,
trailing newline) so tests compare it byte-for-byte; flat formats
(env/ini/csv) dot nested keys; and errors go to stderr with a non-zero
exit.

## Examples

Complete runnable examples live under [`examples/`](examples/): each
subfolder is a **standalone crate** — its own `Cargo.toml`, config
files, source, and an `output.log` showing exactly what it prints:

```sh
cd examples/00_simple && cargo run
```

| Folder | Shows |
| ------ | ----- |
| [`00_simple`](examples/00_simple) | the basic `load` from a config folder |
| [`01_advanced`](examples/01_advanced) | multi-source usage (folders + files + in-code values) |
| [`csv-db`](examples/csv-db) | a CSV record grid (the `db` layout) into a `Vec<Item>` |
| [`csv-header`](examples/csv-header) | reshaping a CSV with a `CustomFormat` (header + renamed/reordered columns) |
| [`csv-list`](examples/csv-list) | single cells expanded into lists via the `array` / `csv` type ids |
| [`csv-transpose`](examples/csv-transpose) | a column-oriented CSV transposed into rows with a `CustomFormat` |
| [`dot-key`](examples/dot-key) | everything `dot_key` does — dotted nesting plus `name[]` / `name[<int>]` array keys |
| [`hot-reload`](examples/hot-reload) | DIY hot-reload — watch the folder with `notify`, re-run `load()` |
| [`xlsx-sheets`](examples/xlsx-sheets) | one Excel workbook, a different table layout per sheet |

Beyond that,
every behavior has a fixture under [`tests/fixtures/`](tests/fixtures/):
`config/` is the input, `expect.json` the merged result, and
`expect.debug.json` the traced form — if something is not covered in
`examples/`, there is a fixture showing it.

## Contributing

Conventions, test layout and the development workflow live in [CONTRIBUTING.md](CONTRIBUTING.md).
