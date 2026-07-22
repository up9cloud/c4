//! Per-format parsing, each behind its Cargo feature: the csv table
//! schema (scalars + aliases, auto detection, numeric literals, the
//! feature-gated typed formats with their PostgreSQL shapes, extended
//! types, header options, row errors), strict json, extension
//! reassignment, format-spec forms, a custom markdown-table format, and
//! toml/ini/env basics.

mod common;

#[allow(unused_imports)] // unused when no extra format feature is on
use common::{base, check, fx, loader};

#[cfg(feature = "csv")]
mod csv {
    use crate::common::{base, check, fx, loader};

    #[test]
    fn scalar_types_and_aliases() {
        // includes int/number/string/boolean aliases, an explicit `auto`
        // row and a row with an empty type cell (also auto)
        check("csv_scalars", base());
    }

    #[test]
    fn two_columns_auto_types() {
        // no type column at all: bool → integer → float → string, and
        // leading-zero integers stay strings
        check("csv_auto", base());
    }

    // a `json` cell holds a whole JSON document (array, object, null, …);
    // cell format ids mirror the file formats one-to-one, so `json`
    // parses with the strict json parser and needs the `json` feature
    #[cfg(feature = "json")]
    #[test]
    fn json_cell_holds_a_document() {
        check("csv_extended", base());
    }

