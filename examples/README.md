# Examples

Complete runnable examples: each subfolder is a **standalone crate** — its
own `Cargo.toml`, config files, source, and an `output.log` showing exactly
what it prints:

```sh
cd examples/00_simple && cargo run
```

| Folder | Shows |
| ------ | ----- |
| [`00_simple`](00_simple) | the basic `load` from a config folder |
| [`01_advanced`](01_advanced) | multi-source usage (folders + files + in-code values) |
| [`csv-db`](csv-db) | a CSV record grid (the `db` layout) into a `Vec<Item>` |
| [`csv-header`](csv-header) | reshaping a CSV with a `CustomFormat` (header + renamed/reordered columns) |
| [`csv-list`](csv-list) | single cells expanded into lists via the `array` / `csv` type ids |
| [`csv-transpose`](csv-transpose) | a column-oriented CSV transposed into rows with a `CustomFormat` |
| [`dot-key`](dot-key) | everything `dot_key` does — dotted nesting plus `name[]` / `name[<int>]` array keys |
| [`hot-reload`](hot-reload) | DIY hot-reload — watch the folder with `notify`, re-run `load()` |
| [`xlsx-sheets`](xlsx-sheets) | one Excel workbook, a different table layout per sheet |

Beyond that, every behavior has a fixture under
[`tests/fixtures/`](../tests/fixtures/): `config/` is the input,
`expect.json` the merged result, and `expect.debug.json` the traced form —
if something is not covered here, there is a fixture showing it.
