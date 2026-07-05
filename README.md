# c4

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

### Simple

One call, one path — a folder whose files deep-merge, or a single file.
Default options (formats: jsonc + yaml):

```rust
let value: c4::Value = c4::load("config")?;   // a folder …
let other: c4::Value = c4::load("app.yml")?;  // … or one file

// dynamic access: index into objects/arrays — a missing key yields
// Value::Null instead of panicking — and convert with the as_* accessors
let host = value["db"]["host"].as_str().unwrap_or("localhost");
let port = value["db"]["port"].as_u64().unwrap_or(5432);
let first = value["servers"][0].as_str();

// same function — the annotated target type decides what you get:
// c4::Value for dynamic access, or any serde type for typed config
#[derive(serde::Deserialize)]
struct MyConfig {
    host: String,
    port: u16,
}
let cfg: MyConfig = c4::load()?;
```

### Advanced

Sources load in the order given — later sources override earlier ones. A
source is a folder, a single file, or an in-code string. Paths accept
anything path-like (`&str`, `String`, `&Path`, `PathBuf`):

```rust
use std::path::Path;

use c4::{Format, Loader, Options, Source};

#[derive(serde::Serialize)]
struct Overrides {
    debug: bool,
}

let value: c4::Value = Loader::new(Options {
        sources: vec![
            Source::folder("./config"),
            Source::folder(Path::new("/etc/myapp")), // any path-like type
            Source::file("./local.yml"),
            Source::string(Format::Jsonc, r#"{ "note": "from code" }"#),
            Source::value(Overrides { debug: true }), // typed override, no parser
        ],
        recursive: true,
        formats: vec![
            "jsonc".into(),                              // format id, default extensions
            Format::Toml.into(),                         // enum form, default extensions
            (Format::Yaml, ["yml", "yaml", "conf"]).into(), // custom extensions
            ("jsonc", ["json", "jsonc"]).into(),         // string id + custom extensions
        ],
        ..Options::default()
    })
    .load()?;
```

Everything goes through one plain-data `Options` struct — sources
included; `Loader` has exactly `new(options)`, `load()` and `trace()`:

