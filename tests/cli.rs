//! CLI behavior, spec: `USAGE` in src/main.rs and CLAUDE.md. Compiled
//! only with the cli feature.
//!
//! Stdout comparisons are fixture-driven: every folder under
//! `tests/cli/<case>/` holds `args.txt` (the command line,
//! whitespace-separated, paths relative to the package root) and exactly
//! one `result.*` file — the expected stdout, byte for byte. Behaviors a
//! stdout file cannot express (cwd default, `-o` file writing, `-f` vs
//! `-o` precedence, error exits) stay as regular tests below.
#![cfg(feature = "cli")]

mod common;

use std::path::Path;
use std::process::{Command, Output};

use common::{expect, fx};
use serde_json::Value as Json;

fn c4(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_c4"))
        .args(args)
        .output()
        .expect("failed to run c4 binary")
}

fn c4_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_c4"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("failed to run c4 binary")
}

fn stdout_json(out: &Output) -> Json {
    assert!(
        out.status.success(),
        "c4 failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("stdout is not valid JSON")
}

fn stdout_str(out: &Output) -> String {
    assert!(
        out.status.success(),
        "c4 failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout.clone()).unwrap()
}

#[test]
fn stdout_cases_match_result_files() {
    let root = Path::new("tests/cli");
    let mut cases: Vec<_> = std::fs::read_dir(root)
        .expect("tests/cli must exist")
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    cases.sort();
    assert!(!cases.is_empty(), "no CLI cases found");

    for case in cases {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        let args_text = std::fs::read_to_string(case.join("args.txt"))
            .unwrap_or_else(|e| panic!("{name}: args.txt: {e}"));
        let args: Vec<&str> = args_text.split_whitespace().collect();

        let result = std::fs::read_dir(&case)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("result."))
            })
            .unwrap_or_else(|| panic!("{name}: no result.* file"));
        let expected = std::fs::read(&result).unwrap();

        let out = c4(&args);
        assert!(
            out.status.success(),
            "case {name} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&expected),
            "stdout mismatch for case {name}"
        );
    }
}

#[test]
fn no_args_reads_config_folder_in_cwd() {
    // `c4` with no arguments loads ./config; without -f/-o the auto
    // format falls back to the Rust Debug form
    let out = c4_in(Path::new("tests/fixtures/simple"), &[]);
    assert_eq!(
        stdout_str(&out),
        std::fs::read_to_string("tests/cli/debug/result.txt").unwrap()
    );
}

#[test]
fn no_args_json_via_format_flag() {
    let out = c4_in(Path::new("tests/fixtures/simple"), &["-f", "json"]);
    assert_eq!(stdout_json(&out), expect("simple"));
}

