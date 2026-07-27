//! Options behavior: scan depth (`dir_depth`), folder/file keying
//! (`dirname_as_key`/`filename_as_key`), load order, dot_key, case
//! sensitivity, the tree-shaped loading (folders/files as keys,
//! including order-driven key collisions and extension handling) and the
//! commented names/keys options. The
//! `tree` feature only flips `Options::default()`; the keying itself
//! works on any build, so these use an explicit flat `base()`.

mod common;

#[allow(unused_imports)] // unused in single-format builds
use common::{base, check, loader};

#[test]
fn order_converts_from_id() {
    use c4::Order;
    // `Options { order: "alphabetic".into(), .. }` — ids accept `-`/`_`
    assert_eq!(Order::from("alphabetic"), Order::Alphabetic);
    assert_eq!(Order::from("reverse-alphabetic"), Order::ReverseAlphabetic);
    assert_eq!(Order::from("reverse"), Order::ReverseAlphabetic);
    assert_eq!(Order::from("folders_first"), Order::FoldersFirstAlphabetic);
    assert_eq!(Order::from("default"), Order::FoldersFirstAlphabetic);
    assert_eq!(Order::from_id("nope"), None);
    // each order round-trips through its canonical id
    for o in [
        Order::FoldersFirstAlphabetic,
        Order::Alphabetic,
        Order::ReverseAlphabetic,
    ] {
        assert_eq!(Order::from(o.id()), o);
    }
}

// the `tree` feature is a default-preset: it flips the keying on and
// scans every depth. The fields themselves are always available.
#[cfg(feature = "tree")]
#[test]
fn tree_feature_flips_defaults() {
    let d = c4::Options::default();
    assert!(d.filename_as_key && d.dirname_as_key && d.sheetname_as_key);
    assert_eq!(d.dir_depth, -1);
}

#[cfg(not(feature = "tree"))]
#[test]
fn default_is_flat_without_tree() {
    let d = c4::Options::default();
    assert!(!d.filename_as_key && !d.dirname_as_key && !d.sheetname_as_key);
    assert_eq!(d.dir_depth, 1);
}

#[cfg(feature = "yaml")]
#[test]
fn dir_depth_zero_skips_subdirectories() {
    // dir_depth: 0 reads only the folder's own files
    let options = c4::Options {
        dir_depth: 0,
        ..base()
    };
    check("recursive_off", options);
}

#[cfg(feature = "yaml")]
#[test]
fn dirname_as_key_nests_subdirectory() {
    // each subfolder becomes a key (dir_depth default 1 reaches the sub);
    // keying works without the `tree` feature
    let options = c4::Options {
        dirname_as_key: true,
        ..base()
    };
    check("recursive_nest", options);
}

#[cfg(feature = "yaml")]
#[test]
fn depth_scan_flattens_paths() {
    // no keying: subfolder files just merge in flat (default dir_depth 1
    // already reaches one level; -1 would reach every level)
    let options = c4::Options {
        dir_depth: -1,
        ..base()
    };
    check("recursive_flat", options);
}

#[cfg(feature = "yaml")]
#[test]
fn folders_load_before_files_by_default() {
    // Order::FoldersFirstAlphabetic (default): z_sub/ loads before a.yml
    // even though "a.yml" sorts first, so the top-level file wins
    let options = c4::Options {
        dir_depth: -1,
        ..base()
    };
    check("order_folders_first", options);
}

#[cfg(feature = "yaml")]
#[test]
fn pure_alphabetic_interleaves_folders_and_files() {
    // a.yml < z_sub: the folder's content loads later and wins
    let options = c4::Options {
        dir_depth: -1,
        order: c4::Order::Alphabetic,
        ..base()
    };
    check("order_alphabetic", options);
}

#[cfg(any(feature = "json", feature = "jsonc"))]
#[test]
fn reverse_alphabetic_order() {
    // b.json loads first, a.json overrides
    let options = c4::Options {
        order: c4::Order::ReverseAlphabetic,
        ..base()
    };
    check("merge_order_reverse", options);
}

#[cfg(all(any(feature = "json", feature = "jsonc"), feature = "yaml"))]
#[test]
fn case_insensitive_lowercases_and_merges_keys() {
    let options = c4::Options {
        case_sensitive: false,
        ..base()
    };
    check("case_insensitive", options);
}

#[cfg(feature = "csv")]
#[test]
fn dot_key_builds_nested_objects() {
    // dot_key defaults to true
    check("csv_dot_key", base());
}

#[cfg(feature = "csv")]
#[test]
fn without_dot_key_stays_flat() {
    let options = c4::Options {
        dot_key: false,
        ..base()
    };
    check("csv_flat_key", options);
}

