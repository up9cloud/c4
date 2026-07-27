//! Excel (`xlsx`/`xlsm`/`xlsb`/`xls`) and OpenDocument (`ods`)
//! spreadsheets — the binary table-shaped formats, read with calamine.
//! Each selected sheet lowers into positional rows anchored at cell A1
//! (leading empty rows/columns are padded in, so column A is always the
//! key and `Error::Table` row numbers are real spreadsheet rows) and
//! feeds the generic table stage.
//!
//! Sheet selection: non-worksheet sheets (chart/dialog/macro/VBA) are
//! always skipped; `Options.ignore_hidden_sheets` and
//! `Options.ignore_commented_sheetnames` filter the rest. Each remaining
//! sheet is treated like a file: with `sheetname_as_key: false` they all
//! deep-merge into one value (in `order` by sheet name); with
//! `sheetname_as_key: true` the workbook becomes an object keyed by sheet
//! name (through `insert_key`, so `dot_key` expands a dotted name). No
//! sheets left → the workbook contributes nothing.

use std::collections::BTreeMap;
use std::fmt::Display;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use calamine::{Data, ExcelDateTime, Reader, Sheet, SheetType, SheetVisible, Sheets};

use crate::{Error, Options, Result, TableLayout, Value};

#[cfg(feature = "excel")]
pub(crate) fn parse_excel(
    path: &Path,
    sheet_name: Option<&str>,
    layout: &TableLayout,
    options: &Options,
) -> Result<Value> {
    // the reader follows the actual file extension, so a remapped
    // extension still parses (as xlsx, the modern default)
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase);
    let workbook = match ext.as_deref() {
        Some("xls") => Sheets::Xls(calamine::open_workbook(path).map_err(open_err(path))?),
        Some("xlsb") => Sheets::Xlsb(calamine::open_workbook(path).map_err(open_err(path))?),
        _ => Sheets::Xlsx(calamine::open_workbook(path).map_err(open_err(path))?),
    };
    parse_workbook(workbook, path, sheet_name, layout, options)
}

#[cfg(feature = "ods")]
pub(crate) fn parse_ods(
    path: &Path,
    sheet_name: Option<&str>,
    layout: &TableLayout,
    options: &Options,
) -> Result<Value> {
    let workbook = Sheets::Ods(calamine::open_workbook(path).map_err(open_err(path))?);
    parse_workbook(workbook, path, sheet_name, layout, options)
}

fn open_err<E: Display>(path: &Path) -> impl Fn(E) -> Error + '_ {
    move |e| Error::Parse {
        path: path.to_path_buf(),
        message: e.to_string(),
    }
}

fn parse_workbook(
    mut workbook: Sheets<BufReader<File>>,
    path: &Path,
    sheet_name: Option<&str>,
    layout: &TableLayout,
    options: &Options,
) -> Result<Value> {
    let sheets: Vec<Sheet> = workbook.sheets_metadata().to_vec();

    // a table source that names a sheet reads exactly that sheet —
    // explicit wins, so the ignore filters do not apply and a missing
    // sheet is an error — and keys the result by the sheet name (so
    // several sources can each read one sheet of the same workbook)
    if let Some(name) = sheet_name {
        if !sheets.iter().any(|sheet| sheet.name == name) {
            return Err(Error::Parse {
                path: path.to_path_buf(),
                message: format!("sheet '{name}' not found"),
            });
        }
        let value = parse_sheet(&mut workbook, name, layout, path, options)?;
        return Ok(sheet_key(name, value, options));
    }

    // every non-ignored sheet parses; a workbook with none contributes
    // nothing (Null)
    let mut names: Vec<String> = sheets
        .into_iter()
        .filter(|sheet| keep(sheet, options))
        .map(|sheet| sheet.name)
        .collect();
    if options.sheetname_as_key {
        if names.is_empty() {
            return Ok(Value::Null);
        }
        // each sheet is a key (names are unique, so order is immaterial)
        let mut root = Value::Object(BTreeMap::new());
        for name in names {
            let value = parse_sheet(&mut workbook, &name, layout, path, options)?;
            super::insert_key(&mut root, &name, value, options.dot_key);
        }
        Ok(root)
    } else {
        // each sheet is treated like a file: they all deep-merge into one
        // value, in `order` applied to the sheet names, later overriding
        // earlier
        sort_sheet_names(&mut names, options.order);
        let mut merged = Value::Object(BTreeMap::new());
        let mut any = false;
        for name in names {
            let value = parse_sheet(&mut workbook, &name, layout, path, options)?;
            if matches!(value, Value::Null) {
                continue; // an empty sheet contributes nothing
            }
            any = true;
            deep_merge(&mut merged, value, !options.case_sensitive);
        }
        Ok(if any { merged } else { Value::Null })
    }
}

