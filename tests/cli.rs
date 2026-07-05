//! CLI behavior, spec: CLAUDE.md "CLI". Compiled only with the cli
//! feature.
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
