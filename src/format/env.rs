//! env files — `KEY=VALUE` lines, `#` comments, optional `export` prefix,
//! dotenv-style values: a quoted value ends at its closing quote, and in
//! an unquoted one `#` starts a comment only after whitespace. All values
//! are strings; no variable interpolation. `dot_key` applies to the keys.

use std::path::Path;

use crate::{Error, Options, Result, Value};

pub(crate) fn parse(text: &str, path: &Path, options: &Options) -> Result<Value> {
    let mut root = Value::Object(Default::default());

    for (index, line) in text.lines().enumerate() {
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
        super::insert_key(
            &mut root,
            key,
            Value::String(value_text(value).to_owned()),
            options.dot_key,
        )
        .map_err(|message| Error::Parse {
            path: path.to_path_buf(),
            message: format!("line {}: {message}", index + 1),
        })?;
    }
    Ok(root)
}

fn value_text(raw: &str) -> &str {
    let trimmed = raw.trim_start();
    if let Some(quote @ ('"' | '\'')) = trimmed.chars().next()
        && let Some(end) = trimmed[1..].find(quote)
    {
        return &trimmed[1..1 + end];
    }
    let end = raw
        .char_indices()
        .find(|&(at, c)| c == '#' && raw[..at].ends_with(char::is_whitespace))
        .map_or(raw.len(), |(at, _)| at);
    raw[..end].trim()
}