/// One sheet's value under its sheet name, inserted the way every data
/// key is — so `dot_key` expands a dotted sheet name (`a.b` →
/// `{a: {b: …}}`) and the array suffixes work.
fn sheet_key(name: &str, value: Value, options: &Options) -> Value {
    let mut root = Value::Object(BTreeMap::new());
    super::insert_key(&mut root, name, value, options.dot_key);
    root
}

/// Deep-merge `incoming` into `target`, mirroring the loader's merge
/// (objects recurse, everything else replaces). When `lowercase`, object
/// keys fold to lowercase — the same normalization the traced merge
/// applies for `case_sensitive: false`. Merges a workbook's sheets (each
/// treated like a file) into one value.
fn deep_merge(target: &mut Value, incoming: Value, lowercase: bool) {
    match (target, incoming) {
        (Value::Object(entries), Value::Object(inc)) => {
            for (key, value) in inc {
                let key = if lowercase { key.to_lowercase() } else { key };
                match entries.get_mut(&key) {
                    Some(slot) => deep_merge(slot, value, lowercase),
                    None => {
                        entries.insert(key, fold_keys(value, lowercase));
                    }
                }
            }
        }
        (slot, value) => *slot = fold_keys(value, lowercase),
    }
}

/// Recursively lowercase an incoming subtree's object keys (no-op unless
/// `lowercase`), matching how the traced merge records fresh leaves.
fn fold_keys(value: Value, lowercase: bool) -> Value {
    match value {
        Value::Object(map) if lowercase => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k.to_lowercase(), fold_keys(v, lowercase)))
                .collect(),
        ),
        other => other,
    }
}

/// Order sheet names for merging, mirroring the folder [`Order`] rules
/// (sheets have no folder/file distinction, so `FoldersFirstAlphabetic`
/// is just alphabetic).
fn sort_sheet_names(names: &mut [String], order: crate::Order) {
    match order {
        crate::Order::ReverseAlphabetic => names.sort_by(|a, b| b.cmp(a)),
        _ => names.sort(),
    }
}

fn keep(sheet: &Sheet, options: &Options) -> bool {
    if sheet.typ != SheetType::WorkSheet {
        return false; // chart/dialog/macro/VBA sheets carry no cells
    }
    if options.ignore_hidden_sheets && sheet.visible != SheetVisible::Visible {
        return false;
    }
    if options.ignore_commented_sheetnames && crate::options::is_commented(&sheet.name) {
        return false;
    }
    true
}

/// Lower one sheet's used range into positional rows anchored at A1 and
/// run the table stage over them.
fn parse_sheet(
    workbook: &mut Sheets<BufReader<File>>,
    name: &str,
    layout: &TableLayout,
    path: &Path,
    options: &Options,
) -> Result<Value> {
    let range = workbook.worksheet_range(name).map_err(open_err(path))?;
    let mut rows: Vec<Vec<String>> = Vec::new();
    if let Some((start_row, start_col)) = range.start() {
        // pad the leading empty rows/columns the used range skips, so
        // column A stays the key column and error rows match the sheet
        rows.resize(start_row as usize, Vec::new());
        for cells in range.rows() {
            let mut row = vec![String::new(); start_col as usize];
            for cell in cells {
                row.push(cell_text(cell).map_err(|message| Error::Parse {
                    path: path.to_path_buf(),
                    message: format!("sheet '{name}': {message}"),
                })?);
            }
            rows.push(row);
        }
    }
    let mut value = super::table::parse(rows, layout, path, options)?;
    // a sheet is a data boundary, like a file: its keys are filtered
    // before any sheet-name key wraps the value
    super::strip_commented_data_keys(&mut value, options);
    Ok(value)
}

