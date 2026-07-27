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

In the AI era, you often don't need a config library. If your application
loads a single settings file, let your AI generate the loading code —
**zero dependencies** is still the lightest solution.

**c4 is for projects where conventions matter.**

It defines a simple, predictable rule set instead of making every project
reinvent one:

- **Deterministic merging** — later sources override earlier ones, files
  merge in filename order, objects merge deeply, arrays and scalars
  replace.
- **Spreadsheet-first tables** — the same table conventions work in CSV,
  Excel and OpenDocument, with typed values instead of stringly-typed data.
- **Easy escape hatches** — when the defaults don't fit, implement a
  `CustomFormat` or `CustomLayout` while reusing the same parsing
  pipeline.

### Design goals

- **Easy to remember** — a small API that covers most use cases.
- **Composable** — formats, parsers and behavior are all optional.
- **Extensible** — custom formats integrate without forking the library.
- **Test-driven** — documented behavior is backed by tests.

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
features, format/extension mapping, merge rules, folder shape (depth and
folder/file/sheet keying), commented-out names and keys (`#`/`_`), typed
table cells, custom formats and provenance tracing — lives in the crate
docs, with runnable examples:

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
c4 --tree                 # key by folder/file/sheet name (a preset)
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