| Option | Default | Meaning |
| ------ | ------- | ------- |
| `sources` | `["config"]` folder | the ordered sources; later overrides earlier |
| `formats` | all compiled-in formats | which formats read which extensions; the **last** claimer of an extension owns it |
| `recursive` | `false` | scan subdirectories (merge mode) |
| `flat` | `true` | when recursive, ignore subfolder paths; set `false` to nest them as keys (folder-shaped config is tree mode's job) |
| `dot_key` | `true` | expand dotted env/table keys: `a.b.c` → `{a:{b:{c:…}}}` |
| `case_sensitive` | `true` | `false` lowercases keys while merging (extension matching is always case-insensitive) |
| `order` | folders first, then files, each alphabetic | load order inside a folder — also decides tree-mode key collisions |
| `table` | see Table formats | header row, column names, extended types, array delimiter |
| `tree` | `false` | tree mode, see below (needs the `tree` feature) |
| `auto_files` | `true` | tree mode: auto-detect the content of extension-less files (table-`auto` logic) |
| `ignore_unknown_ext` | `true` | tree mode: skip files with unclaimed extensions instead of erroring |

## Cargo features

`default = ["jsonc", "yaml", "numeric"]` — at least one format feature
is required (compile error otherwise).

| Feature | Default | Enables |
| ------- | :-----: | ------- |
| `jsonc` | ✓ | JSONC files (comments + trailing commas; superset of JSON) |
| `yaml` | ✓ | YAML files |
| `numeric` | ✓ | extended numeric literals: `0x`/`0b`/`0o`, `_` separators, `123n` |
| `json` | | strict JSON files, and the table `json` cell type |
| `toml` | | TOML files (datetime literals follow `datetime`) |
| `ini` | | INI files |
| `env` | | env files (`KEY=VALUE`) |
| `csv` | | CSV table files |
| `tree` | | tree mode (`Options.tree`) |
| `datetime` | | the `dt` table format — implies `date` + `time` |
| `date`, `time` | | the `date` / `time` table formats |
| `ipv4`, `ipv6` | | the `ipv4` / `ipv6` table formats |
| `inet` | | the `inet` table format — implies `ipv4` + `ipv6` + `cidr` |
| `cidr` | | the `cidr` table format |
| `macaddr` | | the `macaddr` table format — implies `macaddr8` |
| `macaddr8` | | the `macaddr8` table format |
| `uuid` | | the `uuid` table format |
| `cli` | | the `c4` binary — implies everything above |

Value-parser features are pure std (no extra dependencies). Note for
libraries depending on c4: Cargo features are unioned across the whole
build graph, so only applications should decide format features.

## Formats

Format and file extension are two different things: each format claims a
default set of extensions, and both sides are configurable.

| Format  | Default extensions | Default on |
| ------- | ------------------ | ---------- |
| `jsonc` | `.json`, `.jsonc`  | yes        |
| `yaml`  | `.yml`, `.yaml`    | yes        |
| `json`  | `.json` (strict)   | -          |
| `toml`  | `.toml`            | -          |
| `ini`   | `.ini`             | -          |
| `env`   | `.env`, `*.env`    | -          |
| `csv`   | `.csv`             | -          |

- Extension matching is case-insensitive; a hidden file `.X` counts as
  having extension `X` (that is how `.env` matches `env`).
- Unclaimed extensions are ignored; the last claimer of a contested
  extension wins — that also lets you reassign extensions:
  `formats: vec![Format::Yaml.into(), (Format::Jsonc, ["yml"]).into()]`
  makes the jsonc parser read `.yml` files.

## Merge rules

1. Sources merge in the order given; later overrides earlier.
2. Within a folder, entries load in `order` and deep-merge: objects
   merge recursively, arrays and scalars are replaced.
3. Override order between files is decided by **filename alone**, never
   by format: `app.json` + `app.yml` sort by name, so `app.yml` loads
   later and wins. Prefix filenames (`00_a.json`, `01_a.yml`) for
   explicit control.

## Tree mode

With `Options { tree: true, .. }` (Cargo feature `tree`) a folder is not
merged — its shape becomes the config: every subfolder is a key, and
every file is a key named after the file (extension stripped) holding
that file's parsed content:

```text
config/a/b.json = {"c": 1}      →  { "a": { "b": { "c": 1 } },
config/d.json   = {"a": 123}         "d": { "a": 123 } }
```

Always recursive; entries load in `order`, and key collisions (`a.yml`
next to a folder `a/`) deep-merge, so the order decides who wins.
Extension-less files are auto-detected from their content when
`auto_files: true` (`a/blabla` containing `1.1.1.1` becomes an IPv4
value with the `ipv4` feature); files with unknown extensions are
skipped unless `ignore_unknown_ext: false`.

## Table formats (`csv` today, spreadsheets later)

Table rows map to config entries as `key,value[,format]` — no header row
by default, and the format column is optional (missing/empty = `auto`):

```csv
name,hello
port,8080,u16
db.host,localhost,str
born,2024-01-02,dt
```

| Format id | Needs feature | Accepts |
| --------- | ------------- | ------- |
| `i8`–`i64`, `u8`–`u64`, `f32`, `f64` | – | numbers (aliases: `int`, `uint`, `float`, `number`, …) |
| `bool`, `str` | – | `true`/`false`; any text (aliases: `boolean`, `string`, `text`) |
| `auto` | – | auto-detection, see below |
| `dt` (alias `datetime`) | `datetime` | `YYYY-MM-DD[Thh:mm:ss[.frac]][Z\|±hh:mm]` |
| `date`, `time` | `date`, `time` | `YYYY-MM-DD` / `hh:mm:ss[.frac]` |
| `ipv4`, `ipv6` | `ipv4`, `ipv6` | one IP family each |
| `inet` | `inet` | PostgreSQL inet: host address, optional netmask, host bits allowed |
| `cidr` | `cidr` | PostgreSQL cidr: network, optional netmask, host bits must be zero |
| `macaddr`, `macaddr8` | `macaddr`, `macaddr8` | the PostgreSQL MAC groupings (`08:00:2b:…`, `08002b:010203`, `0800.2b01.0203`, bare hex) |
| `uuid` | `uuid` | `8-4-4-4-12` hyphenated or bare 32 hex |
| `null`, `arr:<t>`, `json` | opt-in via `Options.table.types` (`json` also needs the `json` feature) | null; delimited arrays (default `;`); a JSON document |

- Without its feature an id is simply an unknown format and the row
  errors.
- `auto` tries bool, then every *enabled* typed format (cheap shapes
  first: date → time → dt → hyphenated uuid → MAC pair spellings →
  ipv4 → ipv6 → cidr → inet, the last two only when a `/` is present),
  then integers (leading-zero numbers like `007` stay strings), then
  floats, otherwise string. Loose spellings never auto-convert — bare
  hex only becomes a UUID/MAC when the format column says so.
- Numeric literals (feature `numeric`, on by default): `0x`/`0b`/`0o`
  prefixes, `1_000_000` separators and a BigInt-style `123n` suffix
  work wherever a string becomes a number.
- `Options.table.header: true` reads a header row and locates columns
  by name (`Options.table.columns`, default `key`/`value`/`format`).
- A value that does not fit its declared format is an error carrying
  the file path and the 1-based row number.

## Custom formats

A `CustomFormat` is an id, the extensions it claims, and a parser
callback. It goes into `Options.formats` like any built-in — one list,
one claim order — and a string source can name one directly.
Table-shaped formats lower their file into rows and reuse the generic
table stage via `c4::parse_table`. A markdown pipe-table as a config
format:

```rust
use c4::{CustomFormat, Loader, Options, Source};

let md = CustomFormat::new("md-table", ["md"], |text, path, options| {
    let rows: Vec<Vec<String>> = text
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with('|') && !l.contains("---"))
        // markdown tables always start with a header row — dropping it
        // here keeps the data rows positional (key,value[,format]) with
        // no Options.table changes needed
        .skip(1)
        .map(|l| {
            l.trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().to_owned())
                .collect()
        })
        .collect();
    c4::parse_table(rows, path, options)
});

// to read `.md` files, push `md` into Options.formats — custom formats
// claim extensions exactly like built-ins do
let value: c4::Value = Loader::new(Options {
        sources: vec![Source::string(
            md,
            "| key  | value | format |
             | ---- | ----- | ------ |
             | name | c4    | str    |
             | port | 8080  | u16    |",
        )],
        ..Options::default()
    })
    .load()?;
assert_eq!(value["port"].as_u64(), Some(8080));
```

## Trace (provenance)

`trace()` returns the merged tree with, per leaf, the source it came
from and the format it parsed as:

```rust
let traced: c4::TracedValue = c4::Loader::default().trace()?;
```

It serializes to `{ "$id": "Leaf", "value": …, "source": …, "format": … }`
leaves — the shape the CLI `--trace -f json` prints and the test
fixtures assert. `$id` tags the node kind (object nodes are plain maps),
`source` is the file path (`string:<i>` / `value:<i>` for in-code
sources), and
`format` is `Value::format_id()` — `str`, `i64`, `bool`, `dt`, `ipv4`,
`arr:i64`, … Printed with `{traced:#?}` (what a bare `c4 --trace`
prints) it looks like:

```text
Object(
    {
        "port": Leaf {
            value: Int(8080),
            source: File("config/app.json"),
        },
    },
)
```

## CLI

The `cli` feature builds a `c4` binary (all formats and value parsers)
that loads config sources and writes the merged result as one document:

```sh
c4                          # read ./config, print the Rust Debug form
c4 ./config ./local.toml    # explicit sources (folders and/or files)
c4 -f yaml                  # choose the output format
c4 -o merged.toml           # write to a file, format inferred from extension
c4 --trace                  # annotate every value with source + format
c4 --trace -f json          # the same provenance tree as JSON
c4 --tree                   # tree mode: folders/files become keys
```

- Output format: `-f` wins, else the `-o` extension, else `debug` (the
  Rust `{:#?}` form). `-f` accepts
  `json`/`jsonc`/`yaml`/`toml`/`ini`/`env`/`csv`/`debug`.
- Output is deterministic (sorted keys, pretty two-space json, trailing
  newline), so tests compare it byte-for-byte.
- Flat formats (env/ini/csv) emit nested keys dotted; csv always emits
  `key,value,format` rows; env/ini embed arrays as JSON strings.
- Errors go to stderr with a non-zero exit.

## Examples

Complete runnable examples live under [`examples/`](examples/): each
subfolder is a **standalone crate** — its own `Cargo.toml`, config
files, source, and an `output.log` showing exactly what it prints:

```sh
cd examples/readme-simple && cargo run
```

Besides the README mirrors, `examples/watch` shows DIY hot-reload —
watching the config folder with the `notify` crate and re-running
`load()` on changes; c4 itself stays a synchronous loader on purpose. Beyond that, every behavior has
a fixture under [`tests/fixtures/`](tests/fixtures/): `config/` is the
input, `expect.json` the merged result, and `expect.debug.json` the
traced form — if something is not covered in `examples/`, there is a
fixture showing it.

## Contributing

Conventions, test layout and the development workflow live in
[CONTRIBUTING.md](CONTRIBUTING.md). Quick start:

```sh
cargo test                       # default features
cargo test --all-features        # every format + value parser
```
