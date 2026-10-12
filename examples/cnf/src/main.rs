//! `.cnf` files — `my.cnf`, `openssl.cnf` and the like — plus the trace's
//! warnings and typing one section of a loaded value.
//!
//! The `cnf` parser reads these dialects as they are: `[ section ]`
//! headers, `#` comments, bare MySQL flags (`skip-networking` is `true`),
//! quoted values. Every value is a string, yet `into_typed` fills a
//! `u16` or `bool` field from it. Include directives are not followed:
//! each is skipped with a warning, which `trace()` reports (the CLI
//! prints them to stderr).
//!
//! Run inside this folder: `cd examples/cnf && cargo run`
//! (expected output: `output.log` next to this file)

use c4::{Loader, Options};

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)] // fields are shown via Debug
struct Mysqld {
    port: u16,
    #[serde(rename = "bind-address")]
    bind_address: String,
    #[serde(rename = "skip-networking", default)]
    skip_networking: bool,
    password: String,
}

fn main() -> Result<(), c4::Error> {
    let loader = Loader::new(Options {
        sources: vec!["config".into()],
        ..Options::default()
    });

    // trace() carries the warnings next to the merged tree
    let trace = loader.trace()?;
    for warning in &trace.warnings {
        println!("warning: {warning}");
    }

    // load the whole thing dynamically, then type one section of it
    let config: c4::Value = loader.load()?;
    println!("certs = {}", config["CA_default"]["certs"]);
    let mysqld: Mysqld = config["mysqld"].clone().into_typed()?;
    println!("{mysqld:?}");
    Ok(())
}
