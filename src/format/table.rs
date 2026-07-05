//! The generic table subsystem: turns a plain row table
//! (`[[key, value, format], …]`, produced by a table-shaped format module
//! such as the csv one, or by a user-defined format via
//! [`crate::parse_table`]) into a config value. Handles the header
//! options, the format column, `dot_key` expansion and row-order merging.
//!
//! Each typed cell picks a parser. Rust-built-in ones (`i8`–`u64`,
//! `f32`/`f64`, `bool`, `str`) always exist; `dt` exists only with the
//! `datetime` feature, `date`/`time`/`ipv4`/`ipv6`/`inet`/`cidr`/
//! `macaddr`/`macaddr8`/`uuid` with their same-named features, and
//! `json` only with the `json` format feature — without
//! the feature the id is an unknown type and the row errors. Only `auto`
//! degrades (it just stops guessing shapes whose feature is off).

use std::path::Path;

#[cfg(any(
    feature = "datetime",
    feature = "date",
    feature = "time",
    feature = "inet",
    feature = "cidr",
    feature = "macaddr",
    feature = "macaddr8",
    feature = "uuid"
))]
use crate::valid;
use crate::{Error, Options, Result, TableOptions, Value};

/// Row numbers in errors are 1-based indices into `rows` — the header
/// row, when enabled, is row 1.
pub(crate) fn parse(rows: Vec<Vec<String>>, path: &Path, options: &Options) -> Result<Value> {
    let table = &options.table;
    let mut rows = rows.into_iter().enumerate();

    // locate the key / value / format columns
    let (key_col, value_col, type_col) = if table.header {
        let (_, header) = rows
            .next()
            .ok_or_else(|| table_err(path, 1, "missing header row".into()))?;
        let find = |name: &str| header.iter().position(|cell| cell.trim() == name);
        let key_col = find(&table.columns.key)
            .ok_or_else(|| table_err(path, 1, format!("missing column '{}'", table.columns.key)))?;
        let value_col = find(&table.columns.value).ok_or_else(|| {
            table_err(path, 1, format!("missing column '{}'", table.columns.value))
        })?;
        (key_col, value_col, find(&table.columns.format))
    } else {
        (0, 1, Some(2))
    };

    let mut root = Value::Object(Default::default());
    for (index, cells) in rows {
        let row = index + 1;
        if cells.iter().all(|cell| cell.is_empty()) {
            continue; // blank row
        }
        let key = cells
            .get(key_col)
            .ok_or_else(|| table_err(path, row, "missing key cell".into()))?
            .trim();
        let raw = cells
            .get(value_col)
            .ok_or_else(|| table_err(path, row, "missing value cell".into()))?;
        let declared = type_col
            .and_then(|i| cells.get(i))
            .map(|s| s.trim())
            .unwrap_or("");
        let value = convert(raw, declared, table, path, row)?;
        super::deep_merge(&mut root, super::expand_key(key, value, options.dot_key));
    }
    Ok(root)
}

