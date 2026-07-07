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
