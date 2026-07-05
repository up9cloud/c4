//! YAML.

use std::path::Path;

use crate::{Error, Result, Value};

pub(crate) fn parse(text: &str, path: &Path) -> Result<Value> {
    let parsed: serde_yaml::Value = serde_yaml::from_str(text).map_err(|e| Error::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    convert(parsed, path)
}

fn convert(value: serde_yaml::Value, path: &Path) -> Result<Value> {
    Ok(match value {
        serde_yaml::Value::Null => Value::Null,
        serde_yaml::Value::Bool(b) => Value::Bool(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(u) = n.as_u64() {
                Value::Uint(u)
            } else {
                Value::Float(n.as_f64().unwrap_or(f64::NAN))
            }
        }
        serde_yaml::Value::String(s) => Value::String(s),
        serde_yaml::Value::Sequence(items) => Value::Array(
            items
                .into_iter()
                .map(|v| convert(v, path))
                .collect::<Result<_>>()?,
        ),
        serde_yaml::Value::Mapping(map) => {
            let mut object = std::collections::BTreeMap::new();
            for (key, value) in map {
                let key = match key {
                    serde_yaml::Value::String(s) => s,
                    serde_yaml::Value::Bool(b) => b.to_string(),
                    serde_yaml::Value::Number(n) => n.to_string(),
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
        serde_yaml::Value::Tagged(tagged) => convert(tagged.value, path)?,
    })
}
