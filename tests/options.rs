//! Options behavior: scan depth (`dir_depth`), folder/file keying
//! (`dirname_as_key`/`filename_as_key`), load order, dot_key, case
//! sensitivity, and the tree-shaped loading (folders/files as keys,
//! including order-driven key collisions and extension handling). The
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
