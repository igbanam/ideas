# ASCII Org Chart Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Rust CLI (`orgchart`) that parses an indentation-based org DSL and renders ASCII org charts in two layouts (chart style default, tree style), with `--max-depth` collapsing, `--json` export, `--watch` mode, and a `check` subcommand.

**Architecture:** Single binary crate with a `lib.rs` exposing modules (so integration tests can use them) and a thin `main.rs`. Modules: `cli` (clap definitions), `dsl` (hand-rolled line-oriented parser), `model` (tree + collapse transform), `render` (Renderer trait, `chart` and `tree` engines, shared `Charset`), `watch`. The DSL parser is hand-rolled for human-quality, line-numbered errors.

**Tech Stack:** Rust (stable), `clap` v4 derive, `serde`/`serde_json`, `notify`; dev-deps `assert_cmd`, `tempfile`.

**Spec:** `docs/superpowers/specs/2026-10-06-ascii-org-chart-design.md` — the plan argues from the spec; read both.

## Global Constraints

- Crate/package name: `orgchart`. Edition: whatever `cargo new` defaults to.
- All width calculations use `s.chars().count()` (never byte length) — names may be multi-byte.
- Exact parser error strings (single quotes and punctuation verbatim), rendered `error: line {N}: {message}` (or `error: {message}` when there is no line):
  - `tab character in indentation; replace tabs with spaces`
  - `indent of {n} spaces; expected a multiple of {unit} (unit established on line {first_indented_line})`
  - `indent of {n} spaces; expected at most {limit} (one level deeper than line {prev_content_line})`
  - `indented line with no parent`
  - `missing closing ']'`
  - `unexpected text after ']'`
  - `missing person name`
  - `empty tag`
  - `empty organization — at least one person is required` (no line number)
- Exit codes: parse/IO error → 1 printed to stderr; usage error → 2 (clap default). `check` prints nothing on success.
- Chart geometry constants: sibling gap = 2 columns; box inner width = `max(name_chars, title_chars) + 2`; extra centering pad goes to the right; every rendered row is right-trimmed.
- Collapse summary format: `{label} ({count} people)` where `label` = first tag if any else the name, `count` = whole subtree size including the node itself (always ≥ 2 when it triggers, so always plural).
- TDD: every task writes its failing test first. Commit after each task (conventional commits).

## Review Focus

The five input classes most likely to bite a real user, each pinned by a test in the owning task:

1. **CRLF line endings** (files saved on Windows/macOS tools) — must parse cleanly. Test in Task 3 (`parses_crlf`).
2. **Multi-byte names** (`José`, `Müller`) — byte-based widths would corrupt chart alignment. Test in Task 5 (`renders_multibyte_name_aligned`).
3. **`--max-depth 0`** — collapses every parent root to a summary, childless roots untouched. Test in Task 2 (`collapse_depth_zero`).
4. **Wide orgs** (one manager with 8 direct reports, 3 levels) — grid must not corrupt or misalign. Golden fixture in Task 5.
5. **Degenerate tag lists** (`[]`, `[CEO, ]`) — strict parser rejects with `empty tag`. Test in Task 3 (`rejects_empty_tags`).

---

### Task 1: Crate scaffold + CLI definition

**Files:**
- Create: `Cargo.toml`, `src/main.rs`, `src/lib.rs`, `src/cli.rs`
- Test: `src/cli.rs` (unit tests inline)

**Interfaces:**
- Produces (used by Tasks 4, 5, 7): `pub struct Cli` with fields `layout: Layout`, `ascii: bool`, `max_depth: Option<usize>`, `json: bool`, `watch: bool`, `output: Option<PathBuf>`, `path: Option<PathBuf>`, `command: Option<Command>`; `pub enum Layout { Chart, Tree }` (clap `ValueEnum`, `Chart` default); `pub enum Command { Check { path: PathBuf } }`.

- [ ] **Step 1: Scaffold and add dependencies**

```bash
cargo new --name orgchart .   # in the repo root; keep readme.md
cargo add clap --features derive
cargo add serde --features derive
cargo add serde_json
cargo add notify
cargo add --dev assert_cmd
cargo add --dev tempfile
```

