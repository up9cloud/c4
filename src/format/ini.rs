//! INI — `key = value` lines and `[section]` headers. No value types:
//! everything is a string; sections nest one level.

use std::collections::BTreeMap;
use std::path::Path;

use crate::{Error, Result, Value};

pub(crate) fn parse(text: &str, path: &Path) -> Result<Value> {
    let mut root: BTreeMap<String, Value> = BTreeMap::new();
    let mut section: Option<String> = None;

    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = Some(name.trim().to_owned());
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(Error::Parse {
                path: path.to_path_buf(),
                message: format!("line {}: expected 'key = value' or '[section]'", number + 1),
            });
        };
        let entry = (
            key.trim().to_owned(),
            Value::String(value.trim().to_owned()),
        );
        match &section {
            None => {
                root.insert(entry.0, entry.1);
            }
            Some(name) => {
                let slot = root
                    .entry(name.clone())
                    .or_insert_with(|| Value::Object(BTreeMap::new()));
                if let Value::Object(map) = slot {
                    map.insert(entry.0, entry.1);
                }
            }
        }
    }
    Ok(Value::Object(root))
}
