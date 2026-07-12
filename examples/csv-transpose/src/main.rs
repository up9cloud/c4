//! Transposed (column-oriented) csv — another `CustomFormat` escape hatch.
//!
//! Some tables put each *record* in a column rather than a row: one row of
//! keys, one of values, one of formats:
//!
//! ```text
//! a,b,c            <- keys
//! 1,1.1.1.1,8080   <- values
//! int,ipv4         <- formats (optional; the last column has none)
//! ```
//!
//! Transposing the grid turns each **column** into the positional
//! `[key, value, format]` row that `c4::parse_table` expects, so the file
//! above loads as `{ a: 1, b: "1.1.1.1", c: 8080 }`: `a` is an int, `c` is
//! auto-detected, and `b`'s `ipv4` format *validates* the cell (a bad
//! address would error). `load()` returns the value's canonical string;
//! the typed `Value::Ipv4` only survives in `trace()`. The `csv` crate
//! reads the grid.
//!
//! Run inside this folder: `cd examples/csv-transpose && cargo run`
//! (expected output: `output.log` next to this file)

use std::path::Path;

use c4::{CustomFormat, Error, Loader, Options};

fn main() -> Result<(), Error> {
    let csv_transpose = CustomFormat::new("csv-transpose", ["csv"], |text, path, options| {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true) // the format row may be shorter than the rest
            .trim(csv::Trim::All)
            .from_reader(text.as_bytes());

        // read the whole grid, then transpose: column `col` across every
        // row becomes one positional [key, value, format] row (a missing
        // cell — e.g. the last column's format — is just an empty string,
        // which parse_table reads as `auto`)
        let grid: Vec<csv::StringRecord> = reader
            .records()
            .collect::<Result<_, _>>()
            .map_err(|e| parse_err(path, e))?;
        let width = grid.iter().map(csv::StringRecord::len).max().unwrap_or(0);
        let rows: Vec<Vec<String>> = (0..width)
            .map(|col| {
                grid.iter()
                    .map(|record| record.get(col).unwrap_or("").to_owned())
                    .collect()
            })
            .collect();
        c4::parse_table(rows, &c4::TableLayout::Kv, path, options)
    });

    let grid = "\
a,b,c
1,1.1.1.1,8080
int,ipv4";

    let value: c4::Value = Loader::new(Options {
        sources: vec![(csv_transpose, grid).into()],
        ..Options::default()
    })
    .load()?;

    println!("a = {:?}", value["a"].as_i64()); // explicit int
    println!("b = {:?}", value["b"].as_str()); // ipv4-validated, canonical string
    println!("c = {:?}", value["c"].as_u64()); // no format cell -> auto
    Ok(())
}

fn parse_err(path: &Path, e: csv::Error) -> Error {
    Error::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    }
}