Create `src/lib.rs` (empty module list for now) and `src/main.rs` that calls `cli::Cli::parse()` and exits 0.

- [ ] **Step 2: Write the failing test** — in `src/cli.rs`, unit tests for argument parsing:

```rust
#[test]
fn parses_all_flags() {
    let cli = Cli::try_parse_from(["orgchart", "--layout", "tree", "--ascii",
        "--max-depth", "2", "--json", "-o", "out.txt", "org.org"]).unwrap();
    assert!(matches!(cli.layout, Layout::Tree) && cli.ascii && cli.max_depth == Some(2)
        && cli.json && cli.output == Some("out.txt".into()) && cli.path == Some("org.org".into()));
}

#[test]
fn defaults_to_chart_layout() {
    let cli = Cli::try_parse_from(["orgchart", "f.org"]).unwrap();
    assert!(matches!(cli.layout, Layout::Chart) && !cli.ascii && cli.max_depth.is_none());
}

#[test]
fn parses_check_subcommand() {
    let cli = Cli::try_parse_from(["orgchart", "check", "f.org"]).unwrap();
    assert!(matches!(cli.command, Some(Command::Check { .. })));
}

#[test]
fn watch_conflicts_with_json() {
    assert!(Cli::try_parse_from(["orgchart", "--watch", "--json", "f.org"]).is_err());
}
```

- [ ] **Step 3: Run test to verify it fails** — `cargo test` — compile error (no `Cli`).

- [ ] **Step 4: Implement `src/cli.rs`** — clap derive: `Parser` on `Cli`, `ValueEnum` on `Layout`, `Subcommand` on `Command`. `#[arg(long, conflicts_with_all = ["json", "output"])]` on `watch`; `#[arg(long, conflicts_with = "watch")]` on `json`. `path` is optional positional; the literal `-` later means stdin (Task 7). `main.rs` parses and exits 0. Export `pub mod cli;` from `lib.rs`.

- [ ] **Step 5: Run test to verify it passes** — `cargo test` — PASS.

- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat: crate scaffold with clap CLI definition"`

### Task 2: Model + depth collapse

**Files:**
- Create: `src/model.rs`
- Test: `src/model.rs` (unit tests inline)

**Interfaces:**
- Consumes: nothing.
- Produces (used by Tasks 3, 4, 5, 6): `pub struct Person { pub name: String, pub tags: Vec<String> }`, `pub struct Node { pub person: Person, pub children: Vec<Node> }`, `impl Node { pub fn new(name: &str, tags: Vec<String>) -> Node; pub fn subtree_len(&self) -> usize }`, `pub fn collapse_to_depth(roots: Vec<Node>, max_depth: usize) -> Vec<Node>`.

- [ ] **Step 1: Write the failing tests** — inline in `src/model.rs`. Build the spec's org as a helper (Ada[CEO] → Grace[VP Engineering] → {Alan[Staff Eng], Edsger[Staff Eng]}, Kath[VP Data] → Margaret[Eng Manager]):

  - `collapse_depth_one`: `collapse_to_depth(org, 1)` → Ada unchanged with two children; each child replaced by childless node named `"VP Engineering (3 people)"` / `"VP Data (2 people)"` (first tag, subtree count including self).
  - `collapse_depth_zero`: root with children → summary node; a childless second root stays untouched.
  - `collapse_falls_back_to_name_when_no_tags`: node without tags, with children → `"Bob (2 people)"`.
  - `collapse_depth_larger_than_tree_is_noop`: `collapse_to_depth(org, 99)` deep-equals input.
  - `subtree_len_counts_self_and_descendants`.

- [ ] **Step 2: Run tests to verify they fail** — `cargo test` — compile error.

- [ ] **Step 3: Implement `src/model.rs`** — plain structs; `collapse_to_depth` recurses with depth (root = 0): when `depth == max_depth && !children.is_empty()`, replace the node with a childless summary node per the Global Constraints format; otherwise recurse into children. Export `pub mod model;`.

- [ ] **Step 4: Run tests to verify they pass** — `cargo test`.

- [ ] **Step 5: Commit** — `git commit -am "feat: org model with depth-collapse transform"`

### Task 3: DSL parser

