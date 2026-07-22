//! c4 CLI — spec: CLAUDE.md "CLI" (README.md has the short form). Built
//! only with `--features cli`.
//!
//! Loads config sources (default: ./config) and writes the merged result
//! as a single document: `[sources...] [-f fmt] [-o path] [--trace]`.

use std::path::PathBuf;
use std::process::ExitCode;

use c4::{Loader, Options, Order, Source};

const USAGE: &str = "\
usage: c4 [sources...] [options]

Load config sources (default: ./config) and write the merged result as
one document. Sources are folders and/or files given positionally; later
sources override earlier ones.

Output:
  -f, --format <fmt>    json|jsonc|yaml|toml|ini|env|csv|debug
                        (default: the -o extension, else debug)
  -o, --output <path>   write to a file instead of stdout (format from ext)
  --trace               annotate every value with its source + format

Options (mirror Options; each boolean also has a --no-<name> form):
  --dir-depth <n>       subdirectory levels to scan: 1 (default) = the
                        folder + one level, 0 = the folder only, -1 = all
  --filename-as-key     make each file a key named after it (default off)
  --dirname-as-key      make each subfolder a key (default off)
  --sheetname-as-key    spreadsheets: make each sheet a key (default off;
                        off reads only the 'config' sheet)
  --tree                preset: --filename-as-key --dirname-as-key
                        --sheetname-as-key --dir-depth -1
  --dot-key             expand dotted keys a.b.c -> {a:{b:{c}}} (default on)
  --case-sensitive      case-sensitive key merge (default on)
  --order <id>          folders_first_alphabetic (default) | alphabetic |
                        reverse_alphabetic  (also: folders_first, reverse)
  --auto-no-ext-files  keyed files: auto-detect extension-less files (on)
  --ignore-unknown-ext  keyed files: skip unknown-extension files (on)
  --ignore-commented-sheets
                        spreadsheets: skip sheets named #*/.*/_* (default on)
  --ignore-hidden-sheets
                        spreadsheets: skip hidden sheets (default on)
  -h, --help            show this help

Examples:
  c4                                    # read ./config, print the debug form
  c4 ./config ./local.toml -f yaml      # merge sources, print yaml
  c4 -o merged.json                     # write json (format from extension)
  c4 --trace -f json                    # provenance tree as json
  c4 --tree ./config                    # tree preset: folders/files become keys
  c4 --dirname-as-key                   # nest each subfolder as a key
  c4 --dir-depth -1                     # merge every subdirectory level
  c4 --order alphabetic                 # folders and files interleaved
  c4 --no-dot-key --no-case-sensitive   # flat keys, case-insensitive merge

Notes:
  csv is positional key,value[,format]; for a header row or renamed/
  reordered columns, use a CustomFormat (see the csv-header example).
  Spreadsheets (xlsx/xlsm/xlsb/xls/ods) parse each sheet as a db record
  grid (keys row, types row, data rows); the sheet named 'config' is
  read (with --sheetname-as-key, every sheet becomes a key). They are
  input-only formats (-f excel is not valid output).
";

/// Output serializer. Jsonc collapses into Json; env and ini share the
/// flat `key=value` emitter; Debug prints the Rust `{:#?}` form of the
/// loaded `Value` / `TracedValue`.
#[derive(Clone, Copy, PartialEq)]
enum OutFormat {
    Json,
    Yaml,
    Toml,
    Ini,
    Env,
    Csv,
    Debug,
}

