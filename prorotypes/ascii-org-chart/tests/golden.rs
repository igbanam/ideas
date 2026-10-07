//! Golden-file tests: every fixture rendered by every renderer must match
//! its checked-in golden file byte for byte. Run with `UPDATE_GOLDEN=1
//! cargo test` to (re)generate the goldens instead of comparing.

use orgchart::dsl;
use orgchart::render::{Charset, ChartRenderer, Renderer, TreeRenderer};

/// One golden case: fixture name, DSL text, and renderers that each
/// produce a golden file (renderer name, renderer).
type Case = (
    &'static str,
    &'static str,
    Vec<(&'static str, Box<dyn Renderer>)>,
);

fn cases() -> Vec<Case> {
    vec![
        (
            "sample",
            include_str!("fixtures/sample.org"),
            vec![
                ("tree", Box::new(TreeRenderer)),
                ("chart", Box::new(ChartRenderer)),
            ],
        ),
        (
            "wide",
            include_str!("fixtures/wide.org"),
            vec![
                ("tree", Box::new(TreeRenderer)),
                ("chart", Box::new(ChartRenderer)),
            ],
        ),
        (
            "forest",
            include_str!("fixtures/forest.org"),
            vec![
                ("tree", Box::new(TreeRenderer)),
                ("chart", Box::new(ChartRenderer)),
            ],
        ),
        (
            "deep",
            include_str!("fixtures/deep.org"),
            vec![
                ("tree", Box::new(TreeRenderer)),
                ("chart", Box::new(ChartRenderer)),
            ],
        ),
        (
            "unicode",
            include_str!("fixtures/unicode.org"),
            vec![
                ("tree", Box::new(TreeRenderer)),
                ("chart", Box::new(ChartRenderer)),
            ],
        ),
    ]
}

fn golden_path(fixture: &str, renderer: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(format!("{fixture}.{renderer}.txt"))
}

#[test]
fn golden_files_match() {
    let update = std::env::var("UPDATE_GOLDEN").is_ok_and(|v| v == "1");
    let mut checked = 0;
    for (fixture, input, renderers) in cases() {
        let roots = dsl::parse(input).unwrap_or_else(|e| {
            panic!("fixture {fixture} should parse: {e}");
        });
        for (renderer_name, renderer) in renderers {
            let rendered = renderer.render(&roots, Charset::Unicode);
            let path = golden_path(fixture, renderer_name);
            if update {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)
                        .unwrap_or_else(|e| panic!("mkdir {}: {e}", dir.display()));
                }
                std::fs::write(&path, &rendered)
                    .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
            } else {
                let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
                    panic!(
                        "missing golden file {}: {e} (run UPDATE_GOLDEN=1 cargo test)",
                        path.display()
                    )
                });
                assert_eq!(
                    rendered, expected,
                    "{fixture}/{renderer_name} output differs from golden file",
                );
            }
            checked += 1;
        }
    }
    if !update {
        // Ten golden files: five fixtures × two renderers.
        assert_eq!(checked, 10, "unexpected golden test case count");
    }
}