**Files:**
- Create: `src/dsl.rs`
- Test: `src/dsl.rs` (unit tests inline)

**Interfaces:**
- Consumes: `model::{Node, Person}`.
- Produces (used by Tasks 5, 6, 7): `pub struct ParseError { pub line: Option<usize>, pub message: String }` with `impl Display` producing exactly the Global Constraints format; `pub fn parse(input: &str) -> Result<Vec<Node>, ParseError>` (fail-fast: returns the first error).

- [ ] **Step 1: Write the failing tests** — inline in `src/dsl.rs`. One test per rule; assert full structure or the exact error string:

  - `parses_simple_org` — spec example → correct names/tags/children nesting.
  - `parses_bullets` — `- Ada` and `* Bob` lines behave like bare lines.
  - `ignores_comments_and_blanks` — `# comment` lines and blank lines anywhere.
  - `parses_multiple_roots` — two unindented lines → `Vec` of two roots.
  - `trims_tag_whitespace` — `Ada [ CEO , Founder ]` → tags `["CEO", "Founder"]`.
  - `rejects_tab_indent` — exact message (Global Constraints).
  - `rejects_non_multiple_indent` — input `Ada\n  Bob\n Cy` → `error: line 3: indent of 1 spaces; expected a multiple of 2 (unit established on line 2)`.
  - `rejects_level_skip` — `Ada\n  Bob\n      Cy` (6 spaces) → `error: line 3: indent of 6 spaces; expected at most 4 (one level deeper than line 2)`.
  - `rejects_indented_first_line` — `  Bob` → `error: line 1: indented line with no parent`.
  - `infers_odd_unit` — unit 3: `Ada\n   Bob\n      Cy` (3 then 6 spaces) parses.
  - `allows_multi_level_dedent` — 0 → 6 → 2 indents in sequence parses (dedent by any number of units is legal).
  - `rejects_missing_close_bracket`, `rejects_text_after_bracket`, `rejects_missing_name` (`[CEO]` alone), `rejects_empty_tags` (both `[]` and `[CEO, ]`) — exact messages.
  - `rejects_empty_org` — `""` and comment-only input → `error: empty organization — at least one person is required`.
  - `parses_crlf` (Review Focus #1) — `"Ada [CEO]\r\n  Bob [X]\r\n"` parses; also trailing spaces after tags are trimmed.

- [ ] **Step 2: Run tests to verify they fail** — `cargo test`.

- [ ] **Step 3: Implement `src/dsl.rs`** — line-oriented scanner over physical lines (1-based numbers, so blanks/comments still count). Per line: strip trailing `\r`/whitespace; skip blank and `#`-prefixed. If leading whitespace contains `\t` → tab error. Count leading spaces. Track: `unit: Option<usize>` (set on first indented line, error text carries that line number), previous content line's level, and a `Vec<(indent, &mut Node)>` stack. Line at indent `n` with `n > 0` and stack empty → no-parent error. `n % unit != 0` → multiple error. `level = n / unit`; `level > prev_level + 1` → skip error. Pop stack while top indent `> n`; push new node under stack top. Body: strip optional `- `/`* ` bullet; name = text before first `[` (trimmed; empty → missing-name error); remainder must end with `]` (else missing-close error) with only whitespace after (else text-after error); split inside on `,`, trim; any empty → empty-tag error. If no person at all parsed → empty-org error (line `None`). Export `pub mod dsl;`.

- [ ] **Step 4: Run tests to verify they pass** — `cargo test`.

- [ ] **Step 5: Commit** — `git commit -am "feat: strict indentation-based DSL parser"`

### Task 4: Charset + tree renderer

**Files:**
- Create: `src/render/mod.rs`, `src/render/tree.rs`
- Test: `src/render/tree.rs` (unit tests inline)

**Interfaces:**
- Consumes: `model::Node`.
- Produces (used by Tasks 5, 7): `pub enum Charset { Unicode, Ascii }` and `pub trait Renderer { fn render(&self, roots: &[Node], charset: Charset) -> String }` in `render/mod.rs`; `pub struct TreeRenderer` implementing `Renderer`.

- [ ] **Step 1: Write the failing tests** — inline in `src/render/tree.rs`:

  - `renders_flat_org` — input Ada[CEO] with children Bob[Sales], Cy[Legal] → exactly:
    ```
    Ada [CEO]
    ├─ Bob [Sales]
    └─ Cy [Legal]
    ```
  - `renders_nested_org` — Ada → Bob → Dan[Rep], plus sibling Cy → exactly:
    ```
    Ada [CEO]
    ├─ Bob [Sales]
    │  └─ Dan [Rep]
    └─ Cy [Legal]
    ```
  - `renders_without_tags` — bare `Ada` root renders just `Ada` (no ` []`).
  - `renders_forest` — two roots, no separator between root blocks.
  - `renders_ascii_charset` — first test's org with `Charset::Ascii` → `|-- `/`` `-- `` prefixes and `|  ` continuation:
    ```
    Ada [CEO]
    |- Bob [Sales]
    `- Cy [Legal]
    ```

- [ ] **Step 2: Run tests to verify they fail** — `cargo test`.

- [ ] **Step 3: Implement** — `render/mod.rs`: `Charset`, `Renderer` trait. `tree.rs`: recursive render; prefix pieces (Unicode `├─ `, `└─ `, `│  `, `   `; per Global Constraints ASCII swaps `├`→`|`, `└`→`` ` ``, `│`→`|`, `─`→`-`); tags appended as ` [tag, tag]` when non-empty. Export `pub mod render;` from `lib.rs`.