    // strict means strict: a jsonc-ism in a `json` cell fails the row
    #[cfg(feature = "json")]
    #[test]
    fn json_cell_is_strict() {
        let err = loader(vec![(c4::Format::Csv, r#"meta,"{""x"":1,}",json"#).into()])
            .load::<c4::Value>()
            .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    // without the json feature the `json` id does not exist — even when
    // jsonc is compiled in (no cross-parser fallback)
    #[cfg(not(feature = "json"))]
    #[test]
    fn json_cell_without_json_feature_is_unknown_type() {
        let err = loader(vec![(c4::Format::Csv, r#"meta,"{""x"":1}",json"#).into()])
            .load::<c4::Value>()
            .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    // a `jsonc` cell parses with the jsonc parser: comments and trailing
    // commas are fine
    #[cfg(feature = "jsonc")]
    #[test]
    fn jsonc_cell_allows_jsonc_syntax() {
        let value: c4::Value = loader(vec![
            (c4::Format::Csv, r#"meta,"{""x"":1,} // note",jsonc"#).into(),
        ])
        .load()
        .unwrap();
        assert_eq!(value["meta"]["x"].as_i64(), Some(1));
    }

    #[cfg(not(feature = "jsonc"))]
    #[test]
    fn jsonc_cell_without_jsonc_feature_is_unknown_type() {
        let err = loader(vec![(c4::Format::Csv, r#"meta,"{""x"":1}",jsonc"#).into()])
            .load::<c4::Value>()
            .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    // --- list cell type ids: array<sep> and csv<sep><layout> ---

    #[test]
    fn array_splits_flat_and_auto_types_each_element() {
        use c4::Value::{Array, Int, String as S};
        // custom separator; elements go through the shared auto detection
        assert_eq!(
            csv_leaf("tags,a|b|c,array|", "tags"),
            Array(vec![S("a".into()), S("b".into()), S("c".into())])
        );
        // default separator is `,` (quoted so the outer csv keeps one field)
        assert_eq!(
            csv_leaf("nums,\"1,2,3\",array", "nums"),
            Array(vec![Int(1), Int(2), Int(3)])
        );
    }

    #[test]
    fn array_empty_cell_is_empty_array() {
        assert_eq!(csv_leaf("x,,array", "x"), c4::Value::Array(vec![]));
    }

    #[test]
    fn array_with_a_per_element_format() {
        use c4::Value::{Array, String as S, Uint};
        // `array|u8`: every element parsed as u8 (not the auto-guessed i64)
        assert_eq!(
            csv_leaf("lvls,3|5|8,array|u8", "lvls"),
            Array(vec![Uint(3), Uint(5), Uint(8)])
        );
        // the format forces a type that differs from auto — here `str`
        // keeps the digits as strings
        assert_eq!(
            csv_leaf("ids,1|2|3,array|str", "ids"),
            Array(vec![S("1".into()), S("2".into()), S("3".into())])
        );
    }

    #[test]
    fn array_element_out_of_range_errors() {
        // an element that does not fit its per-element format fails the row
        let err = loader(vec![(c4::Format::Csv, "a,1|999,array|u8").into()])
            .load::<c4::Value>()
            .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    #[test]
    fn csv_cell_kv_layout_yields_an_object() {
        // the whole cell is a CSV document parsed under the kv layout
        let value: c4::Value = loader(vec![
            (c4::Format::Csv, "sub,\"a,1\nb,2\",\"csv,kv\"").into(),
        ])
        .load()
        .unwrap();
        assert_eq!(value["sub"]["a"], c4::Value::Int(1));
        assert_eq!(value["sub"]["b"], c4::Value::Int(2));
    }

    #[test]
    fn csv_cell_db_layout_yields_records() {
        let value: c4::Value = loader(vec![
            (c4::Format::Csv, "recs,\"a,b\ni64,i64\n1,2\",\"csv,db\"").into(),
        ])
        .load()
        .unwrap();
        assert_eq!(value["recs"][0]["a"], c4::Value::Int(1));
        assert_eq!(value["recs"][0]["b"], c4::Value::Int(2));
    }

    #[test]
    fn csv_cell_custom_separator() {
        // `csv;kv`: inner delimiter is `;` (no comma, so the format cell
        // survives the outer csv unquoted)
        let value: c4::Value = loader(vec![(c4::Format::Csv, "m,\"a;1\nb;2\",csv;kv").into()])
            .load()
            .unwrap();
        assert_eq!(value["m"]["a"], c4::Value::Int(1));
        assert_eq!(value["m"]["b"], c4::Value::Int(2));
    }

    #[test]
    fn csv_cell_unknown_layout_is_unknown_type() {
        let err = loader(vec![(c4::Format::Csv, "m,x,\"csv,nope\"").into()])
            .load::<c4::Value>()
            .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    #[test]
    fn csv_layout_is_positional_after_the_separator() {
        // `csvdb` reads as separator `d` + layout `b` — an unknown id
        let err = loader(vec![(c4::Format::Csv, "m,x,csvdb").into()])
            .load::<c4::Value>()
            .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    #[test]
    fn csv_non_ascii_separator_is_unknown_type() {
        let err = loader(vec![(c4::Format::Csv, "m,x,\"csv｜kv\"").into()])
            .load::<c4::Value>()
            .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    #[test]
    fn list_cells_load_from_a_fixture() {
        check("csv_list", base());
    }

    #[test]
    fn i128_and_u128_typed_cells() {
        // 128-bit is explicit-only; full-width values round-trip
        assert_eq!(
            csv_leaf("a,170141183460469231731687303715884105727,i128", "a"),
            c4::Value::Int128(i128::MAX)
        );
        assert_eq!(
            csv_leaf("a,340282366920938463463374607431768211455,u128", "a"),
            c4::Value::Uint128(u128::MAX)
        );
        assert_eq!(csv_leaf("a,-5,i128", "a"), c4::Value::Int128(-5));
        // a value past the declared type's range is an error
        let err = loader(vec![
            (
                c4::Format::Csv,
                "a,340282366920938463463374607431768211455,i128",
            )
                .into(),
        ])
        .load::<c4::Value>()
        .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    #[test]
    fn auto_never_widens_past_u64() {
        // an integer beyond u64 stays a float under auto — use an explicit
        // i128/u128 cell to keep the precision
        assert!(matches!(
            csv_leaf("a,340282366920938463463374607431768211455", "a"),
            c4::Value::Float(_)
        ));
    }

    #[test]
    fn explicit_bool_accepts_loose_tokens() {
        for t in ["true", "TRUE", "t", "yes", "Y", "on", "1"] {
            assert_eq!(
                csv_leaf(&format!("a,{t},bool"), "a"),
                c4::Value::Bool(true),
                "true token {t}"
            );
        }
        for f in ["false", "False", "f", "no", "N", "off", "0"] {
            assert_eq!(
                csv_leaf(&format!("a,{f},bool"), "a"),
                c4::Value::Bool(false),
                "false token {f}"
            );
        }
        // an unrecognized token in a bool cell errors
        let err = loader(vec![(c4::Format::Csv, "a,maybe,bool").into()])
            .load::<c4::Value>()
            .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    #[test]
    fn auto_bool_is_strict_case_insensitive() {
        // auto accepts only the words true/false (any case) — not yes/on/1
        assert_eq!(csv_leaf("a,True", "a"), c4::Value::Bool(true));
        assert_eq!(csv_leaf("a,FALSE", "a"), c4::Value::Bool(false));
        assert_eq!(csv_leaf("a,yes", "a"), c4::Value::String("yes".into()));
        assert_eq!(csv_leaf("a,1", "a"), c4::Value::Int(1));
    }

    // The built-in csv format is headerless positional. A header row and
    // renamed/reordered columns are handled by a CustomFormat that maps
    // the header and lowers the file to positional key,value,format rows —
    // the documented escape hatch (also in the `csv-header` example).
    #[test]
    fn header_and_columns_via_custom_format() {
        let csv_header = c4::CustomFormat::new("csv-header", ["csv"], |text, path, options| {
            let mut lines = text.lines().filter(|l| !l.trim().is_empty());
            let header: Vec<String> = lines
                .next()
                .unwrap_or_default()
                .split(',')
                .map(|c| c.trim().to_owned())
                .collect();
            let col = |name: &str| header.iter().position(|h| h == name);
            let (k, v, f) = (col("key"), col("value"), col("format"));
            let (Some(k), Some(v)) = (k, v) else {
                return Err(c4::Error::Parse {
                    path: path.to_path_buf(),
                    message: "csv-header needs key and value columns".into(),
                });
            };
            let rows = lines
                .map(|line| {
                    let cells: Vec<&str> = line.split(',').map(str::trim).collect();
                    let mut row = vec![cells[k].to_owned(), cells[v].to_owned()];
                    if let Some(fc) = f.and_then(|f| cells.get(f)) {
                        row.push((*fc).to_owned());
                    }
                    row
                })
                .collect();
            c4::parse_table(rows, &c4::TableLayout::Kv, path, options)
        });

        // columns in any order (value,key,format), located by their names
        let value: c4::Value = c4::Loader::new(c4::Options {
            sources: vec![(csv_header, "value,key,format\nc4,name,str\n8080,port,u16").into()],
            ..crate::common::base()
        })
        .load()
        .unwrap();
        assert_eq!(value["name"].as_str(), Some("c4"));
        assert_eq!(value["port"].as_u64(), Some(8080));
    }

    // A column-oriented (transposed) csv is another CustomFormat escape
    // hatch: rows of keys / values / formats, transposed so each column
    // becomes a positional [key, value, format] row (the `csv-transpose`
    // example). ipv4 both validates the cell and gates this test.
    #[cfg(feature = "ipv4")]
    #[test]
    fn transpose_via_custom_format() {
        let csv_t = c4::CustomFormat::new("csv-t", ["csv"], |text, path, options| {
            let grid: Vec<Vec<&str>> = text
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.split(',').map(str::trim).collect())
                .collect();
            let width = grid.iter().map(Vec::len).max().unwrap_or(0);
            let rows = (0..width)
                .map(|col| {
                    grid.iter()
                        .map(|r| r.get(col).copied().unwrap_or("").to_owned())
                        .collect()
                })
                .collect();
            c4::parse_table(rows, &c4::TableLayout::Kv, path, options)
        });

        // keys / values / formats rows; the last column has no format cell
        let value: c4::Value = c4::Loader::new(c4::Options {
            sources: vec![(csv_t.clone(), "a,b,c\n1,1.1.1.1,8080\nint,ipv4").into()],
            ..crate::common::base()
        })
        .load()
        .unwrap();
        assert_eq!(value["a"].as_i64(), Some(1)); // explicit int
        assert_eq!(value["b"].as_str(), Some("1.1.1.1")); // ipv4-validated, canonical string
        assert_eq!(value["c"].as_u64(), Some(8080)); // auto (no format cell)

        // the ipv4 format actually validates — a bad address errors
        let bad = c4::Loader::new(c4::Options {
            sources: vec![(csv_t, "b\n999.1.1.1\nipv4").into()],
            ..crate::common::base()
        })
        .load::<c4::Value>();
        assert!(matches!(bad, Err(c4::Error::Table { row: 1, .. })));
    }

    // the type ids only exist with their parser feature, so the fixture
    // checks are gated; without the feature the same rows are errors
    #[cfg(feature = "datetime")]
    #[test]
    fn dt_type_and_datetime_alias() {
        check("csv_datetime", base());
    }

    #[cfg(all(feature = "date", feature = "time", feature = "ipv4", feature = "ipv6"))]
    #[test]
    fn date_time_ip_types() {
        // parsed values serialize back to their (canonical) text, so the
        // expectation file needs no feature-specific variants
        check("csv_date_time_ip", base());
    }

    /// The single leaf of a one-row csv string source — typed assertions
    /// use string sources so each test only involves its own type id.
    #[allow(dead_code)] // unused when no value-parser feature is on
    fn csv_leaf(row: &str, key: &str) -> c4::Value {
        let traced = loader(vec![(c4::Format::Csv, row).into()]).trace().unwrap();
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
        let err = loader(vec![fx("csv_datetime/config").into()])
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
        let err = loader(vec![(c4::Format::Csv, "a,1.1.1.1,ipv4").into()])
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
        check("csv_net", base());
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
        let err = loader(vec![(c4::Format::Csv, "a,10.0.0.1/33,inet").into()])
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
        let err = loader(vec![(c4::Format::Csv, "a,10.0.0.1/24,cidr").into()])
            .load::<c4::Value>()
            .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    #[cfg(feature = "cidr")]
    #[test]
    fn bad_cidr_prefix_reports_row() {
        let err = loader(vec![(c4::Format::Csv, "a,10.0.0.0/33,cidr").into()])
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
            let err = loader(vec![(c4::Format::Csv, format!("a,{bad},macaddr")).into()])
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

    // relaxed human/spreadsheet notation on explicit numeric ids only:
    // thousands separators, currency prefixes and fullwidth digits.
    // Built via parse_table so commas inside a value don't split columns.
    #[cfg(feature = "numeric")]
    #[test]
    fn relaxed_numeric_forms_in_explicit_types() {
        use std::path::Path;

        use c4::Value::{Float, Int};

        let opts = c4::Options::default();
        let cell = |value: &str, ty: &str| -> c4::Value {
            let rows = vec![vec!["k".to_string(), value.to_string(), ty.to_string()]];
            let v = c4::parse_table(rows, &c4::TableLayout::Kv, Path::new("doc"), &opts).unwrap();
            v["k"].clone()
        };

        assert_eq!(cell("1,000.1", "f64"), Float(1000.1));
        assert_eq!(cell("10 234 345.111", "f64"), Float(10_234_345.111));
        assert_eq!(cell("$123.12", "f64"), Float(123.12));
        assert_eq!(cell("€99", "i64"), Int(99));
        assert_eq!(cell("£1,000", "u64"), c4::Value::Uint(1000));
        assert_eq!(cell("¥1,234", "i32"), Int(1234));
        assert_eq!(cell("-$5", "i8"), Int(-5));
        // fullwidth digits
        assert_eq!(cell("１２３", "i64"), Int(123));
        // the standard literal forms still win first
        assert_eq!(cell("0xff", "u8"), c4::Value::Uint(255));
        // a fraction still fails an integer id after cleaning
        let rows = vec![vec![
            "k".to_string(),
            "1,000.5".to_string(),
            "i64".to_string(),
        ]];
        assert!(c4::parse_table(rows, &c4::TableLayout::Kv, Path::new("doc"), &opts).is_err());

        // auto never runs the relaxed pass — it stays a string
        let rows = vec![vec!["k".to_string(), "1,000.1".to_string()]];
        let v = c4::parse_table(rows, &c4::TableLayout::Kv, Path::new("doc"), &opts).unwrap();
        assert_eq!(v["k"], c4::Value::String("1,000.1".into()));
    }

    #[cfg(not(feature = "numeric"))]
    #[test]
    fn radix_literals_without_numeric_feature() {
        // auto: not a number, stays a string
        assert_eq!(csv_leaf("a,0xff", "a"), c4::Value::String("0xff".into()));
        // explicit numeric type: plain Rust parsing only → error
        let err = loader(vec![(c4::Format::Csv, "a,0xff,u8").into()])
            .load::<c4::Value>()
            .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    #[cfg(not(feature = "uuid"))]
    #[test]
    fn uuid_without_feature_is_unknown_type() {
        let err = loader(vec![
            (
                c4::Format::Csv,
                "a,550e8400-e29b-41d4-a716-446655440000,uuid",
            )
                .into(),
        ])
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
        let v: serde_json::Value =
            loader(vec![(c4::Format::Csv, "when,2024-01-02T03:04:05Z").into()])
                .load()
                .unwrap();
        assert_eq!(v, serde_json::json!({ "when": "2024-01-02T03:04:05Z" }));
    }

    #[cfg(feature = "datetime")]
    #[test]
    fn bad_dt_reports_row() {
        let err = loader(vec![(c4::Format::Csv, "when,tomorrow,datetime").into()])
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
        let err = loader(vec![(c4::Format::Csv, "ip,10.0.0.999,ipv4").into()])
            .load::<c4::Value>()
            .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 1, .. }));
    }

    #[test]
    fn bad_cell_reports_row() {
        // 999 does not fit i8; no header, so the bad row is row 1
        let err = loader(vec![fx("csv_bad/config").into()])
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
        ..crate::common::base()
    };
    let mut options = options;
    options.sources = vec![fx("jsonc/config").into()];
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
        ..crate::common::base()
    };
    check("ext_override", options);
}

#[cfg(feature = "jsonc")]
#[test]
fn format_spec_string_tuple_form() {
    // ("jsonc", ["json", "jsonc"]) — string id with custom extensions
    let options = c4::Options {
        formats: vec![("jsonc", ["json", "jsonc"]).into()],
        ..crate::common::base()
    };
    check("formats_tuple", options);
}

#[cfg(feature = "toml")]
#[test]
fn toml_basic() {
    check("toml_basic", base());
}

#[cfg(all(feature = "toml", feature = "datetime"))]
#[test]
fn toml_datetime_serializes_as_text() {
    // the trace format field is "dt" only with the datetime feature
    check("toml_datetime", base());
}

#[cfg(feature = "toml")]
#[test]
fn toml_datetime_follows_dt_feature() {
    use c4::{TracedValue, Value};

    let traced = loader(vec![fx("toml_datetime/config").into()])
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
    check("ini_basic", base());
}

#[cfg(feature = "env")]
#[test]
fn env_basic() {
    // `#` comments, optional `export`, quote stripping; values are
    // strings; no variable interpolation
    check("env_basic", base());
}

#[cfg(feature = "env")]
#[test]
fn env_extension_variants() {
    // `.env` itself (dotfile-as-extension rule) plus `local.env`;
    // ".env" < "local.env", so local.env overrides
    check("env_variants", base());
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
            // it here keeps the data rows positional (key,value[,format]),
            // exactly what parse_table expects
            .skip(1)
            .map(|l| {
                l.trim_matches('|')
                    .split('|')
                    .map(|cell| cell.trim().to_owned())
                    .collect()
            })
            .collect();
        c4::parse_table(rows, &c4::TableLayout::Kv, path, options)
    });
    // one list, one claim order: the defaults plus the custom format
    let mut formats = c4::Options::default().formats;
    formats.push(md.into());
    check(
        "custom_md",
        c4::Options {
            formats,
            ..crate::common::base()
        },
    );
}

// Spreadsheet formats: binary table formats over the same table stage.
// Fixtures are generated by tools/gen-sheets.
#[cfg(feature = "excel")]
mod excel {
    use crate::common::{base, check, expect, fx, loader};

    #[test]
    fn merge_mode_merges_non_ignored_sheets() {
        // app.xlsx: `config` is the only non-ignored sheet — `_notes`/
        // `#draft` (prefix) and `secret` (hidden) are skipped. zz_extra.xlsx
        // has only a `_`-prefixed sheet and contributes nothing.
        check("excel_basic", base());
    }

    #[test]
    fn merge_mode_merges_every_sheet_like_a_file() {
        // b.xlsx has visible sheets c, d (and #x/.y/_z/hidden, all
        // ignored). In merge mode both c and d parse and merge in `order`
        // by sheet name — being db arrays, the later sheet (d) wins.
        let value: serde_json::Value = loader(vec![fx("excel_tree/config/a/b.xlsx").into()])
            .load()
            .unwrap();
        assert_eq!(value, serde_json::json!([{ "k2": 2 }]));
    }

    #[test]
    fn hidden_config_sheet_contributes_nothing() {
        check("excel_hidden_config", base());
    }

    #[test]
    fn hidden_sheets_load_when_option_off() {
        let options = c4::Options {
            ignore_hidden_sheets: false,
            ..crate::common::base()
        };
        check("excel_hidden_config_off", options);
    }

    #[test]
    fn single_xlsx_file_source() {
        // a workbook as a single-file path source parses the same way
        let value: serde_json::Value = loader(vec![fx("excel_basic/config/app.xlsx").into()])
            .load()
            .unwrap();
        assert_eq!(value, expect("excel_basic"));
    }

    #[test]
    fn string_source_is_an_error() {
        // binary format — file sources only
        let err = loader(vec![(c4::Format::Excel, "name,c4").into()])
            .load::<c4::Value>()
            .unwrap_err();
        assert!(matches!(err, c4::Error::Parse { .. }));
    }

    #[test]
    fn physically_absent_type_row_is_all_auto() {
        // the workbook has no row 2 at all (writers don't materialize
        // an all-empty row): keys at row 1, records from row 3. The
        // padded-in blank row 2 is the type row (all auto) — it must
        // not skip as blank, which would consume the first record as
        // type ids.
        check("excel_blank_type_row", base());
    }

    #[test]
    fn bad_typed_cell_reports_the_real_spreadsheet_row() {
        // the grid starts at row 3 of the sheet, so its bad data cell
        // sits on row 5; padded leading rows keep Error::Table row
        // numbers aligned with the spreadsheet
        let err = loader(vec![fx("excel_bad/config").into()])
            .load::<c4::Value>()
            .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 5, .. }));
    }

    #[test]
    fn kv_shaped_sheet_under_the_db_default_fails_loudly() {
        // without the kv override the same workbook hits the eager
        // type-row validation (row 2 of the sheet)
        let err = loader(vec![fx("excel_kv_formats/config").into()])
            .load::<c4::Value>()
            .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 2, .. }));
    }

    #[test]
    fn formats_level_layout_reaches_the_config_sheet() {
        // spreadsheets default to db; (excel, ["xlsx"], "kv") flips a
        // kv-shaped config sheet back — the override direction opposite
        // to the format default
        let options = c4::Options {
            formats: vec![(c4::Format::Excel, ["xlsx"], "kv").into()],
            ..crate::common::base()
        };
        check("excel_kv_formats", options);
    }

    // serial datetime/date/time cells lower to ISO-ish text, then the
    // dt/date/time type ids validate them as usual
    #[cfg(feature = "datetime")]
    #[test]
    fn serial_datetime_cells() {
        check("excel_datetime", base());
    }

    // sheetname_as_key keys a workbook by sheet; folder/file keying nests
    // it under the file and folder names. Keying needs no Cargo feature.
    mod tree {
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

        #[test]
        fn every_sheet_becomes_a_key() {
            // a/b.xlsx sheets c,d → {a:{b:{c:…,d:…}}}; prefixed (#/./_)
            // and hidden sheets are ignored by default
            check("excel_tree", tree_options());
        }

        #[test]
        fn prefixed_sheets_load_when_option_off() {
            let options = c4::Options {
                ignore_commented_sheets: false,
                ..tree_options()
            };
            // #x/.y/_z become keys; the hidden sheet stays ignored
            check("excel_tree_prefix_off", options);
        }
    }
}

#[cfg(feature = "ods")]
mod ods {
    use crate::common::{base, check, loader};

    #[test]
    fn merge_mode_merges_non_ignored_sheets() {
        // `_notes` (prefix) and `hidden1` (table:display="false") are
        // ignored, leaving only `config` to parse
        check("ods_basic", base());
    }

    #[test]
    fn string_source_is_an_error() {
        let err = loader(vec![(c4::Format::Ods, "name,c4").into()])
            .load::<c4::Value>()
            .unwrap_err();
        assert!(matches!(err, c4::Error::Parse { .. }));
    }

    #[test]
    fn every_sheet_becomes_a_key() {
        let options = c4::Options {
            filename_as_key: true,
            dirname_as_key: true,
            sheetname_as_key: true,
            dir_depth: -1,
            ..base()
        };
        check("ods_tree", options);
    }
}

// Table layouts (spec: CLAUDE.md "Table stage"): kv is the default; db
// turns a grid (keys row, type row, records) into an array of record
// objects; a CustomLayout receives the lowered rows. Layouts are chosen
// per table source.
#[cfg(feature = "csv")]
mod table_layouts {
    use crate::common::{expect, expect_debug, fx, loader};

    fn check_table(case: &str, sources: Vec<c4::Source>) {
        let loader = loader(sources);
        let traced = serde_json::to_value(loader.trace().unwrap()).unwrap();
        assert_eq!(traced, expect_debug(case), "trace mismatch for {case}");
        let plain: serde_json::Value = loader.load().unwrap();
        assert_eq!(plain, expect(case), "load mismatch for {case}");
    }

    #[test]
    fn db_layout_grid_becomes_records() {
        // row 1 = keys, row 2 = type ids (empty cell = auto), rest =
        // records; empty cells are omitted from their record and a
        // dotted key column nests per record
        check_table(
            "csv_db",
            vec![(c4::Format::Csv, fx("csv_db/config/data.csv"), "db").into()],
        );
    }

    #[test]
    fn db_without_a_type_row_is_a_custom_layout() {
        // db always expects the type row; a grid without one uses the
        // canonical CustomLayout pattern — insert a row of `auto` cells
        // after the header and delegate to the Db layout (also exercises
        // the format-id string form of the 3-tuple)
        let no_types = c4::CustomLayout::new("db-no-types", |mut rows, path, options| {
            let width = rows.first().map(Vec::len).unwrap_or(0);
            rows.insert(1, vec!["auto".into(); width]);
            c4::parse_table(rows, &c4::TableLayout::Db, path, options)
        });
        check_table(
            "csv_db_no_types",
            vec![("csv", fx("csv_db_no_types/config/data.csv"), no_types).into()],
        );
    }

    #[test]
    fn db_all_blank_type_row_is_all_auto() {
        // the type row is positional — the row right after the keys is
        // the type row even when entirely blank (= every column auto);
        // it must not skip as a blank row, which would consume the
        // first record as type ids. Blank rows before the keys and
        // between records still skip.
        check_table(
            "csv_db_blank_types",
            vec![
                (
                    c4::Format::Csv,
                    fx("csv_db_blank_types/config/data.csv"),
                    "db",
                )
                    .into(),
            ],
        );
    }

    #[test]
    fn db_bad_cell_reports_its_row() {
        let err = loader(vec![
            (c4::Format::Csv, fx("csv_db_bad/config/data.csv"), "db").into(),
        ])
        .load::<c4::Value>()
        .unwrap_err();
        assert!(matches!(err, c4::Error::Table { row: 3, .. }));
    }

    #[test]
    fn formats_level_layout_applies_to_claimed_files() {
        // (csv, ["csv"], "db") in Options.formats: every file the entry
        // claims — here a whole folder scan — parses as a record grid
        let options = c4::Options {
            formats: vec![(c4::Format::Csv, ["csv"], "db").into()],
            ..crate::common::base()
        };
        crate::common::check("csv_db_formats", options);
    }

    #[test]
    fn db_type_row_is_validated_eagerly() {
        // a kv-shaped file under the db layout fails loudly at the type
        // row (instead of silently parsing to an empty array), with a
        // hint pointing at the kv layout
        let err = loader(vec![
            (c4::Format::Csv, fx("csv_db_bad/config/types.csv"), "db").into(),
        ])
        .load::<c4::Value>()
        .unwrap_err();
        let c4::Error::Table { row, message, .. } = err else {
            panic!("expected Table, got {err:?}");
        };
        assert_eq!(row, 2);
        assert!(message.contains("kv"), "hint missing: {message}");
    }

    #[test]
    fn csv_source_cannot_name_a_sheet() {
        let err = loader(vec![
            (
                c4::Format::Csv,
                fx("csv_db/config/data.csv"),
                "sheet1",
                "db",
            )
                .into(),
        ])
        .load::<c4::Value>()
        .unwrap_err();
        assert!(matches!(err, c4::Error::Parse { .. }));
    }
}

// Sheet-targeted table sources: several sources may point at the same
// workbook, each naming a sheet and a layout; the parsed value merges
// under the sheet name (spec: CLAUDE.md "Spreadsheet formats").
#[cfg(feature = "excel")]
mod excel_sheet_sources {
    use crate::common::{expect, expect_debug, fx, loader};

    #[test]
    fn each_sheet_gets_its_own_layout() {
        let path = fx("excel_sheets/config/game.xlsx");
        // CustomLayouts work on the lowered rows: `no_types` handles a db
        // grid without a type row (insert `auto`s, delegate to Db) …
        let no_types = c4::CustomLayout::new("db-no-types", |mut rows, path, options| {
            let width = rows.first().map(Vec::len).unwrap_or(0);
            rows.insert(1, vec!["auto".into(); width]);
            c4::parse_table(rows, &c4::TableLayout::Db, path, options)
        });
        // … and `transpose` rotates a column-oriented sheet into kv rows
        let transpose = c4::CustomLayout::new("transpose", |rows, path, options| {
            let cols = rows.first().map(Vec::len).unwrap_or(0);
            let kv = (0..cols)
                .map(|i| rows.iter().filter_map(|row| row.get(i).cloned()).collect())
                .collect();
            c4::parse_table(kv, &c4::TableLayout::Kv, path, options)
        });
        let loader = loader(vec![
            // sheet sources are 4-tuples: sheet name + explicit layout
            (c4::Format::Excel, path.clone(), "config", "kv").into(),
            (c4::Format::Excel, path.clone(), "items", "db").into(),
            (c4::Format::Excel, path.clone(), "npcs", no_types).into(),
            (c4::Format::Excel, path.clone(), "meta", transpose).into(),
            // naming a sheet bypasses the prefix/hidden filters
            (c4::Format::Excel, path, "_extra", "kv").into(),
        ]);
        assert_eq!(
            serde_json::to_value(loader.trace().unwrap()).unwrap(),
            expect_debug("excel_sheets")
        );
        let plain: serde_json::Value = loader.load().unwrap();
        assert_eq!(plain, expect("excel_sheets"));
    }

    #[test]
    fn missing_sheet_is_an_error() {
        let err = loader(vec![
            (
                c4::Format::Excel,
                fx("excel_sheets/config/game.xlsx"),
                "nope",
                "kv",
            )
                .into(),
        ])
        .load::<c4::Value>()
        .unwrap_err();
        assert!(matches!(err, c4::Error::Parse { .. }));
    }
}

// `array` is always compiled (a native split) — it works through
// `parse_table` with no format feature at all.
#[test]
fn array_type_id_needs_no_feature() {
    let rows = vec![vec![
        "tags".to_string(),
        "a|b|c".to_string(),
        "array|".to_string(),
    ]];
    let value = c4::parse_table(
        rows,
        &c4::TableLayout::Kv,
        std::path::Path::new("mem"),
        &c4::Options::default(),
    )
    .unwrap();
    assert_eq!(
        value["tags"],
        c4::Value::Array(vec![
            c4::Value::String("a".into()),
            c4::Value::String("b".into()),
            c4::Value::String("c".into()),
        ])
    );
}

// without the `csv` feature the `csv` list id is unknown, even via
// `parse_table` (which is always compiled).
#[cfg(not(feature = "csv"))]
#[test]
fn csv_type_id_without_csv_feature_is_unknown() {
    let rows = vec![vec!["m".to_string(), "x".to_string(), "csv".to_string()]];
    let err = c4::parse_table(
        rows,
        &c4::TableLayout::Kv,
        std::path::Path::new("mem"),
        &c4::Options::default(),
    )
    .unwrap_err();
    assert!(matches!(err, c4::Error::Table { row: 1, .. }));
}

// ---- dot_key array segments: `name[]` appends, `name[<int>]` indexes ----
// `parse_table` is always compiled, so these run under every feature set.

fn kv_rows(rows: &[[&str; 2]], options: &c4::Options) -> c4::Value {
    let rows = rows
        .iter()
        .map(|row| row.iter().map(|cell| cell.to_string()).collect())
        .collect();
    c4::parse_table(
        rows,
        &c4::TableLayout::Kv,
        std::path::Path::new("mem"),
        options,
    )
    .unwrap()
}

#[test]
fn kv_index_keys_build_a_sorted_array() {
    let value = kv_rows(
        &[["a[0].b", "1"], ["a[2].c", "3"], ["a[1].c", "2"]],
        &c4::Options::default(),
    );
    assert_eq!(value["a"].as_array().unwrap().len(), 3);
    assert_eq!(value["a"][0]["b"].as_i64(), Some(1));
    assert_eq!(value["a"][1]["c"].as_i64(), Some(2));
    assert_eq!(value["a"][2]["c"].as_i64(), Some(3));
}

#[test]
fn kv_append_keys_push_one_element_per_row() {
    let value = kv_rows(
        &[
            ["ports[]", "80"],
            ["ports[]", "443"],
            ["servers[].host", "a"],
            ["servers[].host", "b"],
        ],
        &c4::Options::default(),
    );
    assert_eq!(
        value["ports"],
        c4::Value::Array(vec![c4::Value::Int(80), c4::Value::Int(443)])
    );
    // each `[]` occurrence appends — two rows give two elements, they
    // never merge into one
    assert_eq!(value["servers"].as_array().unwrap().len(), 2);
    assert_eq!(value["servers"][0]["host"].as_str(), Some("a"));
    assert_eq!(value["servers"][1]["host"].as_str(), Some("b"));
}

#[test]
fn kv_same_index_deep_merges_into_one_element() {
    let value = kv_rows(&[["a[0].b", "1"], ["a[0].c", "2"]], &c4::Options::default());
    assert_eq!(value["a"].as_array().unwrap().len(), 1);
    assert_eq!(value["a"][0]["b"].as_i64(), Some(1));
    assert_eq!(value["a"][0]["c"].as_i64(), Some(2));
}

#[test]
fn kv_index_gaps_stay_null() {
    // skipped indexes leave Null gaps: a[1] + a[4] → 5 elements —
    // deserialize such arrays as Vec<Option<T>>
    let value = kv_rows(&[["a[1]", "1"], ["a[4]", "4"]], &c4::Options::default());
    assert_eq!(
        value["a"],
        c4::Value::Array(vec![
            c4::Value::Null,
            c4::Value::Int(1),
            c4::Value::Null,
            c4::Value::Null,
            c4::Value::Int(4),
        ])
    );
    // leading zeros parse as the same index
    let value = kv_rows(&[["b[01]", "5"]], &c4::Options::default());
    assert_eq!(value["b"][1].as_i64(), Some(5));
    assert!(value["b"][0].is_null());
}

#[test]
fn kv_chained_suffixes_nest_arrays() {
    // each suffix is one nesting level: m[1][2] → {m: [null, [null, null, 9]]}
    let value = kv_rows(&[["m[1][2]", "9"]], &c4::Options::default());
    assert!(value["m"][0].is_null());
    assert_eq!(value["m"][1][2].as_i64(), Some(9));
    // g[][] appends a new inner array per occurrence
    let value = kv_rows(&[["g[][]", "1"], ["g[][]", "2"]], &c4::Options::default());
    assert_eq!(value["g"][0][0].as_i64(), Some(1));
    assert_eq!(value["g"][1][0].as_i64(), Some(2));
    // h[0][] appends inside element 0
    let value = kv_rows(&[["h[0][]", "1"], ["h[0][]", "2"]], &c4::Options::default());
    assert_eq!(value["h"][0][0].as_i64(), Some(1));
    assert_eq!(value["h"][0][1].as_i64(), Some(2));
    // chains keep walking the dotted path
    let value = kv_rows(&[["m[0][0].v", "5"]], &c4::Options::default());
    assert_eq!(value["m"][0][0]["v"].as_i64(), Some(5));
}

#[test]
fn kv_suffixes_chain_through_the_path() {
    let value = kv_rows(
        &[["a[0].b[].c", "1"], ["a[0].b[].c", "2"]],
        &c4::Options::default(),
    );
    assert_eq!(value["a"][0]["b"][0]["c"].as_i64(), Some(1));
    assert_eq!(value["a"][0]["b"][1]["c"].as_i64(), Some(2));
}

#[test]
fn only_valid_suffix_shapes_make_arrays() {
    let options = c4::Options::default();
    for key in [
        "[]",
        "[3]",
        "[0][1]", // no base name
        "a[x]",
        "a[-1]",
        "a[]b",
        "a[1]x[2]", // groups must run back-to-back to the end
        "a[[1]]",
        "a[99999999999999999999]", // does not fit usize
    ] {
        let value = kv_rows(&[[key, "1"]], &options);
        // the whole segment stays a literal object key
        assert_eq!(value[key].as_i64(), Some(1), "key {key:?}");
    }
}

#[test]
fn dot_key_off_keeps_array_suffixes_literal() {
    let options = c4::Options {
        dot_key: false,
        ..crate::common::base()
    };
    let value = kv_rows(&[["a[].b", "1"], ["a[0]", "2"]], &options);
    assert_eq!(value["a[].b"].as_i64(), Some(1));
    assert_eq!(value["a[0]"].as_i64(), Some(2));
}

#[test]
fn array_kind_collisions_later_row_wins() {
    // an array suffix over a non-array replaces it with an array
    let value = kv_rows(&[["a.b", "1"], ["a[]", "2"]], &c4::Options::default());
    assert_eq!(value["a"], c4::Value::Array(vec![c4::Value::Int(2)]));
    // a plain segment descending into an array replaces it with an object
    let value = kv_rows(&[["a[]", "1"], ["a.b", "2"]], &c4::Options::default());
    assert_eq!(value["a"]["b"].as_i64(), Some(2));
}

#[test]
fn db_append_columns_push_per_record() {
    let rows: Vec<Vec<String>> = vec![
        vec!["a[].b".into(), "a[].c".into()],
        vec![String::new(), String::new()], // type row: all auto
        vec!["1".into(), "2".into()],
        vec!["3".into(), "4".into()],
    ];
    let value = c4::parse_table(
        rows,
        &c4::TableLayout::Db,
        std::path::Path::new("mem"),
        &c4::Options::default(),
    )
    .unwrap();
    // each record grows its own array — one element per `[]` column
    assert_eq!(value[0]["a"][0]["b"].as_i64(), Some(1));
    assert_eq!(value[0]["a"][1]["c"].as_i64(), Some(2));
    assert_eq!(value[1]["a"][0]["b"].as_i64(), Some(3));
    assert_eq!(value[1]["a"][1]["c"].as_i64(), Some(4));
}

#[cfg(feature = "env")]
#[test]
fn env_keys_take_array_suffixes() {
    let value: c4::Value = common::loader(vec![("env", "A[]=1\nA[]=2\nB[0].C=x").into()])
        .load()
        .unwrap();
    assert_eq!(value["A"][0].as_str(), Some("1")); // env values stay strings
    assert_eq!(value["A"][1].as_str(), Some("2"));
    assert_eq!(value["B"][0]["C"].as_str(), Some("x"));
}

#[cfg(feature = "csv")]
#[test]
fn csv_array_key() {
    common::check("csv_array_key", base());
}
