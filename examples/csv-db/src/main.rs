//! The `db` table layout on a plain CSV file.
//!
//! `items.csv` is a record grid — row 1 the keys, row 2 the type ids
//! (an empty cell = `auto`), every following row one record. A
//! `(Format::Csv, path, "db")` table source parses it to an **array**
//! of objects, so the whole file deserializes straight into a
//! `Vec<Item>`. Empty cells are omitted from their record — pair them
//! with `Option` (or `#[serde(default)]`) on the struct side. The
//! dotted `stats.atk` column nests per record (`dot_key`).
//!
//! Run inside this folder: `cd examples/csv-db && cargo run`
//! (expected output: `output.log` next to this file)

use c4::{Format, Loader, Options};

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)] // fields are shown via Debug
struct Item {
    id: u32,
    name: String,
    price: f64,
    stats: Option<Stats>,
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct Stats {
    atk: i32,
}

fn main() -> Result<(), c4::Error> {
    let loader = Loader::new(Options {
        sources: vec![(Format::Csv, "items.csv", "db").into()],
        ..Options::default()
    });

    // the file's root is the array itself — deserialize it directly
    let items: Vec<Item> = loader.load()?;
    for item in &items {
        println!("{item:?}");
    }
    Ok(())
}