/// One cell as the text the table stage will type: strings as-is,
/// numbers/booleans via `Display`, ODS ISO date/duration text as-is,
/// Excel serial datetimes via [`serial_text`]. Error cells (`#DIV/0!`,
/// …) fail the file.
fn cell_text(cell: &Data) -> std::result::Result<String, String> {
    Ok(match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => f.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => serial_text(dt),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Error(e) => return Err(format!("cell error {e}")),
    })
}

/// An Excel serial datetime as text (no chrono): a duration-formatted
/// cell is `hh:mm:ss[.mmm]` of its total; a fraction-only serial
/// (a time-formatted cell) is `hh:mm:ss[.mmm]`; a whole-day serial at
/// midnight is `YYYY-MM-DD`; anything else is
/// `YYYY-MM-DD hh:mm:ss[.mmm]` (`.mmm` only when non-zero).
fn serial_text(dt: &ExcelDateTime) -> String {
    if dt.is_duration() {
        // total duration, hours unbounded
        let ms = (dt.as_f64() * 86_400_000.0).round() as u64;
        return time_text(ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    }
    let (year, month, day, hour, min, sec, milli) = dt.to_ymd_hms_milli();
    if dt.as_f64() < 1.0 {
        // fraction-only serial: a pure time-of-day cell
        return time_text(hour.into(), min.into(), sec.into(), milli.into());
    }
    if (hour, min, sec, milli) == (0, 0, 0, 0) {
        return format!("{year:04}-{month:02}-{day:02}");
    }
    format!(
        "{year:04}-{month:02}-{day:02} {}",
        time_text(hour.into(), min.into(), sec.into(), milli.into())
    )
}

fn time_text(hour: u64, min: u64, sec: u64, milli: u64) -> String {
    if milli == 0 {
        format!("{hour:02}:{min:02}:{sec:02}")
    } else {
        format!("{hour:02}:{min:02}:{sec:02}.{milli:03}")
    }
}

#[cfg(test)]
mod tests {
    use calamine::ExcelDateTimeType;

    use super::*;

    fn serial(value: f64, ty: ExcelDateTimeType) -> String {
        serial_text(&ExcelDateTime::new(value, ty, false))
    }

    #[test]
    fn serial_datetime_forms() {
        // 45418 = 2024-05-06 (1900 epoch)
        assert_eq!(serial(45418.0, ExcelDateTimeType::DateTime), "2024-05-06");
        assert_eq!(
            serial(45418.297326388885, ExcelDateTimeType::DateTime),
            "2024-05-06 07:08:09"
        );
        // fraction-only serials are time-of-day
        assert_eq!(serial(0.5, ExcelDateTimeType::DateTime), "12:00:00");
        assert_eq!(serial(0.2503, ExcelDateTimeType::DateTime), "06:00:25.920");
        // durations format their total; hours may exceed 24
        assert_eq!(serial(1.25, ExcelDateTimeType::TimeDelta), "30:00:00");
        assert_eq!(serial(0.5, ExcelDateTimeType::TimeDelta), "12:00:00");
    }

    #[test]
    fn cell_text_forms() {
        assert_eq!(cell_text(&Data::Empty).unwrap(), "");
        assert_eq!(cell_text(&Data::String("a,b".into())).unwrap(), "a,b");
        assert_eq!(cell_text(&Data::Int(-3)).unwrap(), "-3");
        assert_eq!(cell_text(&Data::Float(8080.0)).unwrap(), "8080");
        assert_eq!(cell_text(&Data::Float(1.5)).unwrap(), "1.5");
        assert_eq!(cell_text(&Data::Bool(true)).unwrap(), "true");
        assert_eq!(
            cell_text(&Data::DateTimeIso("2024-05-06T07:08:09".into())).unwrap(),
            "2024-05-06T07:08:09"
        );
        assert!(cell_text(&Data::Error(calamine::CellErrorType::Div0)).is_err());
    }
}
