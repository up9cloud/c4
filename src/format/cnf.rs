//! cnf — the INI-like option files with no spec (`my.cnf`, `openssl.cnf`).
//! Unlike the strict ini parser, this one accepts the union of those
//! dialects, following MySQL where they conflict; behavior is in the
//! crate docs, the reasons in CLAUDE.md.

use std::collections::BTreeMap;
use std::path::Path;

use crate::{Error, Result, Value};

pub(crate) fn parse(text: &str, path: &Path, warnings: &mut Vec<String>) -> Result<Value> {
    let error = |number: usize, message: String| Error::Parse {
        path: path.to_path_buf(),
        message: format!("line {number}: {message}"),
    };
    let mut root: BTreeMap<String, Value> = BTreeMap::new();
    let mut section: Option<String> = None;

    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with(['#', ';', '%']) {
            continue;
        }

        if let Some(rest) = line.strip_prefix('[') {
            let Some((name, after)) = rest.split_once(']') else {
                return Err(error(number, "missing ']' in section header".into()));
            };
            let name = name.trim();
            if name.is_empty() {
                return Err(error(number, "empty section name".into()));
            }
            let after = after.trim();
            if !after.is_empty() && !after.starts_with(['#', ';']) {
                return Err(error(
                    number,
                    format!("unexpected '{after}' after section header"),
                ));
            }
            let slot = root.entry(name.to_owned()).or_insert(Value::Null);
            if !matches!(slot, Value::Object(_)) {
                *slot = Value::Object(BTreeMap::new());
            }
            section = Some(name.to_owned());
            continue;
        }

        if is_directive(line) {
            let directive = line.split_once('#').map_or(line, |(d, _)| d).trim_end();
            warnings.push(format!(
                "line {number}: ignored directive '{directive}' \
                 (c4 does not process directives; add included files as sources)"
            ));
            continue;
        }

        let (key, value) = match line.find(['=', '#']) {
            Some(at) if line[at..].starts_with('=') => {
                (line[..at].trim(), Value::String(value(&line[at + 1..])))
            }
            // a bare key: a MySQL flag such as `skip-networking`
            Some(at) => (line[..at].trim(), Value::Bool(true)),
            None => (line, Value::Bool(true)),
        };
        if key.is_empty() {
            return Err(error(number, "empty key".into()));
        }
        let target = match &section {
            None => &mut root,
            Some(name) => match root.get_mut(name) {
                Some(Value::Object(map)) => map,
                _ => unreachable!("a section header always leaves an object"),
            },
        };
        target.insert(key.to_owned(), value);
    }
    Ok(Value::Object(root))
}

/// MySQL `!include`/`!includedir`/any `!…`, openssl `.include`/`.pragma`.
fn is_directive(line: &str) -> bool {
    if line.starts_with('!') {
        return true;
    }
    [".include", ".pragma"].iter().any(|name| {
        line.strip_prefix(name).is_some_and(|rest| {
            rest.is_empty() || rest.starts_with(|c: char| c == '=' || c.is_whitespace())
        })
    })
}

/// MySQL's order: cut the comment, trim, unquote, unescape.
fn value(raw: &str) -> String {
    let raw = raw[..comment_start(raw)].trim();
    let bytes = raw.as_bytes();
    let quoted =
        bytes.len() >= 2 && matches!(bytes[0], b'"' | b'\'') && bytes[bytes.len() - 1] == bytes[0];
    let raw = if quoted { &raw[1..raw.len() - 1] } else { raw };
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            escape(&mut out, chars.next());
        } else {
            out.push(c);
        }
    }
    out
}

/// Where an unquoted `#` starts the comment. Quote regions open anywhere
/// in the value, and an unclosed one runs to the end of the line.
fn comment_start(raw: &str) -> usize {
    let mut quote = None;
    let mut escaped = false;
    for (at, c) in raw.char_indices() {
        match quote {
            None if c == '#' => return at,
            None if c == '"' || c == '\'' => quote = Some(c),
            None => {}
            Some(_) if escaped => escaped = false,
            Some(_) if c == '\\' => escaped = true,
            Some(q) if c == q => quote = None,
            Some(_) => {}
        }
    }
    raw.len()
}

fn escape(out: &mut String, next: Option<char>) {
    match next {
        Some('b') => out.push('\u{8}'),
        Some('t') => out.push('\t'),
        Some('n') => out.push('\n'),
        Some('r') => out.push('\r'),
        Some('s') => out.push(' '),
        Some('\\') => out.push('\\'),
        // MySQL keeps the backslash of an unknown escape (`C:\Program Files`)
        Some(c) => {
            out.push('\\');
            out.push(c);
        }
        None => out.push('\\'),
    }
}
