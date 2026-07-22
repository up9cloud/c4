//! Mirrors the README "Advanced" usage: several ordered sources (a
//! folder scanned to every depth, an in-code string, a typed in-code
//! value), then a provenance trace.
//!
//! Run inside this folder: `cd examples/01_advanced && cargo run`
//! (expected output: `output.log` next to this file)

use std::path::Path;

use c4::{Format, Loader, Options};

#[derive(serde::Serialize)]
struct Overrides {
    debug: bool,
}

fn main() -> Result<(), c4::Error> {
    // each source converts with .into(): a path-like value is a
    // folder/file source, a (format, text) tuple is a string source, and
    // a 1-tuple (value,) wraps a serde type as a typed override
    let loader = Loader::new(Options {
        sources: vec![
            Path::new("config").into(),              // any path-like type works
            (Format::Jsonc, r#"{ "note": "from code" }"#).into(),
            (Overrides { debug: true },).into(),
        ],
        dir_depth: -1, // scan every subdirectory level
        ..Options::default()
    });

    let value: c4::Value = loader.load()?;
    println!("value = {value:#?}");

    // where did every leaf come from, and what format is it?
    println!("trace = {:#?}", loader.trace()?);
    Ok(())
}
