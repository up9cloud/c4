//! Strict JSON.

use std::path::Path;

use crate::{Error, Result, Value};

pub(crate) fn parse(text: &str, path: &Path) -> Result<Value> {
    if text.trim().is_empty() {
        return Err(Error::Parse {
            path: path.to_path_buf(),
            message: "empty document — strict json needs a value; to treat empty .json \
                      files as empty, let jsonc read them: formats \
                      (\"jsonc\", [\"json\", \"jsonc\"])"
                .into(),
        });
    }
    let parsed: serde_json::Value = serde_json::from_str(text).map_err(|e| Error::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    Ok(super::from_serde_json(parsed))
}
