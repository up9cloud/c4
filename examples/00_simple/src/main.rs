//! Mirrors the README "Simple" usage: `c4::load(path)` takes a folder
//! (or a single file); values are accessed dynamically or deserialized
//! into a struct.
//!
//! Run inside this folder: `cd examples/00_simple && cargo run`
//! (expected output: `output.log` next to this file)

// #[serde(default)] falls back to Default for any field no source set
#[derive(Debug, serde::Deserialize)]
#[serde(default)]
#[allow(dead_code)] // fields are read via the Debug print
struct MyConfig {
    name: String,
    port: u16,
}

impl Default for MyConfig {
    fn default() -> Self {
        Self {
            name: "unnamed".into(),
            port: 5432,
        }
    }
}

fn main() -> Result<(), c4::Error> {
    // one call, one path — the folder's files deep-merge
    let value: c4::Value = c4::load("config")?;
    println!("value = {value:#?}");
    println!("db.host = {:?}", value["db"]["host"].as_str());

    // the same call takes a single file too
    let from_file: c4::Value = c4::load("config/app.json")?;
    println!("app.json only = {from_file:#?}");

    let cfg: MyConfig = c4::load("config")?;
    println!("cfg = {cfg:#?}");
    Ok(())
}
