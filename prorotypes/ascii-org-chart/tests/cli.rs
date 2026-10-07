//! Integration tests for the CLI binary: file/stdin input, flag wiring,
//! output file, and the check subcommand.

use std::fs;
use std::io::Read as _;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

/// The spec's example org.
fn spec_org() -> String {
    concat!(
        "Ada Lovelace [CEO]\n",
        "  Grace Hopper [VP Engineering]\n",
        "    Alan Turing [Staff Eng]\n",
        "    Edsger Dijkstra [Staff Eng]\n",
        "  Katherine Johnson [VP Data]\n",
        "    Margaret Hamilton [Eng Manager]\n",
    )
    .to_string()
}

/// Write `contents` to a named file inside a fresh temp dir, returning
/// the dir (keep it alive until the test ends) and the file path.
fn write_temp(name: &str, contents: &str) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join(name);
    fs::write(&path, contents).unwrap();
    (dir, path)
}

fn orgchart() -> Command {
    Command::cargo_bin("orgchart").unwrap()
}

#[test]
fn renders_file_to_stdout() {
    let (_dir, path) = write_temp("two.org", "Ada [CEO]\n  Bob [Sales]\n");
    let output = orgchart().arg(&path).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains('┌'), "chart should use box-drawing chars: {stdout}");
    assert!(stdout.contains("Ada"));
}

#[test]
fn tree_flag_and_ascii_flag() {
    // Two children so the first row uses the branch (`|-`) connector.
    let (_dir, path) = write_temp("three.org", "Ada [CEO]\n  Bob [Sales]\n  Cy [Support]\n");
    let output = orgchart()
        .args(["--layout", "tree", "--ascii"])
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("|-"), "ascii tree should use `|-`: {stdout}");
}

#[test]
fn max_depth_collapses() {
    let (_dir, path) = write_temp("spec.org", &spec_org());
    let output = orgchart()
        .args(["--max-depth", "1"])
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("(3 people)"),
        "collapsed subtree should be summarized: {stdout}"
    );
}

#[test]
fn json_flag() {
    let (_dir, path) = write_temp("spec.org", &spec_org());
    let output = orgchart()
        .args(["--json"])
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout should be valid JSON ({e}): {stdout}"));
    let org: Vec<Value> = serde_json::from_value(json)
        .unwrap_or_else(|e| panic!("--json should emit an array ({e})"));
    assert_eq!(org[0]["name"], "Ada Lovelace");
}

#[test]
fn output_flag_writes_file() {
    let (dir, path) = write_temp("two.org", "Ada [CEO]\n  Bob [Sales]\n");
    let out = dir.path().join("out.txt");
    let output = orgchart()
        .args(["-o", out.to_str().unwrap()])
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stdout.is_empty(),
        "stdout should be empty when -o is given"
    );
    let written = fs::read_to_string(&out).unwrap();
    assert!(written.contains("Ada"), "output file should contain the chart");
}

#[test]
fn reads_stdin() {
    let output = orgchart()
        .write_stdin("Ada [CEO]")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Ada"), "should read org text from stdin");
}

#[test]
fn reads_stdin_via_dash_literal() {
    let output = orgchart()
        .arg("-")
        .write_stdin("Ada [CEO]\n  Bob [Sales]\n")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Bob"), "`-` should be treated as stdin");
}

#[test]
fn check_reports_parse_error() {
    let (_dir, path) = write_temp("bad.org", "\tAda [CEO]\n");
    let output = orgchart()
        .args(["check"])
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("error: line 1: tab character"),
        "check should report the parse error: {stderr}"
    );
}

#[test]
fn check_silent_on_success() {
    let (_dir, path) = write_temp("good.org", &spec_org());
    let output = orgchart()
        .args(["check"])
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stdout.is_empty(),
        "check prints nothing on success (stdout)"
    );
    assert!(
        output.stderr.is_empty(),
        "check prints nothing on success (stderr)"
    );
}

