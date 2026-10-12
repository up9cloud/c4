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

#[test]
fn strings_coerce_into_typed_fields() {
    // ini/env/cnf values are strings; a typed field parses them the way
    // the table type ids do (a `(value,)` source keeps this feature-free)
    #[derive(serde::Deserialize, Debug, PartialEq)]
    struct Cfg {
        port: u16,
        debug: bool,
        verbose: bool,
        ratio: f64,
        big: i128,
        level: Option<u8>,
        name: String,
    }
    let cfg: Cfg = loader(vec![
        (serde_json::json!({
            "port": "8080", "debug": "true", "verbose": "yes", "ratio": "0.5",
            "big": "170141183460469231731687303715884105727", "level": "3",
            "name": "8080",
        }),)
            .into(),
    ])
    .load()
    .unwrap();
    assert_eq!(
        cfg,
        Cfg {
            port: 8080,
            debug: true,
            verbose: true,
            ratio: 0.5,
            big: i128::MAX,
            level: Some(3),
            name: "8080".into(),
        }
    );

    #[derive(serde::Deserialize, Debug)]
    #[allow(dead_code)]
    struct Port {
        port: u16,
    }
    let err = loader(vec![(serde_json::json!({"port": "70000"}),).into()])
        .load::<Port>()
        .unwrap_err();
    assert!(
        err.to_string().contains("'70000' is not a valid u16"),
        "{err}"
    );
    // an untyped target keeps the string
    let value: c4::Value = loader(vec![(serde_json::json!({"port": "8080"}),).into()])
        .load()
        .unwrap();
    assert_eq!(value["port"].as_str(), Some("8080"));
}

#[test]
fn value_conversions_comparisons_and_display() {
    use c4::Value;
    use std::collections::BTreeMap;

    // From: build values the short way
    assert_eq!(Value::from("c4"), Value::String("c4".into()));
    assert_eq!(Value::from(String::from("c4")), Value::String("c4".into()));
    assert_eq!(Value::from(true), Value::Bool(true));
    assert_eq!(Value::from(8080_u16), Value::Uint(8080));
    assert_eq!(Value::from(-3_i32), Value::Int(-3));
    assert_eq!(Value::from(i128::MAX), Value::Int128(i128::MAX));
    assert_eq!(Value::from(1.5_f64), Value::Float(1.5));
    assert_eq!(Value::from(None::<u8>), Value::Null);
    assert_eq!(Value::from(Some("x")), Value::String("x".into()));
    assert_eq!(
        Value::from(vec![1_i64, 2]),
        Value::Array(vec![Value::Int(1), Value::Int(2)])
    );
    let object = Value::from(BTreeMap::from([("port".to_owned(), Value::from(8080_u16))]));
    assert_eq!(object["port"], Value::Uint(8080));

    // comparisons follow the as_* accessors, both directions
    assert!(object["port"] == 8080);
    assert!(8080_u64 == object["port"]);
    assert!(object["port"] == 8080.0);
    assert!(object["port"] != 80);
    let name = Value::from("c4");
    let owned = String::from("c4");
    assert!(name == "c4");
    assert!("c4" == name);
    assert!(name == owned);
    let date = Value::DateTime("2024-01-02".into());
    assert!(date == "2024-01-02"); // typed text too
    let flag = Value::from(true);
    assert!(flag == true);
    assert!(Value::Null != "null");

    // Display: compact JSON
    let value = Value::from(BTreeMap::from([
        (
            "a".to_owned(),
            Value::from(vec![Value::from(1_i64), Value::Null]),
        ),
        ("b".to_owned(), Value::from("say \"hi\"\n")),
        ("c".to_owned(), Value::Float(f64::NAN)),
        ("d".to_owned(), Value::Ipv4("10.0.0.1".into())),
    ]));
    assert_eq!(
        value.to_string(),
        r#"{"a":[1,null],"b":"say \"hi\"\n","c":null,"d":"10.0.0.1"}"#
    );
    assert_eq!(Value::Float(1.0).to_string(), "1.0");
}

#[test]
fn into_typed_converts_a_loaded_value() {
    #[derive(serde::Deserialize, Debug, PartialEq)]
    struct Db {
        host: String,
        port: u16,
    }
    let config: c4::Value = loader(vec![
        (serde_json::json!({"db": {"host": "a", "port": "5432"}, "port": 8080}),).into(),
    ])
    .load()
    .unwrap();
    // a section: index, clone, convert — same coercions as load
    let db: Db = config["db"].clone().into_typed().unwrap();
    assert_eq!(
        db,
        Db {
            host: "a".into(),
            port: 5432
        }
    );
    let port: u16 = config["port"].clone().into_typed().unwrap();
    assert_eq!(port, 8080);
    // a failure names the key path inside the converted value
    let err = config
        .into_typed::<std::collections::BTreeMap<String, Db>>()
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "deserialize error: port: invalid type: integer `8080`, expected struct Db"
    );
}

