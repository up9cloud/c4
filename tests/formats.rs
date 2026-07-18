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

    // a `json` cell holds a whole JSON document (array, object, null, …);
    // cell format ids mirror the file formats one-to-one, so `json`
    // parses with the strict json parser and needs the `json` feature
    #[cfg(feature = "json")]
    #[test]
    fn json_cell_holds_a_document() {
        check("csv_extended", c4::Options::default());
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
        check("csv_list", c4::Options::default());
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
            ..c4::Options::default()
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
            ..c4::Options::default()
        })
        .load()
        .unwrap();
        assert_eq!(value["a"].as_i64(), Some(1)); // explicit int
        assert_eq!(value["b"].as_str(), Some("1.1.1.1")); // ipv4-validated, canonical string
        assert_eq!(value["c"].as_u64(), Some(8080)); // auto (no format cell)

        // the ipv4 format actually validates — a bad address errors
        let bad = c4::Loader::new(c4::Options {
            sources: vec![(csv_t, "b\n999.1.1.1\nipv4").into()],
            ..c4::Options::default()
        })
        .load::<c4::Value>();
        assert!(matches!(bad, Err(c4::Error::Table { row: 1, .. })));
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
        ..c4::Options::default()
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
            ..c4::Options::default()
        },
    );
}

// Spreadsheet formats: binary table formats over the same table stage.
// Fixtures are generated by tools/gen-sheets.
#[cfg(feature = "excel")]
mod excel {
    use crate::common::{check, expect, fx, loader};

    #[test]
    fn merge_mode_reads_only_the_config_sheet() {
        // app.xlsx: `config` parses; `notes` (other name), `#draft`
        // (prefix) and `secret` (hidden) are ignored. zz_extra.xlsx has
        // no `config` sheet and contributes nothing.
        check("excel_basic", c4::Options::default());
    }

    #[test]
    fn hidden_config_sheet_contributes_nothing() {
        check("excel_hidden_config", c4::Options::default());
    }

    #[test]
    fn hidden_sheets_load_when_option_off() {
        let options = c4::Options {
            ignore_hidden_sheets: false,
            ..c4::Options::default()
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
        check("excel_blank_type_row", c4::Options::default());
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
            ..c4::Options::default()
        };
        check("excel_kv_formats", options);
    }

    // serial datetime/date/time cells lower to ISO-ish text, then the
    // dt/date/time type ids validate them as usual
    #[cfg(feature = "datetime")]
    #[test]
    fn serial_datetime_cells() {
        check("excel_datetime", c4::Options::default());
    }

    #[cfg(feature = "tree")]
    mod tree {
        use crate::common::check;

        fn tree_options() -> c4::Options {
            c4::Options {
                tree: true,
                ..c4::Options::default()
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
                ignore_sheet_prefix: false,
                ..tree_options()
            };
            // #x/.y/_z become keys; the hidden sheet stays ignored
            check("excel_tree_prefix_off", options);
        }
    }
}

#[cfg(feature = "ods")]
mod ods {
    use crate::common::{check, loader};

    #[test]
    fn merge_mode_reads_only_the_config_sheet() {
        // `_notes` (prefix) and `hidden1` (table:display="false") are
        // ignored; only `config` parses
        check("ods_basic", c4::Options::default());
    }

    #[test]
    fn string_source_is_an_error() {
        let err = loader(vec![(c4::Format::Ods, "name,c4").into()])
            .load::<c4::Value>()
            .unwrap_err();
        assert!(matches!(err, c4::Error::Parse { .. }));
    }

    #[cfg(feature = "tree")]
    #[test]
    fn every_sheet_becomes_a_key() {
        let options = c4::Options {
            tree: true,
            ..c4::Options::default()
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
            ..c4::Options::default()
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