// Folder/file keying (the tree shape). This needs no Cargo feature — the
// `tree` feature only changes the defaults — so these run on any build
// with the fixtures' format features.
mod tree {
    #[allow(unused_imports)]
    use crate::common::{base, check};

    fn tree_options() -> c4::Options {
        c4::Options {
            filename_as_key: true,
            dirname_as_key: true,
            sheetname_as_key: true,
            dir_depth: -1,
            ..base()
        }
    }

    #[cfg(any(feature = "json", feature = "jsonc"))]
    #[test]
    fn folders_and_filenames_become_keys() {
        // a/b.json={c:1}, d.json={a:123} → {a:{b:{c:1}}, d:{a:123}}
        check("tree_basic", tree_options());
    }

    // the serialized trace now carries the format id ("ipv4" vs "str"),
    // so the fixture check needs the feature that produced it
    #[cfg(feature = "ipv4")]
    #[test]
    fn auto_parses_extensionless_files() {
        check("tree_auto", tree_options());
    }

    #[cfg(feature = "ipv4")]
    #[test]
    fn auto_file_content_is_typed() {
        let mut options = tree_options();
        options.sources = vec!["tests/fixtures/tree_auto/config".into()];
        let traced = c4::Loader::new(options).trace().unwrap();
        let c4::TracedValue::Object(root) = traced else {
            panic!("root must be an object");
        };
        let c4::TracedValue::Object(a) = &root["a"] else {
            panic!("a must be an object");
        };
        let c4::TracedValue::Leaf { value, .. } = &a["blabla"] else {
            panic!("blabla must be a leaf");
        };
        assert_eq!(*value, c4::Value::Ipv4("1.1.1.1".parse().unwrap()));
    }

