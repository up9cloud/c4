//! JSONC — JSON with comments and trailing commas (a JSON superset).
//! Converts jsonc-parser's AST directly, so this format needs no
//! serde_json dependency.

use std::path::Path;

use jsonc_parser::ast;

use crate::{Error, Result, Value};

pub(crate) fn parse(text: &str, path: &Path) -> Result<Value> {
    let parsed = jsonc_parser::parse_to_ast(text, &Default::default(), &Default::default())
        .map_err(|e| Error::Parse {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
    match parsed.value {
        Some(value) => convert(value, path),
        None => Ok(Value::Null), // empty document
    }
}

// straight from the AST: jsonc-parser's own value type would build a
// SipHash map per object only for c4 to re-sort it into a BTreeMap
fn convert(value: ast::Value<'_>, path: &Path) -> Result<Value> {
    Ok(match value {
        ast::Value::NullKeyword(_) => Value::Null,
        ast::Value::BooleanLit(b) => Value::Bool(b.value),
        ast::Value::StringLit(s) => Value::String(s.value.into_owned()),
        // the parser keeps numbers as raw text
        ast::Value::NumberLit(n) => {
            let raw = n.value;
            if let Ok(i) = raw.parse::<i64>() {
                Value::Int(i)
            } else if let Ok(u) = raw.parse::<u64>() {
                Value::Uint(u)
            } else if let Ok(f) = raw.parse::<f64>() {
                Value::Float(f)
            } else {
                return Err(Error::Parse {
                    path: path.to_path_buf(),
                    message: format!("invalid number literal: {raw}"),
                });
            }
        }
        ast::Value::Array(array) => Value::Array(
            array
                .elements
                .into_iter()
                .map(|v| convert(v, path))
                .collect::<Result<_>>()?,
        ),
        ast::Value::Object(object) => {
            let mut map = std::collections::BTreeMap::new();
            for prop in object.properties {
                // a repeated key: the later one wins, as in the parser's map
                map.insert(prop.name.into_string(), convert(prop.value, path)?);
            }
            Value::Object(map)
        }
    })
}
