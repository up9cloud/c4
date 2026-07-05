//! env files — `KEY=VALUE` lines, `#` comments, optional `export` prefix,
//! single/double quotes stripped. All values are strings; no variable
//! interpolation. `dot_key` applies to the keys.

use crate::{Options, Result, Value};

pub(crate) fn parse(text: &str, options: &Options) -> Result<Value> {
    let mut root = Value::Object(Default::default());

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((key, value)) = line.split_once('=') else {
            continue; // not a KEY=VALUE line — ignore, like dotenv tools do
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let value = unquote(value.trim());
        super::deep_merge(
            &mut root,
            super::expand_key(key, Value::String(value.to_owned()), options.dot_key),
        );
    }
    Ok(root)
}

/// Strip one pair of matching single or double quotes.
fn unquote(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2
        && (bytes[0] == b'"' || bytes[0] == b'\'')
        && bytes[bytes.len() - 1] == bytes[0]
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}