    #[cfg(all(not(feature = "ipv4"), not(feature = "inet")))]
    #[test]
    fn auto_file_content_stays_string_without_feature() {
        let mut options = tree_options();
        options.sources = vec!["tests/fixtures/tree_auto/config".into()];
        let v: serde_json::Value = c4::Loader::new(options).load().unwrap();
        assert_eq!(v, serde_json::json!({ "a": { "blabla": "1.1.1.1" } }));
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn order_decides_key_collisions() {
        // a.yml and the folder a/ both produce the key "a". Default
        // order (folders first): the folder loads first, a.yml loads
        // later and wins c.d
        check("tree_order", tree_options());
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn reverse_order_flips_the_collision() {
        // reverse alphabetic: "a.yml" sorts after "a", so it loads
        // first and the folder's c.d wins
        let options = c4::Options {
            order: c4::Order::ReverseAlphabetic,
            ..tree_options()
        };
        check("tree_order_reverse", options);
    }

    #[cfg(any(feature = "json", feature = "jsonc"))]
    #[test]
    fn unknown_extensions_ignored_by_default() {
        check("tree_unknown", tree_options());
    }

    #[cfg(any(feature = "json", feature = "jsonc"))]
    #[test]
    fn unknown_extensions_error_when_strict() {
        let mut options = c4::Options {
            ignore_unknown_ext: false,
            ..tree_options()
        };
        options.sources = vec!["tests/fixtures/tree_unknown/config".into()];
        let res = c4::Loader::new(options).load::<c4::Value>();
        assert!(matches!(res, Err(c4::Error::Parse { .. })));
    }
}

// Commented names and keys: one prefix set (`#`, `_` — never `.`) and
// four options, all default true. The three name options filter
// scanning; ignore_commented_data_keys filters the keys a source parses to.
mod commented {
    #[allow(unused_imports)] // unused in single-format builds
    use crate::common::{base, check, fx, loader};

    #[cfg(any(feature = "json", feature = "jsonc"))]
    #[test]
    fn commented_files_and_folders_are_skipped() {
        // `_draft.json`/`#old.json` and everything under `_wip/`,
        // `#tmp/` are skipped; `.hidden.json` loads — `.` is not a
        // comment prefix
        check("commented_names", base());
    }

    #[cfg(any(feature = "json", feature = "jsonc"))]
    #[test]
    fn commented_files_and_folders_load_when_options_off() {
        let options = c4::Options {
            ignore_commented_filenames: false,
            ignore_commented_dirnames: false,
            ignore_commented_sheetnames: false,
            ..base()
        };
        check("commented_names_off", options);
    }

    #[cfg(any(feature = "json", feature = "jsonc"))]
    #[test]
    fn commented_names_produce_no_keys() {
        // the filter is on the name, so a skipped file/folder simply
        // has no key to contribute under
        let options = c4::Options {
            filename_as_key: true,
            dirname_as_key: true,
            dir_depth: -1,
            sources: vec![fx("commented_names/config").into()],
            ..base()
        };
        let value: serde_json::Value = c4::Loader::new(options).load().unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                ".hidden": { "hidden": true },
                "app": { "port": 8080 },
                "sub": { "b": { "sub": 1 } },
            })
        );
    }

    #[cfg(any(feature = "json", feature = "jsonc"))]
    #[test]
    fn name_keys_survive_the_key_filter() {
        // the three name options govern the keys made from names:
        // with them off the prefixed file/folder keys appear even though
        // ignore_commented_data_keys is on (it only filters parsed content)
        let options = c4::Options {
            filename_as_key: true,
            dirname_as_key: true,
            dir_depth: -1,
            ignore_commented_filenames: false,
            ignore_commented_dirnames: false,
            sources: vec![fx("commented_names_off/config").into()],
            ..base() // ignore_commented_data_keys stays true
        };
        let value: serde_json::Value = c4::Loader::new(options).load().unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "#old": { "port": 2, "old": true },
                "#tmp": { "d": { "tmp": true } },
                ".hidden": { "hidden": true },
                "_draft": { "port": 1, "draft": true },
                "_wip": { "#deep": { "e": { "deep": true } }, "c": { "wip": true } },
                "app": { "port": 8080 },
                "sub": { "b": { "sub": 1 } },
            })
        );
    }

    #[cfg(any(feature = "json", feature = "jsonc"))]
    #[test]
    fn explicitly_named_sources_are_never_filtered() {
        // the name options filter scanning, not the sources you name: a
        // commented folder and a commented file both load when named
        let value: serde_json::Value = loader(vec![
            fx("commented_names/config/_wip").into(),
            fx("commented_names/config/_draft.json").into(),
        ])
        .load()
        .unwrap();
        assert_eq!(
            value,
            serde_json::json!({ "wip": true, "port": 1, "draft": true })
        );
    }

    #[cfg(any(feature = "json", feature = "jsonc"))]
    #[test]
    fn an_explicitly_named_folder_still_filters_inside() {
        // only the named folder itself is exempt — scanning below it
        // filters as usual, so `_wip/#deep/` stays out
        let options = c4::Options {
            dir_depth: -1,
            sources: vec![fx("commented_names/config/_wip").into()],
            ..base()
        };
        let value: serde_json::Value = c4::Loader::new(options).load().unwrap();
        assert_eq!(value, serde_json::json!({ "wip": true }));
    }

    #[cfg(feature = "excel")]
    #[test]
    fn an_explicitly_named_sheet_is_never_filtered() {
        // a (format, path, sheet, layout) table source reads exactly
        // that sheet, commented name or not, keyed by the sheet name
        let value: serde_json::Value = loader(vec![
            (
                c4::Format::Excel,
                fx("excel_tree/config/a/b.xlsx"),
                "_z",
                "db",
            )
                .into(),
        ])
        .load()
        .unwrap();
        assert_eq!(value, serde_json::json!({ "_z": [{ "p": 3 }] }));
    }

    #[cfg(any(feature = "json", feature = "jsonc"))]
    #[test]
    fn commented_keys_are_dropped() {
        // nested objects and objects inside arrays too; an object left
        // empty stays an empty object
        check("commented_keys", base());
    }

    #[cfg(any(feature = "json", feature = "jsonc"))]
    #[test]
    fn commented_keys_load_when_option_off() {
        let options = c4::Options {
            ignore_commented_data_keys: false,
            ..base()
        };
        check("commented_keys_off", options);
    }

    #[test]
    fn key_filter_runs_after_dot_key_expansion() {
        // dot_key expands first, then prefixed segments are dropped
        // wherever they landed (custom format + parse_table, so this
        // runs under every feature combination)
        let kv = c4::CustomFormat::new("kv", ["kv"], |text, path, options| {
            let rows = text
                .lines()
                .map(|line| line.split('=').map(str::to_owned).collect())
                .collect();
            c4::parse_table(rows, &c4::TableLayout::Kv, path, options)
        });
        let text = "#a.b=1\nc.#d=2\nc.e=3\n_f=4";
        let value: serde_json::Value = loader(vec![(kv, text).into()]).load().unwrap();
        assert_eq!(value, serde_json::json!({ "c": { "e": 3 } }));
    }

    #[test]
    fn value_source_keys_are_filtered_too() {
        // every source kind goes through the filter, typed overrides
        // included
        let map = std::collections::BTreeMap::from([("port", 1), ("_tmp", 2)]);
        let value: serde_json::Value = loader(vec![(map,).into()]).load().unwrap();
        assert_eq!(value, serde_json::json!({ "port": 1 }));
    }
}
