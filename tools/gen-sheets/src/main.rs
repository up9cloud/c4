//! Regenerate every binary spreadsheet fixture (.xlsx / .ods) and the
//! `xlsx-sheets` example workbook.
//!
//! Pure std, zero dependencies: the zip writer emits stored
//! (uncompressed) entries with a fixed timestamp, so the output is
//! deterministic — reruns do not dirty the tree, and CI can regenerate
//! and `git diff --exit-code` to prove the fixtures match this source.
//!
//! Run from anywhere (paths are relative to this crate):
//!
//! ```sh
//! cargo run --manifest-path tools/gen-sheets/Cargo.toml
//! ```
//!
//! Cell model: see [`Cell`]. A sheet is `(name, hidden, rows)` where
//! rows are `(1-based row number, cells)` — skipped row numbers stay
//! empty, which is how the padded-row fixtures are made.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// One spreadsheet cell.
#[derive(Clone)]
enum Cell {
    /// A string cell.
    S(&'static str),
    /// A plain numeric cell.
    N(f64),
    /// A boolean cell.
    B(bool),
    /// xlsx only: a serial number styled as a datetime (numFmt 22).
    Dt(f64),
    /// xlsx only: a serial number styled as a time (numFmt 21).
    Tm(f64),
    /// xlsx only: a serial number styled as a date (numFmt 14).
    D(f64),
}

use Cell::{B, D, Dt, N, S, Tm};

type Row = (u32, Vec<Cell>);
type SheetDef = (&'static str, bool, Vec<Row>);

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures = root.join("tests/fixtures");

    // excel_basic: in merge mode every non-ignored sheet parses and they
    // deep-merge like files; here `config` is the only non-ignored sheet
    // (a `_`-prefixed sheet, a `#`-prefixed sheet and a hidden sheet are
    // all skipped), and a second workbook whose only sheet is ignored
    // contributes nothing. The sheet is a db grid (the spreadsheet default
    // layout): typed columns, an auto column (empty type cell), a dotted
    // key column, and a sparse second record.
    let config_rows = vec![
        (
            1,
            vec![S("name"), S("port"), S("pi"), S("debug"), S("db.host")],
        ),
        (2, vec![S("str"), S("u16"), S("f64"), S(""), S("str")]),
        (3, vec![S("c4"), N(8080.0), N(1.5), B(true), S("localhost")]),
        (4, vec![S("c5"), N(9090.0), S(""), B(false)]),
    ];
    let junk = |n: f64| vec![(1, vec![S("junk"), N(n)])];
    write_xlsx(
        &fixtures.join("excel_basic/config/app.xlsx"),
        &[
            ("config", false, config_rows),
            ("_notes", false, junk(1.0)),
            ("#draft", false, junk(2.0)),
            ("secret", true, junk(3.0)),
        ],
    );
    write_xlsx(
        &fixtures.join("excel_basic/config/zz_extra.xlsx"),
        &[("_misc", false, junk(4.0))],
    );

    // excel_hidden_config(+_off): the same workbook under both settings of
    // ignore_hidden_sheets — variant cases never share one config folder.
    let hidden_config = || {
        vec![(
            "config",
            true,
            vec![(1, vec![S("x")]), (2, vec![S("i64")]), (3, vec![N(1.0)])],
        )]
    };
    write_xlsx(
        &fixtures.join("excel_hidden_config/config/app.xlsx"),
        &hidden_config(),
    );
    write_xlsx(
        &fixtures.join("excel_hidden_config_off/config/app.xlsx"),
        &hidden_config(),
    );

    // excel_tree(+_prefix_off): every sheet becomes a key; prefixed sheets
    // (#/./_) follow ignore_commented_sheets, hidden sheets stay ignored.
    let grid = |key: &'static str, ty: &'static str, value: Cell| {
        vec![(1, vec![S(key)]), (2, vec![S(ty)]), (3, vec![value])]
    };
    let tree_sheets = || {
        vec![
            ("c", false, grid("k1", "str", S("v1"))),
            ("d", false, grid("k2", "i32", N(2.0))),
            ("#x", false, grid("p", "i64", N(1.0))),
            (".y", false, grid("p", "i64", N(2.0))),
            ("_z", false, grid("p", "i64", N(3.0))),
            ("h", true, grid("p", "i64", N(4.0))),
        ]
    };
    write_xlsx(&fixtures.join("excel_tree/config/a/b.xlsx"), &tree_sheets());
    write_xlsx(
        &fixtures.join("excel_tree_prefix_off/config/a/b.xlsx"),
        &tree_sheets(),
    );

    // excel_datetime: serial cells styled as datetime/date/time lower to
    // ISO-ish text, then the db type row types them.
    write_xlsx(
        &fixtures.join("excel_datetime/config/app.xlsx"),
        &[(
            "config",
            false,
            vec![
                (1, vec![S("created"), S("day"), S("start")]),
                (2, vec![S("dt"), S("date"), S("time")]),
                (
                    3,
                    vec![
                        Dt(excel_serial(2024, 5, 6, 7, 8, 9)),
                        D(excel_serial(2024, 5, 6, 0, 0, 0)),
                        Tm(0.5),
                    ],
                ),
            ],
        )],
    );

    // excel_bad: the grid starts at spreadsheet row 3, so the bad typed
    // cell in its data row must report row 5 (padded rows keep real row
    // numbers).
    write_xlsx(
        &fixtures.join("excel_bad/config/app.xlsx"),
        &[(
            "config",
            false,
            vec![(3, vec![S("id")]), (4, vec![S("i32")]), (5, vec![S("abc")])],
        )],
    );

    // excel_blank_type_row: the type row is positional — row 2 is
    // physically absent (writers don't materialize an all-empty row),
    // keys at row 1, records from row 3; the padded-in blank row 2 must
    // read as a type row of all `auto`s, not skip as a blank row (which
    // would consume the first record as type ids).
    write_xlsx(
        &fixtures.join("excel_blank_type_row/config/app.xlsx"),
        &[(
            "config",
            false,
            vec![
                (1, vec![S("lv"), S("exp")]),
                (3, vec![N(0.0), N(0.0)]),
                (4, vec![N(1.0), N(5.0)]),
            ],
        )],
    );

    // excel_sheets: one workbook, several sheets, each read by its own
    // table source (sheet name + layout); `_extra` proves an explicitly
    // named sheet bypasses the prefix filter.
    let game_sheets = || {
        vec![
            (
                "config",
                false,
                vec![
                    (1, vec![S("title"), S("Hello"), S("str")]),
                    (2, vec![S("max_players"), N(8.0), S("u8")]),
                ],
            ),
            (
                "items",
                false,
                vec![
                    (1, vec![S("id"), S("name"), S("price")]),
                    (2, vec![S("u32"), S("str"), S("f64")]),
                    (3, vec![N(1.0), S("sword"), N(10.5)]),
                    (4, vec![N(2.0), S("shield"), N(7.0)]),
                ],
            ),
            (
                "npcs",
                false,
                vec![
                    (1, vec![S("name"), S("hp")]),
                    (2, vec![S("slime"), N(10.0)]),
                    (3, vec![S("bat"), N(12.0)]),
                ],
            ),
            (
                "meta",
                false,
                vec![
                    (1, vec![S("author"), S("version")]),
                    (2, vec![S("up9cloud"), N(2.0)]),
                ],
            ),
            ("_extra", false, vec![(1, vec![S("note"), S("hi")])]),
        ]
    };
    write_xlsx(
        &fixtures.join("excel_sheets/config/game.xlsx"),
        &game_sheets(),
    );
    // the xlsx-sheets example reads the same workbook shape
    write_xlsx(
        &root.join("examples/xlsx-sheets/config.xlsx"),
        &game_sheets(),
    );

    // excel_kv_formats: a kv-shaped config sheet — spreadsheets default
    // to db, so reading it takes a formats-level override
    // (`(excel, ["xlsx"], "kv")`).
    write_xlsx(
        &fixtures.join("excel_kv_formats/config/app.xlsx"),
        &[(
            "config",
            false,
            vec![
                (1, vec![S("title"), S("Hello"), S("str")]),
                (2, vec![S("max_players"), N(8.0), S("u8")]),
            ],
        )],
    );

    // ods_basic / ods_tree: the same selection rules through the ods reader.
    write_ods(
        &fixtures.join("ods_basic/config/app.ods"),
        &[
            (
                "config",
                false,
                vec![
                    (1, vec![S("name"), S("port")]),
                    (2, vec![S("str"), S("u16")]),
                    (3, vec![S("c4"), N(8080.0)]),
                ],
            ),
            ("_notes", false, vec![(1, vec![S("junk"), N(1.0)])]),
            ("hidden1", true, vec![(1, vec![S("junk"), N(2.0)])]),
        ],
    );
    write_ods(
        &fixtures.join("ods_tree/config/a/b.ods"),
        &[
            ("c", false, grid("k1", "str", S("v1"))),
            ("d", false, grid("k2", "i32", N(2.0))),
            ("_z", false, grid("p", "i64", N(1.0))),
            ("h", true, grid("p", "i64", N(2.0))),
        ],
    );

    println!("regenerated all .xlsx/.ods fixtures");
}

/// Excel 1900-epoch serial (days since 1899-12-30, time as day fraction).
fn excel_serial(year: i64, month: i64, day: i64, hour: i64, min: i64, sec: i64) -> f64 {
    // Howard Hinnant's days-from-civil algorithm
    fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe
    }
    let days = days_from_civil(year, month, day) - days_from_civil(1899, 12, 30);
    days as f64 + (hour * 3600 + min * 60 + sec) as f64 / 86400.0
}

