//! Shared helpers: every regular case folder is `config/` plus two
//! expectation files — `expect.value.json` (the plain merged result, what
//! `load()` returns) and `expect.debug.log` (the `{:#?}` form of the whole
//! `Trace`, warnings included — what `c4 --trace -f debug` prints).

#![allow(dead_code)] // each test binary uses a subset of these helpers

use serde_json::Value as Json;

pub fn fx(name: &str) -> String {
    format!("tests/fixtures/{name}")
}

fn read(path: String) -> Json {
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The plain merged expectation of a case (expect.value.json).
pub fn expect(case: &str) -> Json {
    read(format!("tests/fixtures/{case}/expect.value.json"))
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

/// Assert a loader's whole trace — merged tree and warnings — against
/// the case's `expect.debug.log`, in the CLI's `--trace -f debug` form.
pub fn assert_trace(case: &str, loader: &c4::Loader) {
    let trace = loader.trace().unwrap();
    // load() merges into a plain value on its own path; it must agree
    assert_eq!(
        c4::Value::from(trace.value.clone()),
        loader.load::<c4::Value>().unwrap(),
        "load() and trace() disagree for case {case}"
    );
    let actual = format!("{trace:#?}\n");
    let path = format!("tests/fixtures/{case}/expect.debug.log");
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert_eq!(actual, expected, "trace mismatch for case {case}");
}

/// Assert both the traced and the plain load of a case's `config/`
/// folder against its two expectation files.
pub fn check(case: &str, mut options: c4::Options) {
    options.sources = vec![format!("tests/fixtures/{case}/config").into()];
    let loader = c4::Loader::new(options);
    assert_trace(case, &loader);
    let plain: Json = loader.load().unwrap();
    assert_eq!(plain, expect(case), "plain output mismatch for case {case}");
}
