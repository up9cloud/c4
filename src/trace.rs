//! Provenance: [`SourceRef`] labels and the [`TracedValue`] tree that
//! `Loader::trace` returns. Serializes to `$id`-tagged JSON leaves — a
//! debug/test aid, never config data.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::Value;

/// Where a traced value came from.
///
/// Source labels exist for debugging and testing — they are not config
/// data, and no config semantics should be built on top of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceRef {
    /// The file path as the loader saw it (source path joined with the
    /// relative path inside a folder source).
    File(PathBuf),
    /// A [`Source::String`] entry, by its index in the sources list.
    String(usize),
    /// A [`Source::value`] entry, by its index in the sources list.
    Value(usize),
}

/// The result of [`Loader::trace`]: the merged tree with per-leaf
/// provenance. Objects recurse; every non-object value (including whole
/// arrays) is a [`TracedValue::Leaf`].
///
/// Serializes to JSON as
/// `{ "$id": "Leaf", "value": …, "source": …, "format": … }` leaves —
/// the shape used by the CLI `--trace` output and the test fixtures'
/// expect.json. `$id` tags the node kind (an object node is a plain
/// map), `source` is the file path (`SourceRef::String(i)` prints as
/// `"string:<i>"`), `format` is the value's [`Value::format_id`].
#[derive(Debug, Clone, PartialEq)]
pub enum TracedValue {
    Leaf { value: Value, source: SourceRef },
    Object(BTreeMap<String, TracedValue>),
}

impl SourceRef {
    /// The label used in serialized traces: the file path, or
    /// `string:<index>` for string sources.
    fn label(&self) -> String {
        match self {
            SourceRef::File(path) => path.to_string_lossy().into_owned(),
            SourceRef::String(index) => format!("string:{index}"),
            SourceRef::Value(index) => format!("value:{index}"),
        }
    }
}

impl serde::Serialize for TracedValue {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        match self {
            TracedValue::Leaf { value, source } => {
                // "$id" tags the node kind: a plain config object could
                // itself have value/source/format keys, and the tag makes
                // json trace output self-describing for debugging
                let mut map = serializer.serialize_map(Some(4))?;
                map.serialize_entry("$id", "Leaf")?;
                map.serialize_entry("value", value)?;
                map.serialize_entry("source", &source.label())?;
                map.serialize_entry("format", &value.format_id())?;
                map.end()
            }
            TracedValue::Object(entries) => serializer.collect_map(entries),
        }
    }
}