impl OutFormat {
    fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "json" | "jsonc" => OutFormat::Json,
            "yaml" => OutFormat::Yaml,
            "toml" => OutFormat::Toml,
            "ini" => OutFormat::Ini,
            "env" => OutFormat::Env,
            "csv" => OutFormat::Csv,
            "debug" => OutFormat::Debug,
            _ => return None,
        })
    }

    fn from_extension(ext: &str) -> Option<Self> {
        Some(match ext {
            "json" | "jsonc" => OutFormat::Json,
            "yml" | "yaml" => OutFormat::Yaml,
            "toml" => OutFormat::Toml,
            "ini" => OutFormat::Ini,
            "env" => OutFormat::Env,
            "csv" => OutFormat::Csv,
            "debug" => OutFormat::Debug,
            _ => return None,
        })
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("c4: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut sources: Vec<PathBuf> = Vec::new();
    let mut format_flag: Option<String> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut trace = false;
    // the CLI is always flat-by-default and opts into keying via flags
    // (`--tree`, `--filename-as-key`, …). `Options::default()` would flip
    // to tree-shaped loading when the binary is built with the `tree`
    // feature, so pin an explicit flat baseline regardless of build.
    let mut opts = Options {
        filename_as_key: false,
        dirname_as_key: false,
        sheetname_as_key: false,
        dir_depth: 1,
        ..Options::default()
    };

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        // support both `--flag value` and `--flag=value` for value flags
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) => (n, Some(v.to_owned())),
            None => (arg.as_str(), None),
        };
        // a value flag's argument comes from `--flag=v` or the next token
        let mut value = |err: &str| inline.clone().or_else(|| args.next()).ok_or(err.to_owned());
        match name {
            "-f" | "--format" => format_flag = Some(value("-f needs a format id")?),
            "-o" | "--output" => out_path = Some(PathBuf::from(value("-o needs a path")?)),
            "--trace" => trace = true,
            "--order" => {
                let id = value("--order needs an id")?;
                opts.order =
                    Order::from_id(&id).ok_or_else(|| format!("unknown order id '{id}'"))?;
            }
            "--dir-depth" => {
                let v = value("--dir-depth needs an integer")?;
                opts.dir_depth = v
                    .parse()
                    .map_err(|_| format!("--dir-depth needs an integer (got '{v}')"))?;
            }
            "--filename-as-key" => opts.filename_as_key = true,
            "--no-filename-as-key" => opts.filename_as_key = false,
            "--dirname-as-key" => opts.dirname_as_key = true,
            "--no-dirname-as-key" => opts.dirname_as_key = false,
            "--sheetname-as-key" => opts.sheetname_as_key = true,
            "--no-sheetname-as-key" => opts.sheetname_as_key = false,
            "--dot-key" => opts.dot_key = true,
            "--no-dot-key" => opts.dot_key = false,
            "--case-sensitive" => opts.case_sensitive = true,
            "--no-case-sensitive" => opts.case_sensitive = false,
            // the tree preset: folders, files and sheets all become keys,
            // and the whole tree is scanned
            "--tree" => {
                opts.filename_as_key = true;
                opts.dirname_as_key = true;
                opts.sheetname_as_key = true;
                opts.dir_depth = -1;
            }
            "--no-tree" => {
                opts.filename_as_key = false;
                opts.dirname_as_key = false;
                opts.sheetname_as_key = false;
                opts.dir_depth = 1;
            }
            "--auto-no-ext-files" => opts.auto_no_ext_files = true,
            "--no-auto-no-ext-files" => opts.auto_no_ext_files = false,
            "--ignore-unknown-ext" => opts.ignore_unknown_ext = true,
            "--no-ignore-unknown-ext" => opts.ignore_unknown_ext = false,
            "--ignore-commented-sheets" => opts.ignore_commented_sheets = true,
            "--no-ignore-commented-sheets" => opts.ignore_commented_sheets = false,
            "--ignore-hidden-sheets" => opts.ignore_hidden_sheets = true,
            "--no-ignore-hidden-sheets" => opts.ignore_hidden_sheets = false,
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(());
            }
            flag if flag.starts_with('-') => {
                return Err(format!("unknown flag '{flag}'\n{USAGE}"));
            }
            _ => sources.push(PathBuf::from(arg)),
        }
    }

    let format = resolve_format(format_flag.as_deref(), out_path.as_deref())?;

    // a path source auto-detects folder vs file at load time (a missing
    // path gets the loader's NotFound error); default source is ./config
    opts.sources = if sources.is_empty() {
        vec![Source::Path(PathBuf::from("config"))]
    } else {
        sources.into_iter().map(Source::Path).collect()
    };

    let loader = Loader::new(opts);
    // load the typed result first: debug prints it directly, every other
    // format goes through one serde_json tree
    let text = if trace {
        let traced = loader.trace().map_err(|e| e.to_string())?;
        match format {
            OutFormat::Debug => format!("{traced:#?}\n"),
            _ => emit(
                &serde_json::to_value(&traced).map_err(|e| e.to_string())?,
                format,
            )?,
        }
    } else {
        let value: c4::Value = loader.load().map_err(|e| e.to_string())?;
        match format {
            OutFormat::Debug => format!("{value:#?}\n"),
            _ => emit(
                &serde_json::to_value(&value).map_err(|e| e.to_string())?,
                format,
            )?,
        }
    };
    match out_path {
        Some(path) => {
            std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
        }
        None => {
            print!("{text}");
            Ok(())
        }
    }
}

