//! Shared helpers: every regular case folder is `config/` plus two
//! expectation files — `expect.json` (the plain merged result, what
//! `load()` returns) and `expect.debug.json` (the serialized trace:
//! `$id`-tagged leaves with value + source + format, what `trace()`
//! serializes to).

#![allow(dead_code)] // each test binary uses a subset of these helpers

use serde_json::Value as Json;

pub fn fx(name: &str) -> String {
    format!("tests/fixtures/{name}")
}

fn read(path: String) -> Json {
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The plain merged expectation of a case (expect.json).
pub fn expect(case: &str) -> Json {
    read(format!("tests/fixtures/{case}/expect.json"))
}

/// The traced expectation of a case (expect.debug.json).
pub fn expect_debug(case: &str) -> Json {
    read(format!("tests/fixtures/{case}/expect.debug.json"))
}

/// An explicitly **flat** baseline, independent of the `tree` feature
/// (which flips `Options::default()`'s folder/file/sheet keying on and
/// scans every depth). Fixture cases build on this so their expectations
/// hold under every feature combination — including `--all-features`.
pub fn base() -> c4::Options {
    c4::Options {
        filename_as_key: false,
        dirname_as_key: false,
        sheetname_as_key: false,
        dir_depth: 1,
        ..c4::Options::default()
    }
}

/// A loader over the given sources with otherwise-flat options.
pub fn loader(sources: Vec<c4::Source>) -> c4::Loader {
    c4::Loader::new(c4::Options { sources, ..base() })
}

/// Assert both the traced and the plain load of a case's `config/`
/// folder against its two expectation files.
pub fn check(case: &str, mut options: c4::Options) {
    options.sources = vec![format!("tests/fixtures/{case}/config").into()];
    let loader = c4::Loader::new(options);

    let traced = loader.trace().unwrap();
    let traced_json = serde_json::to_value(&traced).unwrap();
    assert_eq!(
        traced_json,
        expect_debug(case),
        "traced output mismatch for case {case}"
    );

    let plain: Json = loader.load().unwrap();
    assert_eq!(plain, expect(case), "plain output mismatch for case {case}");
}