fn convert(
    raw: &str,
    declared: &str,
    table: &TableOptions,
    path: &Path,
    row: usize,
) -> Result<Value> {
    // aliases first, then concrete types
    let ty = match declared {
        "" | "auto" => return Ok(auto(raw)),
        "int" | "integer" => "i64",
        "uint" => "u64",
        "float" | "double" | "number" => "f64",
        "string" | "text" => "str",
        "boolean" => "bool",
        "datetime" => "dt",
        other => other,
    };

    let fail = |message: String| table_err(path, row, message);
    // every "the value does not fit its declared format" failure shares
    // this one message shape
    let bad = || fail(format!("'{raw}' is not a valid {ty}"));
    let int = |min: i64, max: i64| {
        int_literal(raw)
            .filter(|v| (min..=max).contains(v))
            .map(Value::Int)
            .ok_or_else(&bad)
    };
    let uint = |max: u64| {
        uint_literal(raw)
            .filter(|v| *v <= max)
            .map(Value::Uint)
            .ok_or_else(&bad)
    };

    match ty {
        "i8" => int(i8::MIN.into(), i8::MAX.into()),
        "i16" => int(i16::MIN.into(), i16::MAX.into()),
        "i32" => int(i32::MIN.into(), i32::MAX.into()),
        "i64" => int(i64::MIN, i64::MAX),
        "u8" => uint(u8::MAX.into()),
        "u16" => uint(u16::MAX.into()),
        "u32" => uint(u32::MAX.into()),
        "u64" => uint(u64::MAX),
        "f32" => float_literal(raw)
            .map(|f| Value::Float(f64::from(f as f32)))
            .ok_or_else(&bad),
        "f64" => float_literal(raw).map(Value::Float).ok_or_else(&bad),
        "bool" => match raw {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err(fail(format!("'{raw}' is not a valid bool"))),
        },
        "str" => Ok(Value::String(raw.to_owned())),
        // without its feature each of these ids is cfg'd out of the match
        // and lands on the unknown-type error below; the shape rules live
        // in `crate::valid`
        #[cfg(feature = "datetime")]
        "dt" => valid::datetime(raw)
            .then(|| Value::DateTime(raw.to_owned()))
            .ok_or_else(&bad),
        #[cfg(feature = "date")]
        "date" => valid::date(raw)
            .then(|| Value::Date(raw.to_owned()))
            .ok_or_else(&bad),
        #[cfg(feature = "time")]
        "time" => valid::time(raw)
            .then(|| Value::Time(raw.to_owned()))
            .ok_or_else(&bad),
        #[cfg(feature = "ipv4")]
        "ipv4" => raw
            .parse::<std::net::Ipv4Addr>()
            .map(Value::Ipv4)
            .map_err(|_| bad()),
        #[cfg(feature = "ipv6")]
        "ipv6" => raw
            .parse::<std::net::Ipv6Addr>()
            .map(Value::Ipv6)
            .map_err(|_| bad()),
        // PostgreSQL semantics: a bare inet maps onto the address
        // variants; with a netmask (host bits allowed) it stays text
        #[cfg(feature = "inet")]
        "inet" => match raw.parse::<std::net::IpAddr>() {
            Ok(ip) => Ok(ip_value(ip)),
            Err(_) => valid::inet(raw)
                .then(|| Value::Inet(raw.to_owned()))
                .ok_or_else(&bad),
        },
        #[cfg(feature = "cidr")]
        "cidr" => valid::cidr(raw)
            .then(|| Value::Cidr(raw.to_owned()))
            .ok_or_else(&bad),
        #[cfg(feature = "macaddr")]
        "macaddr" => valid::macaddr(raw)
            .then(|| Value::MacAddr(raw.to_owned()))
            .ok_or_else(&bad),
        #[cfg(feature = "macaddr8")]
        "macaddr8" => valid::macaddr8(raw)
            .then(|| Value::MacAddr8(raw.to_owned()))
            .ok_or_else(&bad),
        #[cfg(feature = "uuid")]
        "uuid" => valid::uuid(raw)
            .then(|| Value::Uuid(raw.to_owned()))
            .ok_or_else(&bad),
        "null" if table.types.null => Ok(Value::Null), // value cell is ignored
        #[cfg(feature = "json")]
        "json" if table.types.json => serde_json::from_str::<serde_json::Value>(raw)
            .map(super::from_serde_json)
            .map_err(|_| bad()),
        _ if ty.starts_with("arr:") && table.types.array => {
            let element = &ty[4..];
            if matches!(element, "null" | "json") || element.starts_with("arr:") {
                return Err(fail(format!("'{element}' is not a scalar element type")));
            }
            raw.split(table.delimiter)
                .map(|cell| convert(cell, element, table, path, row))
                .collect::<Result<Vec<_>>>()
                .map(Value::Array)
        }
        _ => Err(fail(format!("unknown or disabled type '{declared}'"))),
    }
}

