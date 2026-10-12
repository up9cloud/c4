//! env files — `KEY=VALUE` lines, `#` comments, optional `export` prefix,
//! dotenv-style values: a quoted value ends at its closing quote (double
//! quotes decode `\n \r \t \" \\`), and in an unquoted one `#` starts a
//! comment only after whitespace. All values are strings; no variable
//! interpolation. `dot_key` applies to the keys.

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
            Value::String(value_text(value)),
            options.dot_key,
        )
        .map_err(|message| Error::Parse {
            path: path.to_path_buf(),
            message: format!("line {}: {message}", index + 1),
        })?;
    }
    Ok(root)
}

fn value_text(raw: &str) -> String {
    let trimmed = raw.trim_start();
    match trimmed.chars().next() {
        Some('"') => {
            if let Some(text) = double_quoted(&trimmed[1..]) {
                return text;
            }
        }
        Some('\'') => {
            if let Some(end) = trimmed[1..].find('\'') {
                return trimmed[1..1 + end].to_owned();
            }
        }
        _ => {}
    }
    let end = raw
        .char_indices()
        .find(|&(at, c)| c == '#' && raw[..at].ends_with(char::is_whitespace))
        .map_or(raw.len(), |(at, _)| at);
    raw[..end].trim().to_owned()
}

/// The text up to the closing `"`, with `\n \r \t \" \\` decoded (any
/// other `\X` stays as written); `None` when the quote never closes.
fn double_quoted(rest: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                c @ ('"' | '\\') => out.push(c),
                c => {
                    out.push('\\');
                    out.push(c);
                }
            },
            c => out.push(c),
        }
    }
    None
}
