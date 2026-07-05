//! Per-format parsing, each behind its Cargo feature: the csv table
//! schema (scalars + aliases, auto detection, numeric literals, the
//! feature-gated typed formats with their PostgreSQL shapes, extended
//! types, header options, row errors), strict json, extension
//! reassignment, format-spec forms, a custom markdown-table format, and
//! toml/ini/env basics.

mod common;

#[allow(unused_imports)] // unused when no extra format feature is on
use common::{check, fx, loader};

#[cfg(feature = "csv")]
mod csv {
    use crate::common::{check, fx, loader};

    #[test]
    fn scalar_types_and_aliases() {
        // includes int/number/string/boolean aliases, an explicit `auto`
        // row and a row with an empty type cell (also auto)
        check("csv_scalars", c4::Options::default());
    }

    #[test]
    fn two_columns_auto_types() {
        // no type column at all: bool → integer → float → string, and
        // leading-zero integers stay strings
        check("csv_auto", c4::Options::default());
    }

    #[test]
    fn extended_types_rejected_by_default() {
        // default TableTypes::scalars(): null / arr:<t> / json rows error out
        let res = loader(vec![c4::Source::folder(fx("csv_extended/config"))]).load::<c4::Value>();
        assert!(res.is_err());
    }

    // the `json` cell type parses only when the json format feature is
    // compiled in; the fixture expects the parsed object
    #[cfg(feature = "json")]
    #[test]
    fn extended_types_opt_in() {
        let mut options = c4::Options::default();
        options.table.types = c4::TableTypes::all();
        check("csv_extended", options);
    }

