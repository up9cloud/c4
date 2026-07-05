//! c4 CLI — spec: CLAUDE.md "CLI" (README.md has the short form). Built
//! only with `--features cli`.
//!
//! Loads config sources (default: ./config) and writes the merged result
//! as a single document: `[sources...] [-f fmt] [-o path] [--trace]`.

use std::path::PathBuf;
use std::process::ExitCode;

use c4::{Loader, Options, Source};

const USAGE: &str = "\
usage: c4 [sources...] [options]

  sources        folders and/or files (default: ./config)
  -f <format>    output format: json|jsonc|yaml|toml|ini|env|csv|debug
                 (default: auto — the -o extension, debug otherwise)
  -o <path>      write to a file instead of stdout (format from extension)
  --trace        annotate every value with the source it came from
  --tree         tree mode: folders/files become keys instead of merging
  -h, --help     show this help
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
    let mut tree = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-f" | "--format" => {
                format_flag = Some(args.next().ok_or("-f needs a format id")?);
            }
            "-o" | "--output" => {
                out_path = Some(PathBuf::from(args.next().ok_or("-o needs a path")?));
            }
            "--trace" => trace = true,
            "--tree" => tree = true,
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

    let sources: Vec<Source> = if sources.is_empty() {
        vec![Source::folder("config")]
    } else {
        sources
            .into_iter()
            // an existing file is a file source; anything else is treated
            // as a folder (missing paths get the loader's NotFound error)
            .map(|p| {
                if p.is_file() {
                    Source::file(p)
                } else {
                    Source::folder(p)
                }
            })
            .collect()
    };

    let loader = Loader::new(Options {
        sources,
        tree,
        ..Options::default()
    });
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
        serde_json::Value::Array(items) => {
            // homogeneous scalar arrays use arr:<t>; anything else is json
            let types: Vec<&str> = items
                .iter()
                .map(|v| match v {
                    serde_json::Value::Bool(_) => "bool",
                    serde_json::Value::Number(n) => csv_number_type(n),
                    serde_json::Value::String(_) => "str",
                    _ => "",
                })
                .collect();
            match types.first() {
                Some(first) if !first.is_empty() && types.iter().all(|t| t == first) => {
                    let cells: Vec<String> = items
                        .iter()
                        .map(|v| match v {
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .collect();
                    (cells.join(";"), format!("arr:{first}"))
                }
                _ => (
                    serde_json::to_string(value).map_err(|e| e.to_string())?,
                    "json".into(),
                ),
            }
        }
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