- [ ] **Step 4: Run tests to verify they pass** — `cargo test`.

- [ ] **Step 5: Commit** — `git commit -am "feat: tree-style renderer with charset support"`

### Task 5: Chart renderer (default layout)

**Files:**
- Create: `src/render/chart.rs`, `tests/golden.rs`, `tests/fixtures/{sample,wide,forest,deep,unicode}.org`, `tests/golden/` (generated)
- Test: `src/render/chart.rs` (unit tests inline) + `tests/golden.rs`

**Interfaces:**
- Consumes: `model::Node`, `render::{Charset, Renderer}`.
- Produces (used by Task 7): `pub struct ChartRenderer` implementing `Renderer`.

**Layout algorithm (pin these rules exactly):**

- Box: inner width `= max(name_chars, title_chars) + 2` (title = *all* tags joined with `", "`; none if no tags); box height `= 3` (no tags) or `4`. Content lines centered, extra pad right.
- Measure pass (recursive), per node: measure children first; `block_w = Σ child_subtree_w + 2*(n-1)`; `subtree_w = max(box_w, block_w)`. Children block offsets: if `block_w >= box_w`, block starts at 0 and `box_x = subtree_w/2 - box_w/2`; else block starts at `box_w/2 - block_w/2` and `box_x = 0`. Invariant: **every node's box center sits at `subtree_w / 2`**. Height `= box_h + connector_rows + max(child heights)`; connector_rows `= 1` for exactly one child, `2` for multiple.
- Draw pass: a per-cell direction-bitmask grid (L=1, R=2, U=4, D=8; merge with `|=`) plus a text overlay grid for name/title characters. Boxes emit their borders as masks (`┌`={R,D}, `┐`={L,D}, `└`={R,U}, `┘`={L,U}, `─`={L,R}, `│`={U,D}); mask→char lookup for all 16 combinations (`┬`={L,R,D}, `┴`={L,R,U}, `├`={U,R,D}, `┤`={U,L,D}, `┼`=all). ASCII: mask containing both horizontal and vertical bits → `+`, horizontal-only → `-`, vertical-only → `|`.
- Connectors per parent with children: bottom border center gets `{D}` (renders `┬` on the border); with one child, one row of `│` at the shared center and the child's top border center gets `{U}` (`┴`); with multiple children, an elbow row (horizontal mask from first child center to last child center, `{U,L,R}` at parent center — `{L,R,U,D}` if it coincides with a child center, corner masks `{R,D}`/`{L,D}` at the extreme child centers, `┬` mask at interior child centers) then a drop row of `│` at each child center; each child's top border center gets `{U}`.
- Forest: lay each root's subtree side by side with gap 2, tops aligned, no connectors between roots. Every row right-trimmed.