    #[cfg(not(feature = "json"))]
    #[test]
    fn json_cell_without_json_feature_is_unknown_type() {
        // even with the json table type opted in, the id only exists when
        // the json format feature is compiled in
        let mut options = c4::Options::default();
        options.table.types = c4::TableTypes::all();
        let mut options = options;
        options.sources = vec![c4::Source::string(
            c4::Format::Csv,
            r#"meta,"{""x"":1}",json"#,
        )];
        let err = c4::Loader::new(options).load::<c4::Value>().unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    #[test]
    fn header_row_opt_in() {
        // header: true expects a key,value,format header row
        let mut options = c4::Options::default();
        options.table.header = true;
        check("csv_header", options);
    }

    #[test]
    fn custom_column_names_and_order() {
        // header is t,k,v: columns are located by name, order is free
        let mut options = c4::Options::default();
        options.table.header = true;
        options.table.columns = c4::TableColumns {
            key: "k".into(),
            value: "v".into(),
            format: "t".into(),
        };
        check("csv_custom_columns", options);
    }

    // the type ids only exist with their parser feature, so the fixture
    // checks are gated; without the feature the same rows are errors
    #[cfg(feature = "datetime")]
    #[test]
    fn dt_type_and_datetime_alias() {
        check("csv_datetime", c4::Options::default());
    }

    #[cfg(all(feature = "date", feature = "time", feature = "ipv4", feature = "ipv6"))]
    #[test]
    fn date_time_ip_types() {
        // parsed values serialize back to their (canonical) text, so the
        // expectation file needs no feature-specific variants
        check("csv_date_time_ip", c4::Options::default());
    }

    /// The single leaf of a one-row csv string source — typed assertions
    /// use string sources so each test only involves its own type id.
    #[allow(dead_code)] // unused when no value-parser feature is on
    fn csv_leaf(row: &str, key: &str) -> c4::Value {
        let traced = loader(vec![c4::Source::string(c4::Format::Csv, row)])
            .trace()
            .unwrap();
        let c4::TracedValue::Object(root) = traced else {
            panic!("root must be an object");
        };
        let c4::TracedValue::Leaf { value, .. } = &root[key] else {
            panic!("{key} must be a leaf");
        };
        value.clone()
    }

    #[cfg(feature = "datetime")]
    #[test]
    fn dt_is_a_typed_value() {
        assert_eq!(
            csv_leaf("born,2024-01-02T03:04:05Z,dt", "born"),
            c4::Value::DateTime("2024-01-02T03:04:05Z".into())
        );
    }

    #[cfg(not(feature = "datetime"))]
    #[test]
    fn dt_without_feature_is_unknown_type() {
        // the id does not exist without the feature — a row using it is
        // an error, not a silent string
        let err = loader(vec![c4::Source::folder(fx("csv_datetime/config"))])
            .load::<c4::Value>()
            .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    #[cfg(feature = "date")]
    #[test]
    fn date_is_a_typed_value() {
        assert_eq!(
            csv_leaf("d,2024-01-02,date", "d"),
            c4::Value::Date("2024-01-02".into())
        );
    }

    #[cfg(feature = "time")]
    #[test]
    fn time_is_a_typed_value() {
        assert_eq!(
            csv_leaf("t,03:04:05,time", "t"),
            c4::Value::Time("03:04:05".into())
        );
    }

    #[cfg(feature = "ipv4")]
    #[test]
    fn ipv4_is_a_typed_value() {
        assert_eq!(
            csv_leaf("ip4,10.0.0.1,ipv4", "ip4"),
            c4::Value::Ipv4("10.0.0.1".parse().unwrap())
        );
    }

    #[cfg(feature = "ipv6")]
    #[test]
    fn ipv6_is_a_typed_value() {
        assert_eq!(
            csv_leaf("ip6,::1,ipv6", "ip6"),
            c4::Value::Ipv6("::1".parse().unwrap())
        );
    }

    #[cfg(not(feature = "ipv4"))]
    #[test]
    fn ipv4_without_feature_is_unknown_type() {
        let err = loader(vec![c4::Source::string(c4::Format::Csv, "a,1.1.1.1,ipv4")])
            .load::<c4::Value>()
            .unwrap_err();
        match err {
            c4::Error::Table { row, message, .. } => {
                assert_eq!(row, 1);
                assert!(message.contains("ipv4"), "message: {message}");
            }
            other => panic!("expected Error::Table, got {other:?}"),
        }
    }

    #[cfg(all(
        feature = "inet",
        feature = "cidr",
        feature = "macaddr",
        feature = "macaddr8",
        feature = "uuid"
    ))]
    #[test]
    fn net_string_types() {
        // serialized form is the text, so expect.json needs no variants
        check("csv_net", c4::Options::default());
    }

    #[cfg(feature = "inet")]
    #[test]
    fn inet_parses_either_ip_family() {
        assert_eq!(
            csv_leaf("a,10.0.0.1,inet", "a"),
            c4::Value::Ipv4("10.0.0.1".parse().unwrap())
        );
        assert_eq!(
            csv_leaf("a,::1,inet", "a"),
            c4::Value::Ipv6("::1".parse().unwrap())
        );
    }

    #[cfg(feature = "inet")]
    #[test]
    fn inet_with_netmask_keeps_host_bits() {
        // PostgreSQL inet: optional netmask, host bits below it allowed
        assert_eq!(
            csv_leaf("a,10.0.0.1/24,inet", "a"),
            c4::Value::Inet("10.0.0.1/24".into())
        );
        let err = loader(vec![c4::Source::string(
            c4::Format::Csv,
            "a,10.0.0.1/33,inet",
        )])
        .load::<c4::Value>()
        .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    #[cfg(feature = "cidr")]
    #[test]
    fn cidr_is_a_typed_value() {
        assert_eq!(
            csv_leaf("a,10.0.0.0/8,cidr", "a"),
            c4::Value::Cidr("10.0.0.0/8".into())
        );
        // PostgreSQL cidr: the netmask is optional — a bare address is a
        // full-length host network
        assert_eq!(
            csv_leaf("a,10.1.2.3,cidr", "a"),
            c4::Value::Cidr("10.1.2.3".into())
        );
    }

    #[cfg(feature = "cidr")]
    #[test]
    fn cidr_rejects_host_bits_below_the_mask() {
        // valid inet, but not a cidr network
        let err = loader(vec![c4::Source::string(
            c4::Format::Csv,
            "a,10.0.0.1/24,cidr",
        )])
        .load::<c4::Value>()
        .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    #[cfg(feature = "cidr")]
    #[test]
    fn bad_cidr_prefix_reports_row() {
        let err = loader(vec![c4::Source::string(
            c4::Format::Csv,
            "a,10.0.0.0/33,cidr",
        )])
        .load::<c4::Value>()
        .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    #[cfg(feature = "inet")]
    #[test]
    fn auto_guesses_masked_forms() {
        // inet implies cidr: proper networks trace as cidr, addresses
        // with host bits set fall to inet; bare hosts stay ipv4/ipv6
        assert_eq!(
            csv_leaf("a,10.0.0.0/8", "a"),
            c4::Value::Cidr("10.0.0.0/8".into())
        );
        assert_eq!(
            csv_leaf("a,10.0.0.1/24", "a"),
            c4::Value::Inet("10.0.0.1/24".into())
        );
        assert_eq!(
            csv_leaf("a,10.0.0.1", "a"),
            c4::Value::Ipv4("10.0.0.1".parse().unwrap())
        );
    }

    #[cfg(feature = "macaddr")]
    #[test]
    fn macaddr_is_a_typed_value() {
        assert_eq!(
            csv_leaf("a,aa:bb:cc:dd:ee:ff,macaddr", "a"),
            c4::Value::MacAddr("aa:bb:cc:dd:ee:ff".into())
        );
        assert_eq!(
            csv_leaf("a,aa-bb-cc-dd-ee-ff,macaddr", "a"),
            c4::Value::MacAddr("aa-bb-cc-dd-ee-ff".into())
        );
    }

    #[cfg(feature = "macaddr")]
    #[test]
    fn explicit_macaddr_accepts_postgres_forms() {
        // exactly the PostgreSQL macaddr input formats
        for form in [
            "08:00:2b:01:02:03",
            "08-00-2b-01-02-03",
            "08002b:010203",
            "08002b-010203",
            "0800.2b01.0203",
            "0800-2b01-0203",
            "08002b010203",
        ] {
            assert_eq!(
                csv_leaf(&format!("a,{form},macaddr"), "a"),
                c4::Value::MacAddr(form.into()),
                "form {form}"
            );
        }
        // wrong digit count, mixed separators, off-list groupings fail
        for bad in ["08002b:0102", "08:00-2b:01:02:03", "0800:2b01:0203"] {
            let err = loader(vec![c4::Source::string(
                c4::Format::Csv,
                format!("a,{bad},macaddr"),
            )])
            .load::<c4::Value>()
            .unwrap_err();
            assert!(matches!(err, c4::Error::Table { row: 1, .. }), "bad {bad}");
        }
    }

    #[cfg(feature = "macaddr8")]
    #[test]
    fn explicit_macaddr8_accepts_postgres_forms() {
        for form in [
            "08:00:2b:01:02:03:04:05",
            "08002b:0102030405",
            "08002b01:02030405",
            "0800.2b01.0203.0405",
            "08002b0102030405",
        ] {
            assert_eq!(
                csv_leaf(&format!("a,{form},macaddr8"), "a"),
                c4::Value::MacAddr8(form.into()),
                "form {form}"
            );
        }
    }

    #[cfg(feature = "macaddr")]
    #[test]
    fn auto_only_guesses_pair_form_macs() {
        // bare/grouped hex is only a mac when the type says so
        assert_eq!(
            csv_leaf("a,08002b:010203", "a"),
            c4::Value::String("08002b:010203".into())
        );
    }

    #[cfg(feature = "macaddr8")]
    #[test]
    fn macaddr8_is_a_typed_value() {
        assert_eq!(
            csv_leaf("a,aa:bb:cc:dd:ee:ff:00:11,macaddr8", "a"),
            c4::Value::MacAddr8("aa:bb:cc:dd:ee:ff:00:11".into())
        );
    }

    #[cfg(feature = "uuid")]
    #[test]
    fn uuid_is_a_typed_value() {
        assert_eq!(
            csv_leaf("a,550e8400-e29b-41d4-a716-446655440000,uuid", "a"),
            c4::Value::Uuid("550e8400-e29b-41d4-a716-446655440000".into())
        );
        // the explicit format also takes the bare 32-hex spelling
        assert_eq!(
            csv_leaf("a,550e8400e29b41d4a716446655440000,uuid", "a"),
            c4::Value::Uuid("550e8400e29b41d4a716446655440000".into())
        );
    }

    #[cfg(feature = "uuid")]
    #[test]
    fn auto_only_guesses_hyphenated_uuids() {
        assert_eq!(
            csv_leaf("a,550e8400e29b41d4a716446655440000", "a"),
            c4::Value::String("550e8400e29b41d4a716446655440000".into())
        );
    }

    #[cfg(feature = "uuid")]
    #[test]
    fn auto_guesses_uuid() {
        assert_eq!(
            csv_leaf("a,550e8400-e29b-41d4-a716-446655440000", "a"),
            c4::Value::Uuid("550e8400-e29b-41d4-a716-446655440000".into())
        );
    }

    #[cfg(feature = "macaddr")]
    #[test]
    fn auto_guesses_macaddr() {
        assert_eq!(
            csv_leaf("a,aa:bb:cc:dd:ee:ff", "a"),
            c4::Value::MacAddr("aa:bb:cc:dd:ee:ff".into())
        );
    }

    #[cfg(feature = "numeric")]
    #[test]
    fn numeric_literals_in_auto() {
        use c4::Value::{Int, String as Str, Uint};
        for (raw, want) in [
            ("0xff", Int(255)),
            ("0b101", Int(5)),
            ("0o17", Int(15)),
            ("-0x10", Int(-16)),
            ("1_000_000", Int(1_000_000)),
            ("123n", Int(123)),
            ("0xffn", Int(255)),
            ("18_446_744_073_709_551_615", Uint(u64::MAX)),
            // leading-zero decimals still stay strings
            ("007", Str("007".into())),
            // misplaced separators are not numbers
            ("1__0", Str("1__0".into())),
            ("_1", Str("_1".into())),
            ("1_", Str("1_".into())),
        ] {
            assert_eq!(csv_leaf(&format!("a,{raw}"), "a"), want, "raw {raw}");
        }
    }

    #[cfg(feature = "numeric")]
    #[test]
    fn numeric_literals_in_explicit_types() {
        assert_eq!(csv_leaf("a,0x10,u8", "a"), c4::Value::Uint(16));
        assert_eq!(csv_leaf("a,1_000,i64", "a"), c4::Value::Int(1000));
        assert_eq!(csv_leaf("a,1_000.5,f64", "a"), c4::Value::Float(1000.5));
        assert_eq!(csv_leaf("a,0xff,f64", "a"), c4::Value::Float(255.0));
    }

    #[cfg(not(feature = "numeric"))]
    #[test]
    fn radix_literals_without_numeric_feature() {
        // auto: not a number, stays a string
        assert_eq!(csv_leaf("a,0xff", "a"), c4::Value::String("0xff".into()));
        // explicit numeric type: plain Rust parsing only → error
        let err = loader(vec![c4::Source::string(c4::Format::Csv, "a,0xff,u8")])
            .load::<c4::Value>()
            .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    #[cfg(not(feature = "uuid"))]
    #[test]
    fn uuid_without_feature_is_unknown_type() {
        let err = loader(vec![c4::Source::string(
            c4::Format::Csv,
            "a,550e8400-e29b-41d4-a716-446655440000,uuid",
        )])
        .load::<c4::Value>()
        .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    #[cfg(feature = "datetime")]
    #[test]
    fn auto_guesses_dt() {
        assert_eq!(
            csv_leaf("when,2024-01-02T03:04:05Z", "when"),
            c4::Value::DateTime("2024-01-02T03:04:05Z".into())
        );
    }

    #[cfg(feature = "date")]
    #[test]
    fn auto_guesses_date() {
        assert_eq!(
            csv_leaf("day,2024-01-02", "day"),
            c4::Value::Date("2024-01-02".into())
        );
    }

    #[cfg(feature = "time")]
    #[test]
    fn auto_guesses_time() {
        assert_eq!(
            csv_leaf("at,03:04:05", "at"),
            c4::Value::Time("03:04:05".into())
        );
    }

    #[cfg(feature = "ipv4")]
    #[test]
    fn auto_guesses_ipv4() {
        assert_eq!(
            csv_leaf("a,1.1.1.1", "a"),
            c4::Value::Ipv4("1.1.1.1".parse().unwrap())
        );
    }

    #[cfg(feature = "ipv6")]
    #[test]
    fn auto_guesses_ipv6() {
        assert_eq!(
            csv_leaf("a,2001:db8::1", "a"),
            c4::Value::Ipv6("2001:db8::1".parse().unwrap())
        );
    }

    // inet also auto-guesses IPs, so this needs both features off
    #[cfg(all(not(feature = "ipv4"), not(feature = "inet")))]
    #[test]
    fn auto_without_ipv4_feature_keeps_string() {
        assert_eq!(
            csv_leaf("a,1.1.1.1", "a"),
            c4::Value::String("1.1.1.1".into())
        );
    }

    #[cfg(not(feature = "datetime"))]
    #[test]
    fn auto_without_dt_feature_keeps_string() {
        let v: serde_json::Value = loader(vec![c4::Source::string(
            c4::Format::Csv,
            "when,2024-01-02T03:04:05Z",
        )])
        .load()
        .unwrap();
        assert_eq!(v, serde_json::json!({ "when": "2024-01-02T03:04:05Z" }));
    }

    #[cfg(feature = "datetime")]
    #[test]
    fn bad_dt_reports_row() {
        let err = loader(vec![c4::Source::string(
            c4::Format::Csv,
            "when,tomorrow,datetime",
        )])
        .load::<c4::Value>()
        .unwrap_err();
        match err {
            c4::Error::Table { row, .. } => assert_eq!(row, 1),
            other => panic!("expected Error::Table, got {other:?}"),
        }
    }

    #[cfg(feature = "ipv4")]
    #[test]
    fn bad_ipv4_reports_row() {
        let err = loader(vec![c4::Source::string(
            c4::Format::Csv,
            "ip,10.0.0.999,ipv4",
        )])
        .load::<c4::Value>()
        .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    #[test]
    fn bad_cell_reports_row() {
        // 999 does not fit i8; no header, so the bad row is row 1
        let err = loader(vec![c4::Source::folder(fx("csv_bad/config"))])
            .load::<c4::Value>()
            .unwrap_err();
        match err {
            c4::Error::Table { row, .. } => assert_eq!(row, 1),
            other => panic!("expected Error::Table, got {other:?}"),
        }
    }
}

#[cfg(feature = "json")]
#[test]
fn strict_json_rejects_comments() {
    let options = c4::Options {
        formats: vec![c4::Format::Json.into()],
        ..c4::Options::default()
    };
    let mut options = options;
    options.sources = vec![c4::Source::folder(fx("jsonc/config"))];
    let res = c4::Loader::new(options).load::<c4::Value>();
    assert!(matches!(res, Err(c4::Error::Parse { .. })));
}

#[cfg(all(feature = "jsonc", feature = "yaml"))]
#[test]
fn reassigned_extension_uses_claiming_parser() {
    // the extension decides the parser, last claimer wins:
    // [yaml, (jsonc, ["yml"])] → .yml files are read by the jsonc parser
    let options = c4::Options {
        formats: vec![c4::Format::Yaml.into(), (c4::Format::Jsonc, ["yml"]).into()],
        ..c4::Options::default()
    };
    check("ext_override", options);
}

#[cfg(feature = "jsonc")]
#[test]
fn format_spec_string_tuple_form() {
    // ("jsonc", ["json", "jsonc"]) — string id with custom extensions
    let options = c4::Options {
        formats: vec![("jsonc", ["json", "jsonc"]).into()],
        ..c4::Options::default()
    };
    check("formats_tuple", options);
}

#[cfg(feature = "toml")]
#[test]
fn toml_basic() {
    check("toml_basic", c4::Options::default());
}

#[cfg(all(feature = "toml", feature = "datetime"))]
#[test]
fn toml_datetime_serializes_as_text() {
    // the trace format field is "dt" only with the datetime feature
    check("toml_datetime", c4::Options::default());
}

#[cfg(feature = "toml")]
#[test]
fn toml_datetime_follows_dt_feature() {
    use c4::{TracedValue, Value};

    let traced = loader(vec![c4::Source::folder(fx("toml_datetime/config"))])
        .trace()
        .unwrap();
    let TracedValue::Object(root) = traced else {
        panic!("root must be an object");
    };
    let TracedValue::Leaf { value, .. } = &root["when"] else {
        panic!("when must be a leaf");
    };
    #[cfg(feature = "datetime")]
    assert_eq!(*value, Value::DateTime("2024-01-02T03:04:05Z".into()));
    #[cfg(not(feature = "datetime"))]
    assert_eq!(*value, Value::String("2024-01-02T03:04:05Z".into()));
}

#[cfg(feature = "ini")]
#[test]
fn ini_basic() {
    // ini has no value types: everything is a string; sections nest
    check("ini_basic", c4::Options::default());
}

#[cfg(feature = "env")]
#[test]
fn env_basic() {
    // `#` comments, optional `export`, quote stripping; values are
    // strings; no variable interpolation
    check("env_basic", c4::Options::default());
}

#[cfg(feature = "env")]
#[test]
fn env_extension_variants() {
    // `.env` itself (dotfile-as-extension rule) plus `local.env`;
    // ".env" < "local.env", so local.env overrides
    check("env_variants", c4::Options::default());
}

#[test]
fn custom_format_markdown_table() {
    // a user-defined format: lower the markdown pipe-table into rows and
    // hand them to the generic table stage
    let md = c4::CustomFormat::new("md-table", ["md"], |text, path, options| {
        let rows: Vec<Vec<String>> = text
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with('|') && !l.contains("---"))
            // markdown tables always start with a header row — dropping
            // it here keeps the data rows positional (key,value[,format])
            // with no Options.table changes needed
            .skip(1)
            .map(|l| {
                l.trim_matches('|')
                    .split('|')
                    .map(|cell| cell.trim().to_owned())
                    .collect()
            })
            .collect();
        c4::parse_table(rows, path, options)
    });
    // one list, one claim order: the defaults plus the custom format
    let mut formats = c4::Options::default().formats;
    formats.push(md.into());
    check(
        "custom_md",
        c4::Options {
            formats,
            ..c4::Options::default()
        },
    );
}