// ------------------------------------------------------------------ xml

/// Escape text content (`&`, `<`, `>`).
fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape an attribute value (text escapes plus `"`).
fn esc_attr(text: &str) -> String {
    esc(text).replace('"', "&quot;")
}

/// Display a numeric cell the way spreadsheets store it (shortest
/// round-trip; integers without a decimal point).
fn num(value: f64) -> String {
    format!("{value}")
}

// ----------------------------------------------------------------- xlsx

const XLSX_STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<cellXfs count="4">
<xf numFmtId="0" applyNumberFormat="0"/>
<xf numFmtId="22" applyNumberFormat="1"/>
<xf numFmtId="21" applyNumberFormat="1"/>
<xf numFmtId="14" applyNumberFormat="1"/>
</cellXfs>
</styleSheet>
"#;

/// 0-based column index → A1 letters.
fn col_ref(mut index: usize) -> String {
    let mut letters = String::new();
    index += 1;
    while index > 0 {
        let rem = (index - 1) % 26;
        letters.insert(0, (b'A' + rem as u8) as char);
        index = (index - 1) / 26;
    }
    letters
}

fn xlsx_cell(row: u32, col: usize, cell: &Cell) -> String {
    let r = format!("{}{row}", col_ref(col));
    match cell {
        S(text) => format!(
            r#"<c r="{r}" t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,
            esc(text)
        ),
        N(value) => format!(r#"<c r="{r}"><v>{}</v></c>"#, num(*value)),
        B(value) => format!(r#"<c r="{r}" t="b"><v>{}</v></c>"#, u8::from(*value)),
        Dt(value) => format!(r#"<c r="{r}" s="1"><v>{}</v></c>"#, num(*value)),
        Tm(value) => format!(r#"<c r="{r}" s="2"><v>{}</v></c>"#, num(*value)),
        D(value) => format!(r#"<c r="{r}" s="3"><v>{}</v></c>"#, num(*value)),
    }
}

fn xlsx_sheet_xml(rows: &[Row]) -> String {
    let mut rows = rows.to_vec();
    rows.sort_by_key(|(number, _)| *number);
    let mut body = String::new();
    for (number, cells) in &rows {
        let cells: String = cells
            .iter()
            .enumerate()
            .map(|(col, cell)| xlsx_cell(*number, col, cell))
            .collect();
        write!(body, r#"<row r="{number}">{cells}</row>"#).unwrap();
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{body}</sheetData></worksheet>"#
    )
}

fn write_xlsx(path: &PathBuf, sheets: &[SheetDef]) {
    let mut overrides = String::from(
        r#"<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>"#,
    );
    let mut sheet_tags = String::new();
    let mut rel_tags = String::from(
        r#"<Relationship Id="rIdStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#,
    );
    let mut entries: Vec<(String, String)> = Vec::new();
    for (i, (name, hidden, rows)) in sheets.iter().enumerate() {
        let id = i + 1;
        let state = if *hidden { r#" state="hidden""# } else { "" };
        write!(
            sheet_tags,
            r#"<sheet name="{}" sheetId="{id}"{state} r:id="rId{id}"/>"#,
            esc_attr(name)
        )
        .unwrap();
        write!(
            rel_tags,
            r#"<Relationship Id="rId{id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{id}.xml"/>"#
        )
        .unwrap();
        write!(
            overrides,
            r#"<Override PartName="/xl/worksheets/sheet{id}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#
        )
        .unwrap();
        entries.push((format!("xl/worksheets/sheet{id}.xml"), xlsx_sheet_xml(rows)));
    }

    let content_types = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>{overrides}</Types>"#
    );
    let root_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;
    let workbook = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>{sheet_tags}</sheets></workbook>"#
    );
    let workbook_rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rel_tags}</Relationships>"#
    );

    let mut zip = Zip::new();
    zip.add("[Content_Types].xml", content_types.as_bytes());
    zip.add("_rels/.rels", root_rels.as_bytes());
    zip.add("xl/workbook.xml", workbook.as_bytes());
    zip.add("xl/_rels/workbook.xml.rels", workbook_rels.as_bytes());
    zip.add("xl/styles.xml", XLSX_STYLES.as_bytes());
    for (name, xml) in &entries {
        zip.add(name, xml.as_bytes());
    }
    write_file(path, &zip.finish());
}

// ------------------------------------------------------------------ ods

const ODS_MIMETYPE: &str = "application/vnd.oasis.opendocument.spreadsheet";

fn ods_cell(cell: &Cell) -> String {
    match cell {
        S(text) => format!(
            r#"<table:table-cell office:value-type="string"><text:p>{}</text:p></table:table-cell>"#,
            esc(text)
        ),
        N(value) => format!(
            r#"<table:table-cell office:value-type="float" office:value="{0}"><text:p>{0}</text:p></table:table-cell>"#,
            num(*value)
        ),
        B(value) => format!(
            r#"<table:table-cell office:value-type="boolean" office:boolean-value="{value}"><text:p>{}</text:p></table:table-cell>"#,
            if *value { "TRUE" } else { "FALSE" }
        ),
        _ => panic!("styled serial cells are xlsx-only"),
    }
}

fn write_ods(path: &PathBuf, sheets: &[SheetDef]) {
    let mut tables = String::new();
    for (name, hidden, rows) in sheets {
        let style = if *hidden { "taHidden" } else { "taVisible" };
        let mut rows = rows.to_vec();
        rows.sort_by_key(|(number, _)| *number);
        let last = rows.iter().map(|(number, _)| *number).max().unwrap_or(0);
        let mut body = String::new();
        for number in 1..=last {
            let cells: String = rows
                .iter()
                .find(|(n, _)| *n == number)
                .map(|(_, cells)| cells.iter().map(ods_cell).collect())
                .unwrap_or_default();
            write!(body, "<table:table-row>{cells}</table:table-row>").unwrap();
        }
        write!(
            tables,
            r#"<table:table table:name="{}" table:style-name="{style}">{body}</table:table>"#,
            esc_attr(name)
        )
        .unwrap();
    }
    let manifest = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.2"><manifest:file-entry manifest:full-path="/" manifest:media-type="{ODS_MIMETYPE}"/><manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/></manifest:manifest>"#
    );
    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" office:version="1.2"><office:automatic-styles><style:style style:name="taVisible" style:family="table"><style:table-properties table:display="true"/></style:style><style:style style:name="taHidden" style:family="table"><style:table-properties table:display="false"/></style:style></office:automatic-styles><office:body><office:spreadsheet>{tables}</office:spreadsheet></office:body></office:document-content>"#
    );

    // the mimetype must be the first entry (and is stored, like all our
    // entries — calamine only checks its bytes)
    let mut zip = Zip::new();
    zip.add("mimetype", ODS_MIMETYPE.as_bytes());
    zip.add("META-INF/manifest.xml", manifest.as_bytes());
    zip.add("content.xml", content.as_bytes());
    write_file(path, &zip.finish());
}

// ------------------------------------------------------------------ zip

/// A minimal zip writer: stored (uncompressed) entries only, fixed
/// timestamp (1980-01-01 00:00, the DOS epoch) — byte-deterministic.
struct Zip {
    data: Vec<u8>,
    /// (name, crc, size, local header offset)
    entries: Vec<(String, u32, u32, u32)>,
}

const DOS_DATE: u16 = 0x0021; // 1980-01-01
const DOS_TIME: u16 = 0;

impl Zip {
    fn new() -> Self {
        Self {
            data: Vec::new(),
            entries: Vec::new(),
        }
    }

    fn add(&mut self, name: &str, bytes: &[u8]) {
        let offset = self.data.len() as u32;
        let crc = crc32(bytes);
        let size = bytes.len() as u32;
        self.data.extend_from_slice(&0x04034b50u32.to_le_bytes()); // local header
        self.data.extend_from_slice(&20u16.to_le_bytes()); // version needed
        self.data.extend_from_slice(&0u16.to_le_bytes()); // flags
        self.data.extend_from_slice(&0u16.to_le_bytes()); // method: stored
        self.data.extend_from_slice(&DOS_TIME.to_le_bytes());
        self.data.extend_from_slice(&DOS_DATE.to_le_bytes());
        self.data.extend_from_slice(&crc.to_le_bytes());
        self.data.extend_from_slice(&size.to_le_bytes()); // compressed
        self.data.extend_from_slice(&size.to_le_bytes()); // uncompressed
        self.data
            .extend_from_slice(&(name.len() as u16).to_le_bytes());
        self.data.extend_from_slice(&0u16.to_le_bytes()); // extra len
        self.data.extend_from_slice(name.as_bytes());
        self.data.extend_from_slice(bytes);
        self.entries.push((name.to_owned(), crc, size, offset));
    }

    fn finish(mut self) -> Vec<u8> {
        let central_offset = self.data.len() as u32;
        for (name, crc, size, offset) in &self.entries {
            self.data.extend_from_slice(&0x02014b50u32.to_le_bytes()); // central header
            self.data.extend_from_slice(&20u16.to_le_bytes()); // made by
            self.data.extend_from_slice(&20u16.to_le_bytes()); // version needed
            self.data.extend_from_slice(&0u16.to_le_bytes()); // flags
            self.data.extend_from_slice(&0u16.to_le_bytes()); // method: stored
            self.data.extend_from_slice(&DOS_TIME.to_le_bytes());
            self.data.extend_from_slice(&DOS_DATE.to_le_bytes());
            self.data.extend_from_slice(&crc.to_le_bytes());
            self.data.extend_from_slice(&size.to_le_bytes());
            self.data.extend_from_slice(&size.to_le_bytes());
            self.data
                .extend_from_slice(&(name.len() as u16).to_le_bytes());
            self.data.extend_from_slice(&0u16.to_le_bytes()); // extra len
            self.data.extend_from_slice(&0u16.to_le_bytes()); // comment len
            self.data.extend_from_slice(&0u16.to_le_bytes()); // disk number
            self.data.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
            self.data.extend_from_slice(&0u32.to_le_bytes()); // external attrs
            self.data.extend_from_slice(&offset.to_le_bytes());
            self.data.extend_from_slice(name.as_bytes());
        }
        let central_size = self.data.len() as u32 - central_offset;
        let count = self.entries.len() as u16;
        self.data.extend_from_slice(&0x06054b50u32.to_le_bytes()); // end of central dir
        self.data.extend_from_slice(&0u16.to_le_bytes()); // disk number
        self.data.extend_from_slice(&0u16.to_le_bytes()); // central dir disk
        self.data.extend_from_slice(&count.to_le_bytes());
        self.data.extend_from_slice(&count.to_le_bytes());
        self.data.extend_from_slice(&central_size.to_le_bytes());
        self.data.extend_from_slice(&central_offset.to_le_bytes());
        self.data.extend_from_slice(&0u16.to_le_bytes()); // comment len
        self.data
    }
}

/// IEEE CRC-32, bit-by-bit (speed is irrelevant here).
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn write_file(path: &PathBuf, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().expect("fixture paths have parents")).unwrap();
    std::fs::write(path, bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}