- [ ] **Step 1: Write the failing unit tests** — inline in `src/render/chart.rs`, asserting exact strings:

  - `renders_single_node_without_tags`:
    ```
    ┌─────┐
    │ Ada │
    └─────┘
    ```
  - `renders_parent_single_child` — `Ada [CEO]` with child `Grace [VP Eng]`:
    ```
      ┌─────┐
      │ Ada │
      │ CEO │
      └──┬──┘
         │
    ┌────┴───┐
    │ Grace  │
    │ VP Eng │
    └────────┘
    ```
  - `renders_parent_two_children` — `Ada [CEO]` with children `Bob [Sales]`, `Cy [Legal]`:
    ```
           ┌─────┐
           │ Ada │
           │ CEO │
           └──┬──┘
        ┌─────┴────┐
        │          │
    ┌───┴───┐  ┌───┴───┐
    │  Bob  │  │  Cy   │
    │ Sales │  │ Legal │
    └───────┘  └───────┘
    ```
  - `renders_ascii_charset` — single-node case in ASCII (`+`/`-`/`|`).
  - `renders_multibyte_name_aligned` (Review Focus #2) — `José [Eng]` alone → box rows all the same display width as the border (chars().count, not bytes).
  - `renders_collapsed_summary_node` — a node named `VP Engineering (3 people)` (no tags) renders as a 3-line box.

- [ ] **Step 2: Run tests to verify they fail** — `cargo test`.

- [ ] **Step 3: Implement `src/render/chart.rs`** — measure + draw passes per the pinned rules above.

- [ ] **Step 4: Run tests to verify they pass** — `cargo test`.

- [ ] **Step 5: Write the golden-file integration test** — `tests/golden.rs`: for each fixture in `tests/fixtures/{sample,wide,forest,deep,unicode}.org` and each renderer (TreeRenderer, ChartRenderer): parse fixture, render with Unicode charset, compare to `tests/golden/{fixture}.{renderer}.txt`. If env `UPDATE_GOLDEN=1` is set, write the file instead of comparing. Fixture contents: the spec's Ada org (`sample`), a root with 8 children and 3 levels (`wide`, Review Focus #4), two roots (`forest`), a 4-level chain (`deep`), and a chart with `José` and `Müller` (`unicode`).

- [ ] **Step 6: Generate and review goldens** — `UPDATE_GOLDEN=1 cargo test` then **eyeball every golden file** (alignment, connectors, no ragged rows); fix layout bugs if any; re-run `cargo test` to PASS.

- [ ] **Step 7: Commit** — `git add -A && git commit -m "feat: chart-style renderer with golden tests"`

### Task 6: JSON export

**Files:**
- Modify: `src/model.rs` (add serde derives)
- Test: `src/model.rs` (inline)

**Interfaces:**
- Consumes: `model::{Node, Person}`.
- Produces (used by Task 7): `Node` serializes as `{"name": "...", "tags": [...], "children": [...]}` — `#[derive(Serialize)]` on both structs and `#[serde(flatten)]` on `Node.person`.

- [ ] **Step 1: Write the failing test** — inline: serialize `Ada [CEO]` with child `Bob [Sales]` via `serde_json::to_string`, assert the exact string `{"name":"Ada Lovelace","tags":["CEO"],"children":[{"name":"Bob","tags":["Sales"],"children":[]}]}`. (Key order follows field order; if flatten reorders, assert by parsing into `serde_json::Value` and comparing `Value`s instead.)

- [ ] **Step 2: Run test to verify it fails** — `cargo test`.

- [ ] **Step 3: Implement** — add derives to `model.rs`; no new modules.

- [ ] **Step 4: Run test to verify it passes** — `cargo test`.

- [ ] **Step 5: Commit** — `git commit -am "feat: JSON export of the org tree"`

### Task 7: CLI wiring (render, check, stdin, -o)

**Files:**
- Create: `src/pipeline.rs`
- Modify: `src/main.rs`
- Test: `tests/cli.rs` (assert_cmd)

**Interfaces:**
- Consumes: everything above. `Cli` flags map: `--ascii` → `Charset::Ascii`, `--layout` → renderer choice, `--max-depth` → `model::collapse_to_depth` before rendering/exporting.
- Produces: `pub fn process(cli: &Cli, input: &str) -> Result<String, String>` — formatted output (rendered chart or JSON) or the fully formatted error string (already `error: ...`). `main.rs`: reads input (file path, `-` or absent → stdin), calls `process`, writes to `-o` file or stdout, prints errors to stderr and exits 1; `check` parses only, exits 0 silently or 1 with the error.

- [ ] **Step 1: Write the failing integration tests** — `tests/cli.rs` with `assert_cmd` + `tempfile`:

  - `renders_file_to_stdout` — temp file with the 2-person org; `orgchart <file>` exit 0, stdout contains `┌` and `Ada`.
  - `tree_flag_and_ascii_flag` — `--layout tree --ascii` stdout contains `|-`.
  - `max_depth_collapses` — `--max-depth 1` on the spec org → stdout contains `(3 people)`.
  - `json_flag` — `--json` stdout parses as JSON with `name == "Ada Lovelace"`.
  - `output_flag_writes_file` — `-o out.txt` creates the file with the chart, stdout empty.
  - `reads_stdin` — no path arg, `.write_stdin("Ada [CEO]")` → stdout contains `Ada`.
  - `check_reports_parse_error` — `orgchart check badfile` (tab indent) → exit 1, stderr contains `error: line 1: tab character`.
  - `check_silent_on_success` — exit 0, empty stdout/stderr.
  - `render_reports_parse_error` — exit 1, stderr contains `error: line`.
  - `missing_file_is_io_error` — exit 1, stderr contains `error:` (message: `error: cannot read {path}: {io_error}`).

- [ ] **Step 2: Run tests to verify they fail** — `cargo test`.

- [ ] **Step 3: Implement `src/pipeline.rs` + `main.rs`** — per Interfaces above. Pipeline order: parse → collapse (if `--max-depth`) → `--json` ? serialize : render. `check` ignores render flags. Export `pub mod pipeline;`.

- [ ] **Step 4: Run tests to verify they pass** — `cargo test`.

- [ ] **Step 5: Commit** — `git commit -am "feat: wire CLI flags, stdin, output file, and check subcommand"`

### Task 8: Watch mode + readme

**Files:**
- Create: `src/watch.rs`
- Modify: `src/main.rs`, `readme.md`
- Test: `src/watch.rs` (unit tests inline)

**Interfaces:**
- Consumes: `pipeline::process`, `notify`.
- Produces: `pub fn watch(path: &Path, cli: &Cli) -> notify::Result<()>` — initial render, then on each watcher event: sleep 200 ms (collapsing event bursts), re-render. Each render: clear screen (`\x1b[2J\x1b[H`), then print output or the error (keep watching on parse errors). Ctrl-C terminates via default SIGINT (no handler). Extract `pub fn render_once(path: &Path, cli: &Cli) -> String` (clear-screen + output-or-error text) for testability.

- [ ] **Step 1: Write the failing tests** — inline in `src/watch.rs`, using `tempfile`:

  - `render_once_shows_chart` — temp file with valid org → returned string starts with the clear-screen escape and contains `┌`.
  - `render_once_shows_error_and_keeps_going` — temp file with a tab indent → string contains `error: line 1: tab character` (function returns, does not panic).

- [ ] **Step 2: Run tests to verify they fail** — `cargo test`.

- [ ] **Step 3: Implement `src/watch.rs`** — `notify::recommended_watcher` sending `()` into an mpsc channel; loop blocks on receive, sleeps 200 ms, drains the channel, calls `render_once`. `main.rs` dispatches to `watch` when `--watch` (requires a real file path; absent path → clap usage error via `required_if_eq`-style validation or manual exit-2 message `--watch requires a file argument`). Export `pub mod watch;`.

- [ ] **Step 4: Run tests to verify they pass** — `cargo test` (full suite green).

- [ ] **Step 5: Manual smoke test** — run `cargo run -- sample.org --watch`, edit the file, confirm re-render; Ctrl-C exits.

- [ ] **Step 6: Write `readme.md`** — DSL syntax with the spec example, rendered sample output, all flags, `check` subcommand, install via `cargo install --path .`.

- [ ] **Step 7: Commit** — `git add -A && git commit -m "feat: watch mode; docs: readme"`
