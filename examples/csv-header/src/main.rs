//! Header rows and renamed / reordered columns for csv — the escape hatch.
//!
//! The built-in `csv` format is deliberately headerless and positional:
//! each row is `key,value[,format]` (col 0 / 1 / 2). When your file has a
//! header — or the key/value/format columns are renamed or in a different
//! order — you handle the header yourself in a `CustomFormat` and lower
//! the file to those positional rows via `c4::parse_table`. This recreates
//! exactly what the old `Options.table` header/columns settings did.
//!
//! The row parsing here uses the `csv` crate (quoting/escaping handled for
//! you), so the `CustomFormat` is just: read the header, find the columns
//! by name, emit `[key, value, format]` rows.
//!
//! Run inside this folder: `cd examples/csv-header && cargo run`
//! (expected output: `output.log` next to this file)

use std::path::Path;

use c4::{CustomFormat, Error, Loader, Options};

fn main() -> Result<(), Error> {
    let csv_header = CustomFormat::new("csv-header", ["csv"], |text, path, options| {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .trim(csv::Trim::All)
            .from_reader(text.as_bytes());

        // locate the key / value / format columns by name (any order)
        let header = reader.headers().map_err(|e| parse_err(path, e))?.clone();
        let col = |name: &str| header.iter().position(|h| h == name);
        let (Some(key), Some(value)) = (col("key"), col("value")) else {
            return Err(Error::Parse {
                path: path.to_path_buf(),
                message: "csv-header needs 'key' and 'value' columns".into(),
            });
        };
        let format = col("format");

        // rebuild each record as a positional [key, value, format] row
        let mut rows = Vec::new();
        for record in reader.records() {
            let record = record.map_err(|e| parse_err(path, e))?;
            let mut row = vec![record[key].to_owned(), record[value].to_owned()];
            if let Some(cell) = format.and_then(|f| record.get(f)) {
                row.push(cell.to_owned());
            }
            rows.push(row);
        }
        c4::parse_table(rows, path, options)
    });

    // the header names the columns, so they can be reordered freely — here
    // the file is value,key,format instead of the positional key,value,format
    let text = "\
value,key,format
c4,name,str
8080,port,u16
true,debug,bool";

    let value: c4::Value = Loader::new(Options {
        sources: vec![(csv_header, text).into()],
        ..Options::default()
    })
    .load()?;

    println!("name = {:?}", value["name"].as_str());
    println!("port = {:?}", value["port"].as_u64());
    println!("debug = {:?}", value["debug"].as_bool());
    Ok(())
}

fn parse_err(path: &Path, e: csv::Error) -> Error {
    Error::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    }
}
