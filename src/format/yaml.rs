//! YAML, read with `yaml_serde`; `<<` merge keys are applied.

use std::path::Path;

use crate::{Error, Result, Value};

pub(crate) fn parse(text: &str, path: &Path) -> Result<Value> {
    let error = |e: yaml_serde::Error| Error::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    };
    let mut parsed: yaml_serde::Value = yaml_serde::from_str(text).map_err(error)?;
    parsed.apply_merge().map_err(error)?;
    convert(parsed, path)
}

fn convert(value: yaml_serde::Value, path: &Path) -> Result<Value> {
    Ok(match value {
        yaml_serde::Value::Null => Value::Null,
        yaml_serde::Value::Bool(b) => Value::Bool(b),
        yaml_serde::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(u) = n.as_u64() {
                Value::Uint(u)
            } else {
                Value::Float(n.as_f64().unwrap_or(f64::NAN))
            }
        }
        yaml_serde::Value::String(s) => Value::String(s),
        yaml_serde::Value::Sequence(items) => Value::Array(
            items
                .into_iter()
                .map(|v| convert(v, path))
                .collect::<Result<_>>()?,
        ),
        yaml_serde::Value::Mapping(map) => {
            let mut object = std::collections::BTreeMap::new();
            for (key, value) in map {
                let key = match key {
                    yaml_serde::Value::String(s) => s,
                    yaml_serde::Value::Bool(b) => b.to_string(),
                    yaml_serde::Value::Number(n) => n.to_string(),
                    other => {
                        return Err(Error::Parse {
                            path: path.to_path_buf(),
                            message: format!("unsupported mapping key: {other:?}"),
                        });
                    }
                };
                object.insert(key, convert(value, path)?);
            }
            Value::Object(object)
        }
        yaml_serde::Value::Tagged(tagged) => convert(tagged.value, path)?,
    })
}
