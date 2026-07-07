//! Options behavior: recursive scanning, flat, load order, dot_key,
//! case sensitivity, and tree mode (folders/files as keys, including
//! order-driven key collisions and extension handling).

mod common;

#[allow(unused_imports)] // unused in single-format builds
use common::check;
#[allow(unused_imports)] // unused in single-format builds
use common::loader;

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

#[cfg(feature = "yaml")]
#[test]
fn non_recursive_skips_subdirectories() {
    check("recursive_off", c4::Options::default());
}

#[cfg(feature = "yaml")]
#[test]
fn recursive_nests_subdirectory_as_key() {
    // flat defaults to true — nesting by path is the opt-in now
    let options = c4::Options {
        recursive: true,
        flat: false,
        ..c4::Options::default()
    };
    check("recursive_nest", options);
}

#[cfg(feature = "yaml")]
#[test]
fn recursive_flat_ignores_paths() {
    // flat is the default; recursive alone flattens subfolder paths
    let options = c4::Options {
        recursive: true,
        ..c4::Options::default()
    };
    check("recursive_flat", options);
}

#[cfg(feature = "yaml")]
#[test]
fn folders_load_before_files_by_default() {
    // Order::FoldersFirstAlphabetic (default): z_sub/ loads before a.yml
    // even though "a.yml" sorts first, so the top-level file wins
    let options = c4::Options {
        recursive: true,
        ..c4::Options::default()
    };
    check("order_folders_first", options);
}

#[cfg(feature = "yaml")]
#[test]
fn pure_alphabetic_interleaves_folders_and_files() {
    // a.yml < z_sub: the folder's content loads later and wins
    let options = c4::Options {
        recursive: true,
        order: c4::Order::Alphabetic,
        ..c4::Options::default()
    };
    check("order_alphabetic", options);
}

#[cfg(any(feature = "json", feature = "jsonc"))]
#[test]
fn reverse_alphabetic_order() {
    // b.json loads first, a.json overrides
    let options = c4::Options {
        order: c4::Order::ReverseAlphabetic,
        ..c4::Options::default()
    };
    check("merge_order_reverse", options);
}

#[cfg(all(any(feature = "json", feature = "jsonc"), feature = "yaml"))]
#[test]
fn case_insensitive_lowercases_and_merges_keys() {
    let options = c4::Options {
        case_sensitive: false,
        ..c4::Options::default()
    };
    check("case_insensitive", options);
}

#[cfg(feature = "csv")]
#[test]
fn dot_key_builds_nested_objects() {
    // dot_key defaults to true
    check("csv_dot_key", c4::Options::default());
}

#[cfg(feature = "csv")]
#[test]
fn without_dot_key_stays_flat() {
    let options = c4::Options {
        dot_key: false,
        ..c4::Options::default()
    };
    check("csv_flat_key", options);
}

#[cfg(feature = "tree")]
mod tree {
    #[allow(unused_imports)]
    use crate::common::check;

    fn tree_options() -> c4::Options {
        c4::Options {
            tree: true,
            ..c4::Options::default()
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
        let options = c4::Options {
            ignore_unknown_ext: false,
            ..tree_options()
        };
        let mut options = options;
        options.sources = vec!["tests/fixtures/tree_unknown/config".into()];
        let res = c4::Loader::new(options).load::<c4::Value>();
        assert!(matches!(res, Err(c4::Error::Parse { .. })));
    }
}

#[cfg(not(feature = "tree"))]
#[test]
fn tree_without_feature_is_unsupported() {
    let options = c4::Options {
        tree: true,
        ..c4::Options::default()
    };
    let mut options = options;
    options.sources = vec!["tests/fixtures/simple/config".into()];
    let res = c4::Loader::new(options).load::<c4::Value>();
    assert!(matches!(res, Err(c4::Error::Unsupported(_))));
}