#[test]
fn render_reports_parse_error() {
    let (_dir, path) = write_temp("bad.org", "Ada [CEO]\n\tBob [Staff]\n");
    let output = orgchart().arg(&path).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("error: line"),
        "render mode should report parse errors: {stderr}"
    );
}

#[test]
fn missing_file_is_io_error() {
    let output = orgchart()
        .arg("/nonexistent/org/org.org")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.starts_with("error: cannot read /nonexistent/org/org.org: "),
        "missing file should be an IO error: {stderr}"
    );
}

#[test]
fn watch_without_file_arg_is_usage_error() {
    // Watch with no positional path, or with `-` (stdin): usage
    // error on stderr with exit code 2, no output.
    for args in [vec!["--watch"], vec!["--watch", "-"]] {
        let output = orgchart().args(&args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "args {args:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(
            stderr, "error: --watch requires a file argument\n",
            "args {args:?}"
        );
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn render_flags_conflict_with_check_subcommand() {
    // check is validate-only; --watch / -o would be silently ignored,
    // so they are rejected with a usage error (exit 2).
    for args in [vec!["--watch"], vec!["-o", "out.txt"]] {
        let (_dir, path) = write_temp("good.org", "Ada [CEO]\n");
        let output = orgchart()
            .args(&args)
            .arg("check")
            .arg(&path)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "args {args:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        let expected = if args[0] == "--watch" {
            "error: --watch cannot be combined with the check subcommand\n"
        } else {
            "error: -o/--output cannot be combined with the check subcommand\n"
        };
        assert_eq!(stderr, expected, "args {args:?}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn version_flag_prints_cargo_version() {
    let output = orgchart().arg("--version").output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains(concat!("orgchart ", env!("CARGO_PKG_VERSION"))),
        "--version should print the package version: {stdout}"
    );
}

/// A big org: enough rendered output to overrun any OS pipe buffer
/// (thousands of tree rows), so a reader that bows out early forces
/// a real EPIPE mid-write.
fn huge_org(rows: usize) -> String {
    let mut org = String::from("Root [CEO]\n");
    for i in 0..rows {
        org.push_str(&format!("  Person {i} [Staff]\n"));
    }
    org
}

#[test]
fn broken_pipe_exits_cleanly_when_reader_closes_early() {
    // A consumer closing the pipe mid-write (e.g. `| head -1`) must
    // not be an error: exit 0, nothing on stderr. Deterministic EPIPE
    // exercise: spawn with piped stdout, read one byte, drop the
    // reader — the next write of the huge render fails with
    // BrokenPipe (Rust ignores SIGPIPE, so it surfaces as an io
    // error, which the BrokenPipe arm turns into a clean exit).
    let (_dir, path) = write_temp("huge.org", &huge_org(10_000));
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_orgchart"))
        .args(["--layout", "tree"])
        .arg(&path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("orgchart binary should spawn");
    let mut stdout = child.stdout.take().unwrap();
    let mut first = [0u8; 1];
    stdout.read_exact(&mut first).expect("child should write a first byte");
    drop(stdout); // the consumer goes away after one byte
    let status = child.wait().expect("child should be waitable");
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(
        stderr.is_empty(),
        "a closed pipe must not be reported as an error: {stderr}"
    );
    assert_eq!(status.code(), Some(0));
}

#[test]
fn bom_json_first_name_is_clean() {
    // A file written by a BOM-emitting editor must serialize the
    // first person's name without the BOM under --json.
    let (_dir, path) = write_temp("bom.org", &format!("\u{feff}{}", spec_org()));
    let output = orgchart()
        .args(["--json"])
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let org: Vec<Value> = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout should be valid JSON ({e}): {stdout}"));
    assert_eq!(org[0]["name"], "Ada Lovelace");
    assert!(
        !stdout.contains('\u{feff}'),
        "BOM must not appear anywhere in JSON output"
    );
}