#[test]
fn output_file_infers_format_from_extension() {
    let dir = std::env::temp_dir().join(format!("c4-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("merged.yml");

    let out = c4(&["-o", path.to_str().unwrap(), &fx("simple/config")]);
    // with -o nothing goes to stdout
    assert_eq!(stdout_str(&out), "");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "db:\n  host: localhost\n  port: 5432\nname: c4\nport: 8080\n"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn output_file_with_debug_extension() {
    // xxx.debug selects the Rust Debug representation
    let dir = std::env::temp_dir().join(format!("c4-cli-test-dbg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("merged.debug");

    let out = c4(&["-o", path.to_str().unwrap(), &fx("simple/config")]);
    assert_eq!(stdout_str(&out), "");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        std::fs::read_to_string("tests/cli/debug/result.txt").unwrap()
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn format_flag_wins_over_output_extension() {
    let dir = std::env::temp_dir().join(format!("c4-cli-test-f-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("merged.yml");

    let out = c4(&[
        "-f",
        "json",
        "-o",
        path.to_str().unwrap(),
        &fx("jsonc/config"),
    ]);
    assert_eq!(stdout_str(&out), "");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        std::fs::read_to_string("tests/cli/json_pretty/result.json").unwrap()
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn missing_source_errors_with_nonzero_exit() {
    let out = c4(&[&fx("does_not_exist")]);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not found"),
        "stderr should mention the missing source: {stderr}"
    );
}

#[test]
fn unknown_output_extension_falls_back_to_debug() {
    // auto cannot name a format for .xyz → debug, not an error
    let dir = std::env::temp_dir().join(format!("c4-cli-test-xyz-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("merged.xyz");

    let out = c4(&["-o", path.to_str().unwrap(), &fx("simple/config")]);
    assert_eq!(stdout_str(&out), "");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        std::fs::read_to_string("tests/cli/debug/result.txt").unwrap()
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn unknown_output_format_errors() {
    let out = c4(&["-f", "nope", &fx("simple/config")]);
    assert!(!out.status.success());
}

#[test]
fn warnings_go_to_stderr_in_every_mode() {
    for args in [
        vec!["-f", "json", "tests/fixtures/cnf_basic/config/openssl.cnf"],
        vec![
            "--trace",
            "-f",
            "json",
            "tests/fixtures/cnf_basic/config/openssl.cnf",
        ],
    ] {
        let out = c4(&args);
        assert!(out.status.success(), "{args:?}");
        assert_eq!(
            String::from_utf8_lossy(&out.stderr),
            "c4: warning: tests/fixtures/cnf_basic/config/openssl.cnf: line 4: ignored directive \
             '.include /etc/ssl/extra.cnf' (c4 does not process directives; add included files \
             as sources)\n\
             c4: warning: tests/fixtures/cnf_basic/config/openssl.cnf: line 5: ignored directive \
             '.pragma dollarid:true' (c4 does not process directives; add included files as \
             sources)\n",
            "{args:?}"
        );
    }
}

/// Write `json` to a temp file, convert it with `-f <format>`, and return
/// the output file (named for its format so c4 can read it back) or the
/// failed run.
fn convert(name: &str, json: &Json, format: &str) -> Result<std::path::PathBuf, String> {
    let dir = std::env::temp_dir().join(format!("c4-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("in.json");
    std::fs::write(&input, json.to_string()).unwrap();
    let out = c4(&["-f", format, input.to_str().unwrap()]);
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    let path = dir.join(if format == "env" { ".env" } else { "out.ini" });
    std::fs::write(&path, &out.stdout).unwrap();
    Ok(path)
}

#[test]
fn env_output_reads_back() {
    let json = serde_json::json!({
        "multi": "line1\nline2\ttab",
        "q": "say \"hi\" # not a comment",
        "back": "C:\\temp\\",
        "lead": "  padded ",
        "hash": "#fff",
        "empty": "",
        "single": "it's",
        "db": { "host": "localhost" }
    });
    let path = convert("env-rt", &json, "env").unwrap();
    let back = stdout_json(&c4(&["-f", "json", "--no-dot-key", path.to_str().unwrap()]));
    let expected = serde_json::json!({
        "multi": "line1\nline2\ttab",
        "q": "say \"hi\" # not a comment",
        "back": "C:\\temp\\",
        "lead": "  padded ",
        "hash": "#fff",
        "empty": "",
        "single": "it's",
        "db.host": "localhost"
    });
    assert_eq!(back, expected);
}

#[test]
fn env_output_rejects_unwritable_keys() {
    let err = convert("env-key", &serde_json::json!({ "a=b": "x" }), "env").unwrap_err();
    assert!(err.contains("key 'a=b' cannot be written as env"), "{err}");
}

#[test]
fn ini_output_reads_back() {
    let json = serde_json::json!({
        "name": "c4",
        "list": [1, 2],
        "server": { "host": "localhost", "note": "a;b #c" },
        "db": { "port": 5432 }
    });
    let path = convert("ini-rt", &json, "ini").unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "list=[1,2]\nname=c4\n\n[db]\nport=5432\n\n[server]\nhost=localhost\nnote=a;b #c\n"
    );
    let back = stdout_json(&c4(&["-f", "json", path.to_str().unwrap()]));
    assert_eq!(
        back,
        serde_json::json!({
            "name": "c4",
            "list": "[1,2]",
            "server": { "host": "localhost", "note": "a;b #c" },
            "db": { "port": "5432" }
        })
    );
}

#[test]
fn ini_output_rejects_what_ini_cannot_hold() {
    for (json, needle) in [
        (
            serde_json::json!({ "a": { "b": { "c": 1 } } }),
            "'a.b' nests below a section",
        ),
        (
            serde_json::json!({ "multi": "x\ny" }),
            "value of 'multi' has a newline",
        ),
        (
            serde_json::json!({ "pad": " x" }),
            "value of 'pad' has leading or trailing whitespace",
        ),
        (
            serde_json::json!({ "a=b": 1 }),
            "key 'a=b' cannot be written as ini",
        ),
        (
            serde_json::json!({ "s]": { "k": 1 } }),
            "section 's]' cannot be written as ini",
        ),
    ] {
        let err = convert("ini-err", &json, "ini").unwrap_err();
        assert!(err.contains(needle), "{json}: {err}");
    }
}
