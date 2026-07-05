//! Mirrors the README "Advanced" usage: several ordered sources (a
//! folder scanned recursively, an in-code string, a typed in-code
//! value), then a provenance trace.
//!
//! Run inside this folder: `cd examples/readme-advanced && cargo run`
//! (expected output: `output.log` next to this file)

use std::path::Path;

use c4::{Format, Loader, Options, Source};

#[derive(serde::Serialize)]
struct Overrides {
    debug: bool,
}

fn main() -> Result<(), c4::Error> {
    let loader = Loader::new(Options {
        sources: vec![
            Source::folder(Path::new("config")), // any path-like type works
            Source::string(Format::Jsonc, r#"{ "note": "from code" }"#),
            Source::value(Overrides { debug: true }),
        ],
        recursive: true,
        ..Options::default()
    });

    let value: c4::Value = loader.load()?;
    println!("value = {value:#?}");

    // where did every leaf come from, and what format is it?
    println!("trace = {:#?}", loader.trace()?);
    Ok(())
}
