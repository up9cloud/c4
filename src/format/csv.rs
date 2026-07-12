//! CSV — the first table-shaped format. This module only lowers the file
//! into a plain row table (`[[key, value, format], …]`); all table semantics
//! (header matching, the format column, dot_key, merging) live in
//! [`super::table`].

use std::path::Path;

use crate::{Error, Options, Result, TableLayout, Value};

pub(crate) fn parse(
    text: &str,
    layout: &TableLayout,
    path: &Path,
    options: &Options,
) -> Result<Value> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());

    let mut rows: Vec<Vec<String>> = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| Error::Parse {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
        rows.push(record.iter().map(str::to_owned).collect());
    }
    super::table::parse(rows, layout, path, options)
}
