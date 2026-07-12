//! Source handling: single file (str and Path), multiple folders/files
//! in precedence order, and in-code string / typed-value overrides with
//! provenance.

mod common;

#[allow(unused_imports)] // unused in single-format builds
use std::path::Path;

#[allow(unused_imports)] // unused in single-format builds
use common::{expect, expect_debug, fx, loader};
#[allow(unused_imports)] // unused in single-format builds
use serde_json::{Value as Json, json};

#[cfg(any(feature = "json", feature = "jsonc"))]
#[test]
fn single_file_source_accepts_path_types() {
    // &Path works the same as &str (anything Into<PathBuf>)
    let v: Json = loader(vec![
        Path::new("tests/fixtures/simple/config/app.json").into(),
    ])
    .load()
    .unwrap();
    assert_eq!(v, json!({ "name": "c4", "port": 8080 }));
}

#[cfg(any(feature = "json", feature = "jsonc"))]
#[test]
fn path_source_loads_folder_or_file() {
    // one path source — folder vs single file decided at load time
    let folder: Json = loader(vec![fx("simple/config").into()]).load().unwrap();
    assert_eq!(folder["name"], json!("c4"));

    let file: Json = loader(vec![fx("simple/config/app.json").into()])
        .load()
        .unwrap();
    assert_eq!(file, json!({ "name": "c4", "port": 8080 }));
}

#[cfg(all(any(feature = "json", feature = "jsonc"), feature = "yaml"))]
#[test]
fn later_sources_override_earlier() {
    let loader = loader(vec![
        fx("multi_sources/base").into(),
        fx("multi_sources/override").into(),
        fx("multi_sources/local.json").into(),
    ]);

    let traced_json = serde_json::to_value(loader.trace().unwrap()).unwrap();
    assert_eq!(traced_json, expect_debug("multi_sources"));
    let plain: Json = loader.load().unwrap();
    assert_eq!(plain, expect("multi_sources"));
}