/// Auto-detection: bool, then each enabled value-parser type in the
/// order date → time → dt → uuid (hyphenated) → macaddr → macaddr8
/// (pair spellings) → ipv4 → ipv6 → cidr → inet (masked forms only,
/// cheap shape scans before parser-backed guesses, strict before
/// loose), then i64, then u64, then f64, otherwise string. Integers
/// with leading zeros (`007`) stay strings.
pub(crate) fn auto(raw: &str) -> Value {
    match raw {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    // cheap fixed-shape scans first, parser-backed guesses after; the
    // strict types (ipv4/ipv6/cidr) run before inet, which only catches
    // masked addresses with host bits set (e.g. 1.1.1.1/24)
    #[cfg(feature = "date")]
    if valid::date(raw) {
        return Value::Date(raw.to_owned());
    }
    #[cfg(feature = "time")]
    if valid::time(raw) {
        return Value::Time(raw.to_owned());
    }
    #[cfg(feature = "datetime")]
    if valid::datetime(raw) {
        return Value::DateTime(raw.to_owned());
    }
    #[cfg(feature = "uuid")]
    if valid::uuid_hyphenated(raw) {
        return Value::Uuid(raw.to_owned());
    }
    #[cfg(feature = "macaddr")]
    if valid::macaddr_pairs(raw) {
        return Value::MacAddr(raw.to_owned());
    }
    #[cfg(feature = "macaddr8")]
    if valid::macaddr8_pairs(raw) {
        return Value::MacAddr8(raw.to_owned());
    }
    #[cfg(feature = "ipv4")]
    if let Ok(ip) = raw.parse::<std::net::Ipv4Addr>() {
        return Value::Ipv4(ip);
    }
    #[cfg(feature = "ipv6")]
    if let Ok(ip) = raw.parse::<std::net::Ipv6Addr>() {
        return Value::Ipv6(ip);
    }
    // masked forms only guess when a mask is present; no bare-inet guess
    // is needed because `inet` implies `ipv4` + `ipv6`
    #[cfg(feature = "cidr")]
    if raw.contains('/') && valid::cidr(raw) {
        return Value::Cidr(raw.to_owned());
    }
    #[cfg(feature = "inet")]
    if raw.contains('/') && valid::inet(raw) {
        return Value::Inet(raw.to_owned());
    }
    if let Some(number) = auto_number(raw) {
        return number;
    }
    Value::String(raw.to_owned())
}

/// The number leg of `auto`: leading-zero decimals never convert, and
/// with the `numeric` feature the extended literal forms apply.
fn auto_number(raw: &str) -> Option<Value> {
    #[cfg(feature = "numeric")]
    let (cleaned, bigint) = clean_literal(raw)?;
    #[cfg(not(feature = "numeric"))]
    let (cleaned, bigint) = (raw.to_owned(), false);

    #[cfg(feature = "numeric")]
    if let Some((digits, radix)) = split_radix(&cleaned) {
        return i64::from_str_radix(&digits, radix)
            .ok()
            .map(Value::Int)
            .or_else(|| u64::from_str_radix(&digits, radix).ok().map(Value::Uint));
    }

    let body = cleaned.strip_prefix('-').unwrap_or(&cleaned);
    let leading_zero =
        body.len() > 1 && body.starts_with('0') && body.as_bytes()[1].is_ascii_digit();
    let numeric_shape = !body.is_empty()
        && body.chars().any(|c| c.is_ascii_digit())
        && body
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'));
    if leading_zero || !numeric_shape {
        return None;
    }
    if let Ok(i) = cleaned.parse::<i64>() {
        return Some(Value::Int(i));
    }
    if let Ok(u) = cleaned.parse::<u64>() {
        return Some(Value::Uint(u));
    }
    if bigint {
        return None; // a BigInt beyond u64 cannot be represented
    }
    cleaned.parse::<f64>().ok().map(Value::Float)
}

/// Validate and strip ES2021 `_` separators (each must sit between two
/// hex digits) and a BigInt-style trailing `n` (stripped only when the
/// rest looks like an integer literal). `None` = not a numeric literal.
#[cfg(feature = "numeric")]
fn clean_literal(raw: &str) -> Option<(String, bool)> {
    let b = raw.as_bytes();
    for (i, &c) in b.iter().enumerate() {
        if c == b'_' {
            let between_digits = i > 0
                && i + 1 < b.len()
                && b[i - 1].is_ascii_hexdigit()
                && b[i + 1].is_ascii_hexdigit();
            if !between_digits {
                return None;
            }
        }
    }
    let mut s: String = raw.chars().filter(|&c| c != '_').collect();
    let bigint = s.ends_with('n')
        && s.len() > 1
        && s[..s.len() - 1]
            .bytes()
            .all(|c| c.is_ascii_hexdigit() || matches!(c, b'x' | b'X' | b'o' | b'O' | b'+' | b'-'));
    if bigint {
        s.truncate(s.len() - 1);
    }
    Some((s, bigint))
}

/// `[sign]0x/0b/0o` prefix → (sign + digits, radix).
#[cfg(feature = "numeric")]
fn split_radix(s: &str) -> Option<(String, u32)> {
    let (sign, body) = match s.strip_prefix('-') {
        Some(body) => ("-", body),
        None => ("", s.strip_prefix('+').unwrap_or(s)),
    };
    let (digits, radix) =
        if let Some(d) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
            (d, 16)
        } else if let Some(d) = body.strip_prefix("0b").or_else(|| body.strip_prefix("0B")) {
            (d, 2)
        } else if let Some(d) = body.strip_prefix("0o").or_else(|| body.strip_prefix("0O")) {
            (d, 8)
        } else {
            return None;
        };
    if digits.is_empty() {
        return None;
    }
    Some((format!("{sign}{digits}"), radix))
}