#[test]
fn an_empty_value_says_how_to_leave_a_field_unset() {
    #[derive(serde::Deserialize, Debug)]
    #[allow(dead_code)]
    struct Cfg {
        port: Option<u16>,
    }
    let err = loader(vec![(serde_json::json!({"port": ""}),).into()])
        .load::<Cfg>()
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "deserialize error: port: the value is empty, which is not a valid u16 \
         — to leave the field unset, remove the line (an Option field is then None)"
    );
}

#[test]
fn load_keeps_typed_variants() {
    // load::<Value>() hands the merged tree over as is — no serde round
    // trip that would flatten typed variants into strings
    let typed = c4::CustomFormat::new("typed", ["typed"], |_, _, _| {
        Ok(c4::Value::Object(std::collections::BTreeMap::from([
            (
                "when".to_owned(),
                c4::Value::DateTime("2024-01-02T03:04:05Z".into()),
            ),
            ("ip".to_owned(), c4::Value::Ipv4("10.0.0.1".into())),
        ])))
    });
    let loader = loader(vec![(typed, "").into()]);
    let value: c4::Value = loader.load().unwrap();
    assert_eq!(
        value["when"],
        c4::Value::DateTime("2024-01-02T03:04:05Z".into())
    );
    assert_eq!(value["when"].format_id(), "dt");
    assert_eq!(value["ip"], c4::Value::Ipv4("10.0.0.1".into()));
    assert_eq!(value["ip"].as_str(), Some("10.0.0.1"));
    assert_eq!(value["ip"].as_ip(), Some([10, 0, 0, 1].into()));

    // a Value inside a typed struct keeps its variants too
    #[derive(serde::Deserialize)]
    struct Cfg {
        when: c4::Value,
        ip: Option<c4::Value>,
    }
    let cfg: Cfg = loader.load().unwrap();
    assert_eq!(cfg.when.format_id(), "dt");
    assert_eq!(cfg.ip.unwrap().format_id(), "ipv4");

    // other deserializers still build a Value the usual way
    let from_json: c4::Value = serde_json::from_str(r#"{"a": [1, "x", null]}"#).unwrap();
    assert_eq!(from_json["a"][0].as_i64(), Some(1));
    assert_eq!(from_json["a"][1].as_str(), Some("x"));
}

#[test]
fn numbers_and_bools_coerce_into_string_fields() {
    #[derive(serde::Deserialize, Debug, PartialEq)]
    struct Cfg {
        password: String,
        version: String,
        ratio: String,
        flag: String,
        tag: Option<String>,
    }
    let cfg: Cfg = loader(vec![
        (serde_json::json!({
            "password": 123456, "version": 1.0, "ratio": 0.5, "flag": true, "tag": -3,
        }),)
            .into(),
    ])
    .load()
    .unwrap();
    assert_eq!(
        cfg,
        Cfg {
            password: "123456".into(),
            version: "1.0".into(),
            ratio: "0.5".into(),
            flag: "true".into(),
            tag: Some("-3".into()),
        }
    );
}

#[test]
fn deserialize_errors_name_the_key_path() {
    #[derive(serde::Deserialize, Debug)]
    #[allow(dead_code)]
    struct Item {
        id: u8,
    }
    #[derive(serde::Deserialize, Debug)]
    #[allow(dead_code)]
    struct Db {
        port: u16,
    }
    #[derive(serde::Deserialize, Debug)]
    #[allow(dead_code)]
    struct Cfg {
        db: Db,
        items: Vec<Item>,
    }
    let load = |json: serde_json::Value| {
        loader(vec![(json,).into()])
            .load::<Cfg>()
            .unwrap_err()
            .to_string()
    };
    let err = load(serde_json::json!({"db": {"port": "x"}, "items": []}));
    assert_eq!(err, "deserialize error: db.port: 'x' is not a valid u16");
    let err = load(serde_json::json!({"db": {"port": 1}, "items": [{"id": 1}, {"id": 300}]}));
    assert!(err.starts_with("deserialize error: items[1].id: "), "{err}");
    let err = load(serde_json::json!({"db": {}, "items": []}));
    assert_eq!(err, "deserialize error: db: missing field `port`");
}

#[cfg(all(unix, any(feature = "json", feature = "jsonc")))]
#[test]
fn io_errors_name_the_path() {
    let dir = std::env::temp_dir().join(format!("c4-io-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // a dangling symlink with a claimed extension fails to read
    let link = dir.join("dead.json");
    std::os::unix::fs::symlink("/nonexistent/c4", &link).unwrap();
    let err = loader(vec![dir.clone().into()])
        .load::<c4::Value>()
        .unwrap_err();
    let _ = std::fs::remove_dir_all(&dir);
    let c4::Error::Io { path, source } = &err else {
        panic!("expected Error::Io, got {err:?}");
    };
    assert_eq!(path, &link);
    assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
    assert!(
        err.to_string().starts_with(&link.display().to_string()),
        "{err}"
    );
    assert!(std::error::Error::source(&err).is_some());
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
    // trace() is a real type, not JSON with magic keys — the `$id` JSON
    // is only its serialized form
    use c4::{SourceRef, TracedValue, Value};

    let traced = loader(vec![fx("simple/config").into()])
        .trace()
        .unwrap()
        .value;

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