#[cfg(all(feature = "jsonc", feature = "yaml"))]
#[test]
fn string_source_overrides_files() {
    let loader = loader(vec![
        fx("simple/config").into(),
        (c4::Format::Jsonc, r#"{ "port": 1 }"#).into(),
    ]);

    // string sources serialize in traces as "string:<index in sources>"
    let traced_json = serde_json::to_value(loader.trace().unwrap()).unwrap();
    assert_eq!(
        traced_json,
        json!({
            "name": { "$id": "Leaf", "value": "c4", "source": "tests/fixtures/simple/config/app.json", "format": "str" },
            "port": { "$id": "Leaf", "value": 1, "source": "string:1", "format": "i64" },
            "db": {
                "host": { "$id": "Leaf", "value": "localhost", "source": "tests/fixtures/simple/config/db.yml", "format": "str" },
                "port": { "$id": "Leaf", "value": 5432, "source": "tests/fixtures/simple/config/db.yml", "format": "i64" }
            }
        })
    );

    let plain: Json = loader.load().unwrap();
    assert_eq!(
        plain,
        json!({
            "name": "c4",
            "port": 1,
            "db": { "host": "localhost", "port": 5432 }
        })
    );
}

#[cfg(all(feature = "jsonc", feature = "yaml"))]
#[test]
fn value_source_overrides_files() {
    #[derive(serde::Serialize)]
    struct Overrides {
        port: u16,
        db: Db,
    }
    #[derive(serde::Serialize)]
    struct Db {
        host: &'static str,
    }

    let loader = loader(vec![
        fx("simple/config").into(),
        (Overrides {
            port: 1,
            db: Db { host: "prod" },
        },)
            .into(),
    ]);

    // typed sources trace as "value:<index in sources>"
    let traced_json = serde_json::to_value(loader.trace().unwrap()).unwrap();
    assert_eq!(
        traced_json["port"],
        json!({ "$id": "Leaf", "value": 1, "source": "value:1", "format": "i64" })
    );
    assert_eq!(
        traced_json["db"]["host"],
        json!({ "$id": "Leaf", "value": "prod", "source": "value:1", "format": "str" })
    );
    // untouched keys keep their file provenance
    assert_eq!(traced_json["db"]["port"]["value"], json!(5432));
}

#[test]
fn value_source_serde_shapes() {
    // enums follow serde conventions; None roots contribute nothing
    #[derive(serde::Serialize)]
    enum Mode {
        Fast,
    }
    #[derive(serde::Serialize)]
    enum Backend {
        Postgres { host: &'static str },
    }
    let v: c4::Value = loader(vec![
        (std::collections::BTreeMap::from([("mode", Mode::Fast)]),).into(),
        (std::collections::BTreeMap::from([(
            "backend",
            Backend::Postgres { host: "db" },
        )]),)
            .into(),
        (None::<bool>,).into(),
    ])
    .load()
    .unwrap();
    assert_eq!(v["mode"].as_str(), Some("Fast"));
    // data-carrying variants become { variant: … }
    assert_eq!(v["backend"]["Postgres"]["host"].as_str(), Some("db"));

    // non-string map keys fail at load time, not at construction
    let res =
        loader(vec![(std::collections::BTreeMap::from([(1, 2)]),).into()]).load::<c4::Value>();
    assert!(matches!(res, Err(c4::Error::Parse { .. })));
}

#[test]
fn table_layout_converts_from_id() {
    use c4::TableLayout;
    assert_eq!(TableLayout::from("kv"), TableLayout::Kv);
    assert_eq!(TableLayout::from("kvf"), TableLayout::Kv);
    assert_eq!(TableLayout::from("db"), TableLayout::Db);
    assert_eq!(TableLayout::from_id("db_auto"), None); // removed on purpose
    assert_eq!(TableLayout::default(), TableLayout::Kv);
}

#[test]
fn table_source_requires_a_table_format() {
    // toml is not a table format — the error is format-independent
    let err = loader(vec![
        (
            c4::Format::Toml,
            "tests/fixtures/simple/config/app.json",
            "db",
        )
            .into(),
    ])
    .load::<c4::Value>()
    .unwrap_err();
    assert!(matches!(err, c4::Error::Parse { .. }));
}

#[test]
fn table_source_must_be_a_file() {
    // a folder (or csv text passed as the path) is Error::Parse with a
    // hint, not NotFound — the common mistake is passing in-code text
    let err = loader(vec![
        (c4::Format::Csv, "tests/fixtures/simple/config", "db").into(),
    ])
    .load::<c4::Value>()
    .unwrap_err();
    let c4::Error::Parse { message, .. } = err else {
        panic!("expected Parse, got {err:?}");
    };
    assert!(message.contains("string source"), "hint missing: {message}");
}

#[test]
fn three_tuple_third_is_always_a_layout() {
    // the 3-tuple never names a sheet — its third element is a layout
    // id string or a TableLayout/CustomLayout value
    let c4::Source::Table { sheet, layout, .. } =
        c4::Source::from((c4::Format::Csv, "x.csv", "db"))
    else {
        panic!("expected a table source");
    };
    assert_eq!(sheet, None);
    assert_eq!(layout, c4::TableLayout::Db);

    let c4::Source::Table { sheet, layout, .. } =
        c4::Source::from((c4::Format::Excel, "x.xlsx", c4::TableLayout::Kv))
    else {
        panic!("expected a table source");
    };
    assert_eq!(sheet, None);
    assert_eq!(layout, c4::TableLayout::Kv);
}

#[test]
#[should_panic(expected = "unknown table layout id")]
fn three_tuple_with_a_sheet_name_panics() {
    // sheets go in the 4-tuple; a non-layout string in the 3-tuple is a
    // config-time mistake
    let _: c4::Source = (c4::Format::Excel, "x.xlsx", "items").into();
}

#[test]
#[should_panic(expected = "not a table format")]
fn formats_layout_on_a_non_table_format_panics() {
    // (yaml, ["yml"], "db") is a config-time mistake — panic, like
    // unknown ids
    let _: c4::FormatSpec = (c4::Format::Yaml, ["yml"], "db").into();
}
