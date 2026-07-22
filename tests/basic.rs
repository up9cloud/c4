//! Core loading behavior: mixed formats in one folder, deep merge,
//! filename-order overrides, jsonc syntax, empty/missing folders, struct
//! deserialization, the typed trace tree, `Value` accessors/indexing,
//! and `format_id` derivation.

mod common;

#[allow(unused_imports)] // unused in single-format builds
use common::{base, check, fx, loader};

#[cfg(all(any(feature = "json", feature = "jsonc"), feature = "yaml"))]
#[test]
fn simple_mixed_formats() {
    check("simple", base());
}

#[cfg(any(feature = "json", feature = "jsonc"))]
#[test]
fn deep_merge_in_alphabetic_order() {
    // a.json then b.json: objects merge recursively, arrays/scalars replace
    check("merge_order", base());
}

#[cfg(all(any(feature = "json", feature = "jsonc"), feature = "yaml"))]
#[test]
fn same_basename_merges_in_filename_order() {
    // formats carry no override order: app.json sorts before app.yml, so
    // the yml file simply loads later and wins
    check("precedence", base());
}

#[cfg(feature = "jsonc")]
#[test]
fn jsonc_comments_and_trailing_commas() {
    check("jsonc", base());
}

#[test]
fn empty_folder_is_empty_object() {
    // the folder only holds .gitkeep, whose extension no format claims
    check("empty", base());
}

#[cfg(all(any(feature = "json", feature = "jsonc"), feature = "yaml"))]
#[test]
fn load_takes_a_folder_or_a_file() {
    // an existing file loads as a single file source (single-file sources
    // are unaffected by the keying options / the `tree` default)
    let v: serde_json::Value = c4::load(fx("simple/config/app.json")).unwrap();
    assert_eq!(v, serde_json::json!({ "name": "c4", "port": 8080 }));

    // a folder path — c4::load uses Options::default(), which the `tree`
    // feature flips to keyed loading, so only assert the flat merge when
    // tree is off
    #[cfg(not(feature = "tree"))]
    {
        let v: serde_json::Value = c4::load(fx("simple/config")).unwrap();
        assert_eq!(v, common::expect("simple"));
    }
}

#[test]
fn load_missing_path_is_not_found() {
    let res = c4::load::<c4::Value>(fx("does_not_exist"));
    assert!(matches!(res, Err(c4::Error::NotFound(_))));
}

#[test]
fn missing_folder_is_error() {
    let res = loader(vec![fx("does_not_exist").into()]).load::<c4::Value>();
    assert!(matches!(res, Err(c4::Error::NotFound(_))));
}

#[cfg(all(any(feature = "json", feature = "jsonc"), feature = "yaml"))]
#[test]
fn deserialize_into_struct() {
    #[derive(serde::Deserialize)]
    struct Db {
        host: String,
        port: u16,
    }
    #[derive(serde::Deserialize)]
    struct Cfg {
        name: String,
        port: u16,
        db: Db,
    }

    let cfg: Cfg = loader(vec![fx("simple/config").into()]).load().unwrap();
    assert_eq!(cfg.name, "c4");
    assert_eq!(cfg.port, 8080);
    assert_eq!(cfg.db.host, "localhost");
    assert_eq!(cfg.db.port, 5432);
}

#[cfg(all(any(feature = "json", feature = "jsonc"), feature = "yaml"))]
#[test]
fn trace_returns_typed_tree() {
    // trace() is a real type, not JSON with magic keys — expect.json is
    // only its serialized form
    use c4::{SourceRef, TracedValue, Value};

    let traced = loader(vec![fx("simple/config").into()]).trace().unwrap();

    let TracedValue::Object(root) = traced else {
        panic!("root must be an object");
    };
    let TracedValue::Leaf { value, source } = &root["port"] else {
        panic!("port must be a leaf");
    };
    assert_eq!(*value, Value::Int(8080));
    assert_eq!(
        *source,
        SourceRef::File("tests/fixtures/simple/config/app.json".into())
    );

    let TracedValue::Object(db) = &root["db"] else {
        panic!("db must stay an object in the trace");
    };
    let TracedValue::Leaf { value, source } = &db["host"] else {
        panic!("db.host must be a leaf");
    };
    assert_eq!(*value, Value::String("localhost".into()));
    assert_eq!(
        *source,
        SourceRef::File("tests/fixtures/simple/config/db.yml".into())
    );
}

#[cfg(all(any(feature = "json", feature = "jsonc"), feature = "yaml"))]
#[test]
fn value_accessors_and_indexing() {
    // dynamic access without deserializing: indexing yields Null for
    // missing keys, as_* converts leaves
    let value: c4::Value = loader(vec![fx("simple/config").into()]).load().unwrap();
    assert_eq!(value["db"]["host"].as_str(), Some("localhost"));
    assert_eq!(value["db"]["port"].as_u64(), Some(5432));
    assert_eq!(value["port"].as_i64(), Some(8080));
    assert_eq!(value["port"].as_f64(), Some(8080.0));
    assert!(value["no_such_key"]["nested"].is_null());
    assert_eq!(value["name"].as_bool(), None);
    assert_eq!(
        value.get("db").and_then(|db| db.get("host")),
        Some(&c4::Value::String("localhost".into()))
    );
    assert_eq!(value["db"].as_object().map(|o| o.len()), Some(2));
}

#[test]
fn format_ids_derive_from_values() {
    use c4::Value;
    assert_eq!(Value::Null.format_id(), "null");
    assert_eq!(Value::Bool(true).format_id(), "bool");
    assert_eq!(Value::Int(1).format_id(), "i64");
    assert_eq!(Value::Uint(1).format_id(), "u64");
    assert_eq!(Value::Int128(1).format_id(), "i128");
    assert_eq!(Value::Uint128(1).format_id(), "u128");
    assert_eq!(Value::Float(1.5).format_id(), "f64");
    assert_eq!(Value::String("x".into()).format_id(), "str");
    assert_eq!(
        Value::Array(vec![Value::Int(1), Value::Int(2)]).format_id(),
        "arr:i64"
    );
    // mixed, nested and empty arrays are just "arr"
    assert_eq!(
        Value::Array(vec![Value::Int(1), Value::String("x".into())]).format_id(),
        "arr"
    );
    assert_eq!(Value::Array(vec![]).format_id(), "arr");
}

#[test]
fn large_128_bit_values_serialize_as_strings() {
    use c4::Value;
    // fits u64 → JSON number; beyond u64 has no JSON number form, so it
    // serializes as its decimal string (a lossy round trip)
    assert_eq!(
        serde_json::to_value(Value::Int128(42)).unwrap(),
        serde_json::json!(42)
    );
    assert_eq!(
        serde_json::to_value(Value::Uint128(u128::MAX)).unwrap(),
        serde_json::json!("340282366920938463463374607431768211455")
    );
    // 128-bit still converts numerically via the accessors
    assert_eq!(Value::Uint128(7).as_u64(), Some(7));
    assert_eq!(Value::Int128(i128::MAX).as_i128(), Some(i128::MAX));
}
