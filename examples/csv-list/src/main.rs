//! The `array` and `csv` list cell type ids.
//!
//! `config.csv` is a plain `key,value,format` file (the `kv` layout) —
//! but three of its cells expand into lists:
//!
//! - `tags` uses `array|`: a flat list split on `|`, each element
//!   auto-typed (`boss|flying|fire` → `["boss", "flying", "fire"]`).
//! - `loot` uses `array`: the same, with the default `,` separator (the
//!   value is quoted so the outer csv keeps it as one field).
//! - `phases` uses `array|u8`: `array<sep><format>` applies one type id
//!   to **every** element, so each is parsed as `u8` instead of the
//!   auto-guessed `i64`. For a list whose elements need *different*
//!   formats, reach for a `csv` cell instead (`a,1,i8` / `b,2,i16` rows).
//! - `attacks` uses `csv,db`: the **whole cell** is parsed as its own
//!   CSV document (separator `,`) under the `db` layout — a key row, a
//!   type-id row, then one record per row — so it becomes an array of
//!   objects. `csv<sep><layout>` picks both the inner separator and the
//!   layout; `csv,kv` would instead give an object.
//!
//! Because `array` splits each piece through `auto` and `csv` runs the
//! normal table stage, everything deserializes straight into the
//! `Monster` struct below.
//!
//! Run inside this folder: `cd examples/csv-list && cargo run`
//! (expected output: `output.log` next to this file)

use c4::{Format, Loader, Options};

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)] // fields are shown via Debug
struct Monster {
    name: String,
    tags: Vec<String>,
    loot: Vec<String>,
    phases: Vec<u8>,
    attacks: Vec<Attack>,
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct Attack {
    name: String,
    dmg: u32,
    element: String,
}

fn main() -> Result<(), c4::Error> {
    let loader = Loader::new(Options {
        // a single csv file under the kv layout — its root is an object
        sources: vec![(Format::Csv, "config.csv", "kv").into()],
        ..Options::default()
    });

    let monster: Monster = loader.load()?;
    println!("{monster:#?}");
    Ok(())
}