/// String → i64, honoring the `numeric` extended literal forms.
fn int_literal(raw: &str) -> Option<i64> {
    #[cfg(feature = "numeric")]
    {
        let (s, _) = clean_literal(raw)?;
        if let Some((digits, radix)) = split_radix(&s) {
            return i64::from_str_radix(&digits, radix).ok();
        }
        s.parse().ok()
    }
    #[cfg(not(feature = "numeric"))]
    raw.parse().ok()
}

/// String → u64, honoring the `numeric` extended literal forms.
fn uint_literal(raw: &str) -> Option<u64> {
    #[cfg(feature = "numeric")]
    {
        let (s, _) = clean_literal(raw)?;
        if let Some((digits, radix)) = split_radix(&s) {
            return u64::from_str_radix(&digits, radix).ok();
        }
        s.parse().ok()
    }
    #[cfg(not(feature = "numeric"))]
    raw.parse().ok()
}

/// String → f64; radix and BigInt forms convert through integers.
fn float_literal(raw: &str) -> Option<f64> {
    #[cfg(feature = "numeric")]
    {
        let (s, bigint) = clean_literal(raw)?;
        if let Some((digits, radix)) = split_radix(&s) {
            return i64::from_str_radix(&digits, radix)
                .ok()
                .map(|v| v as f64)
                .or_else(|| u64::from_str_radix(&digits, radix).ok().map(|v| v as f64));
        }
        if bigint {
            return s
                .parse::<i64>()
                .ok()
                .map(|v| v as f64)
                .or_else(|| s.parse::<u64>().ok().map(|v| v as f64));
        }
        s.parse().ok()
    }
    #[cfg(not(feature = "numeric"))]
    raw.parse().ok()
}

/// The `inet` type and auto guess: either IP family, mapped onto the
/// existing ipv4/ipv6 variants.
#[cfg(feature = "inet")]
fn ip_value(ip: std::net::IpAddr) -> Value {
    match ip {
        std::net::IpAddr::V4(v4) => Value::Ipv4(v4),
        std::net::IpAddr::V6(v6) => Value::Ipv6(v6),
    }
}

fn table_err(path: &Path, row: usize, message: String) -> Error {
    Error::Table {
        path: path.to_path_buf(),
        row,
        message,
    }
}
