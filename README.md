# c4

[![Documentation](https://img.shields.io/crates/v/c4-config?label=latest)](https://docs.rs/c4-config)
[![build status](https://github.com/up9cloud/c4/actions/workflows/main.yml/badge.svg?branch=master)](https://github.com/up9cloud/c4/actions)
![Downloads](https://img.shields.io/crates/d/c4-config.svg)

Load config from folders, files and in-code strings/values into one
deep-merged value. Formats are individually selectable via Cargo
features, with a table (CSV) format whose cells carry typed values,
per-value provenance tracing, and a CLI.

```sh
cargo add c4-config    # the package is c4-config; in code it is `c4`
```

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
(default formats: jsonc + yaml):

```rust
let value: c4::Value = c4::load("config")?;   // a folder …
let one: c4::Value = c4::load("app.yml")?;    // … or a single file

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
cd examples/readme-simple && cargo run
```

The `readme-simple` / `readme-advanced` crates walk through basic and
multi-source usage, and `examples/watch` shows DIY hot-reload — watching
the config folder with the `notify` crate and re-running `load()` on
changes; c4 itself stays a synchronous loader on purpose. Beyond that,
every behavior has a fixture under [`tests/fixtures/`](tests/fixtures/):
`config/` is the input, `expect.json` the merged result, and
`expect.debug.json` the traced form — if something is not covered in
`examples/`, there is a fixture showing it.

## Contributing

Conventions, test layout and the development workflow live in [CONTRIBUTING.md](CONTRIBUTING.md).