/// The default is auto: `-f` wins, else the `-o` extension decides, and
/// when neither names a format (no `-o`, or an extension no output
/// format matches) the fallback is the Rust Debug form.
fn resolve_format(
    flag: Option<&str>,
    out_path: Option<&std::path::Path>,
) -> Result<OutFormat, String> {
    if let Some(id) = flag {
        return OutFormat::from_id(id).ok_or_else(|| format!("unknown output format '{id}'"));
    }
    if let Some(ext) = out_path
        .and_then(|p| p.extension())
        .and_then(|e| e.to_str())
    {
        if let Some(format) = OutFormat::from_extension(&ext.to_lowercase()) {
            return Ok(format);
        }
    }
    Ok(OutFormat::Debug)
}

fn emit(tree: &serde_json::Value, format: OutFormat) -> Result<String, String> {
    match format {
        OutFormat::Debug => unreachable!("debug is handled before serialization"),
        OutFormat::Json => serde_json::to_string_pretty(tree)
            .map(|s| s + "\n")
            .map_err(|e| e.to_string()),
        OutFormat::Yaml => serde_yaml::to_string(tree).map_err(|e| e.to_string()),
        OutFormat::Toml => toml::to_string_pretty(tree).map_err(|e| e.to_string()),
        OutFormat::Ini | OutFormat::Env => {
            let mut out = String::new();
            for (key, value) in flatten(tree) {
                out.push_str(&format!("{key}={}\n", env_value(value)?));
            }
            Ok(out)
        }
        OutFormat::Csv => {
            let mut out = String::new();
            for (key, value) in flatten(tree) {
                let (cell, ty) = csv_cell(value)?;
                out.push_str(&format!(
                    "{},{},{ty}\n",
                    csv_escape(&key),
                    csv_escape(&cell)
                ));
            }
            Ok(out)
        }
    }
}

/// Depth-first flatten of nested objects into dotted keys (the reverse of
/// `dot_key`); keys come out sorted because objects are sorted maps.
fn flatten(tree: &serde_json::Value) -> Vec<(String, &serde_json::Value)> {
    fn walk<'a>(
        prefix: &str,
        v: &'a serde_json::Value,
        out: &mut Vec<(String, &'a serde_json::Value)>,
    ) {
        match v {
            serde_json::Value::Object(map) => {
                for (key, value) in map {
                    let key = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    walk(&key, value, out);
                }
            }
            leaf => out.push((prefix.to_owned(), leaf)),
        }
    }
    let mut out = Vec::new();
    walk("", tree, &mut out);
    out
}

/// env/ini value text: scalars raw, arrays embedded as JSON strings.
fn env_value(value: &serde_json::Value) -> Result<String, String> {
    Ok(match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => quote_if_needed(s),
        serde_json::Value::Bool(_) | serde_json::Value::Number(_) => value.to_string(),
        serde_json::Value::Array(_) => {
            quote_if_needed(&serde_json::to_string(value).map_err(|e| e.to_string())?)
        }
        serde_json::Value::Object(_) => unreachable!("objects are flattened"),
    })
}

fn quote_if_needed(s: &str) -> String {
    if s.is_empty()
        || s.chars()
            .any(|c| c.is_whitespace() || matches!(c, '#' | '"' | '\''))
    {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_owned()
    }
}

/// csv value cell + explicit format column.
fn csv_cell(value: &serde_json::Value) -> Result<(String, String), String> {
    Ok(match value {
        serde_json::Value::Null => (String::new(), "null".into()),
        serde_json::Value::Bool(b) => (b.to_string(), "bool".into()),
        serde_json::Value::Number(n) => (n.to_string(), csv_number_type(n).into()),
        serde_json::Value::String(s) => (s.clone(), "str".into()),
        // arrays emit as a single json cell (the table stage reads them
        // back with the `json` type); objects are flattened away upstream
        serde_json::Value::Array(_) => (
            serde_json::to_string(value).map_err(|e| e.to_string())?,
            "json".into(),
        ),
        serde_json::Value::Object(_) => unreachable!("objects are flattened"),
    })
}

fn csv_number_type(n: &serde_json::Number) -> &'static str {
    if n.is_i64() {
        "i64"
    } else if n.is_u64() {
        "u64"
    } else {
        "f64"
    }
}

fn csv_escape(cell: &str) -> String {
    if cell.contains(',') || cell.contains('"') || cell.contains('\n') {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_owned()
    }
}
