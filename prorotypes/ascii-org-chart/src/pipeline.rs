//! The render/validate pipeline shared by every CLI mode: parse →
//! collapse (if `--max-depth`) → `--json` ? serialize : render.

use crate::cli::{Cli, Command, Layout};
use crate::dsl;
use crate::model;
use crate::render::{Charset, ChartRenderer, Renderer, TreeRenderer};

/// Run the pipeline for `cli` over `input` (the org DSL text). Returns
/// the formatted output (rendered chart or JSON) on success, or the
/// fully formatted error string (`error: ...`) on failure. `check`
/// ignores render flags and produces no output on success.
pub fn process(cli: &Cli, input: &str) -> Result<String, String> {
    if let Some(Command::Check { .. }) = cli.command {
        return dsl::parse(input).map(|_| String::new()).map_err(|e| e.to_string());
    }

    let mut roots = dsl::parse(input).map_err(|e| e.to_string())?;
    if let Some(max_depth) = cli.max_depth {
        roots = model::collapse_to_depth(roots, max_depth);
    }

    if cli.json {
        // Node serializes only strings and sequences, so serialization
        // cannot fail on legal input — but the error contract requires
        // every Err string to be fully formatted, just in case.
        serde_json::to_string(&roots)
            .map(|json| format!("{json}\n"))
            .map_err(|e| format!("error: {e}"))
    } else {
        let charset = if cli.ascii { Charset::Ascii } else { Charset::Unicode };
        let rendered = match cli.layout {
            Layout::Chart => ChartRenderer.render(&roots, charset),
            Layout::Tree => TreeRenderer.render(&roots, charset),
        };
        Ok(rendered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn json_cli() -> Cli {
        Cli::try_parse_from(["orgchart", "--json"]).unwrap()
    }

    /// A legal `levels`-deep org: level i+1 is indented i+1 units.
    fn deep_org(levels: usize) -> String {
        let mut org = String::from("Ada [CEO]");
        for i in 1..=levels {
            org.push_str(&format!("\n{}Person {i}", "  ".repeat(i)));
        }
        org
    }

    #[test]
    fn json_output_ends_with_newline() {
        let out = process(&json_cli(), "Ada [CEO]\n  Bob [Sales]\n").unwrap();
        assert!(out.ends_with('\n'), "JSON output should be POSIX-newline terminated");
        serde_json::from_str::<serde_json::Value>(&out)
            .expect("JSON output (with trailing newline) should still parse");
    }

    #[test]
    fn deep_org_serializes_under_json() {
        // serde_json's 128-depth recursion limit applies to
        // deserialization, not to serializing nested strings/sequences,
        // so a 130-level org must still export under --json. (Round-
        // tripping it through from_str would hit the limit on the
        // consumer side — out of scope here.)
        let out = process(&json_cli(), &deep_org(130)).unwrap();
        assert!(out.starts_with(r#"[{"name":"Ada""#));
        assert!(out.ends_with('\n'));
        assert!(out.contains("Person 130"));
    }

    #[test]
    fn check_ignores_render_flags() {
        let cli = Cli::try_parse_from(["orgchart", "--layout", "tree", "--ascii",
            "--max-depth", "1", "--json", "check", "org.org"]).unwrap();
        assert_eq!(process(&cli, "Ada [CEO]\n  Bob [Sales]\n"), Ok(String::new()));
    }
}
