# ASCII Org Chart — Design

Date: 2026-10-06
Status: Approved design, pending implementation plan

## Purpose

A Rust CLI tool that turns a plain-text org definition into an ASCII org
chart a human can paste into a terminal, README, or code comment. The input
is an indentation-based DSL (a multi-layered bullet list); the output is a
rendered tree of boxes and connectors. Built for humans first: helpful
errors, simple flags, readable output.

## Non-goals (v1)

- No library crate publishing (internal module structure only; can split
  into a workspace later if a programmatic audience emerges).
- No styling metadata beyond tags (no colors, IDs, cross-references).
- No interactive TUI editing.
- No config files — everything through CLI flags.

## DSL

The input is a line-oriented, indentation-based list. Example:

```text
# comments start with '#'
Ada Lovelace [CEO]
  Grace Hopper [VP Engineering]
    Alan Turing [Staff Eng]
    Edsger Dijkstra [Staff Eng]
  Katherine Johnson [VP Data]
    Margaret Hamilton [Eng Manager]
```

Rules:

- One person per line. Name is free text up to the first `[` (or end of
  line).
- Optional bullets: leading `-` or `*` before the name, stripped.
- Metadata: optional trailing `[tag, tag, ...]` list. The first tag is
  conventionally the person's title; the rest are free-form tags. Rendered
  as a second line inside the box.
- Indentation defines hierarchy. Spaces only — tabs are rejected with an
  error suggesting conversion. The indent unit is inferred from the first
  indented line and then enforced: every subsequent indent level must be an
  exact multiple of that unit.
- Indentation may increase by at most one unit per line. Skipping a level
  (e.g., jumping two units at once) is an error:
  `error: line 5: indent of 4 spaces; expected at most 2 (one level deeper than line 4)`.
  Decreases may be by any number of units (dedenting back to an ancestor
  is normal).
- Blank lines and full-line `#` comments are ignored anywhere.
- Multiple top-level (unindented) entries are allowed; they render as a
  forest side by side.
- Empty org (no people) is an error.

Error style (strict parser, line-numbered):

```
error: line 7: indent of 3 spaces; expected a multiple of 2 (unit established on line 2)
error: line 4: tab character in indentation; replace tabs with spaces
error: line 9: empty organization — at least one person is required
```

## Model

```rust
struct Person { name: String, tags: Vec<String> }
struct Node { person: Person, children: Vec<Node> }
```

The parser produces `Vec<Node>` (the forest). All renderers and the JSON
exporter consume this model. Depth-collapsing (`--max-depth`) transforms
the model once, before rendering.

## Rendering

Two layout engines, selected with `--layout`. **Chart is the default.**

### Chart style (default)

Classic corporate org chart: each person in a box, children arranged in a
horizontal row beneath the parent, connected by elbow lines.

```text
                    ┌──────────────┐
                    │ Ada Lovelace │
                    │     CEO      │
                    └──────┬───────┘
              ┌────────────┴────────────┐
    ┌─────────┴─────────┐     ┌────────┴────────┐
    │   Grace Hopper    │     │ Katherine Johnson│
    │  VP Engineering   │     │     VP Data     │
    └───┬───────────┬───┘     └────────┬────────┘
        │           │                  │
  ┌─────┴────┐ ┌────┴─────┐      ┌─────┴──────┐
  │ A.Turing │ │ Dijkstra │      │  Hamilton  │
  └──────────┘ └──────────┘      └────────────┘
```

Algorithm: recursively compute each subtree's width (max of own box width
vs. sum of children's widths plus gaps); place children left-to-right;
center the parent over its children span. Connectors are drawn between
rows using box-drawing characters.

### Tree style

`tree`-command output: names listed vertically with `│ ├ └` connectors.

```text
Ada Lovelace [CEO]
├─ Grace Hopper [VP Engineering]
│  ├─ Alan Turing [Staff Eng]
│  └─ Edsger Dijkstra [Staff Eng]
└─ Katherine Johnson [VP Data]
   └─ Margaret Hamilton [Eng Manager]
```

### Shared behaviors

- `--ascii` degrades box-drawing characters (`│` → `|`, `┌` → `+`, `─` →
  `-`) for terminals that don't handle Unicode.
- `--max-depth N`: subtrees deeper than N render as a single summary box
  like `Engineering (4 people)` (derived from the collapsed node's first
  tag, falling back to the name). Applies to both layouts, implemented as
  a model transformation before rendering.

## CLI

```
orgchart [OPTIONS] [FILE]      # render (default command)
orgchart check FILE             # validate-only: parse, report errors, no output

Options:
  --layout tree|chart   default: chart
  --ascii               ASCII-only connectors (default: Unicode)
  --max-depth N         collapse subtrees below depth N
  --json                print parsed tree as JSON instead of rendering
  --watch               re-render on file change (debounced ~200ms,
                        clears screen, keeps parse errors visible)
  -o, --output FILE     write to file instead of stdout
```

- FILE defaults to stdin (except `--watch`, which requires a real file).
- `--json` output shape mirrors the model: `[{"name": "...", "tags":
  [...], "children": [...]}]`.
- `--watch` uses the `notify` crate; on parse error it shows the error
  and keeps watching instead of exiting.

## Architecture (Approach A: single crate, hand-rolled)

One binary crate, module-per-concern:

```
src/
  main.rs      # CLI entry, arg parsing (clap)
  cli.rs       # flag definitions
  dsl.rs       # line-oriented parser → model, with line-numbered errors
  model.rs     # Person, Node, collapse(depth) transformation
  render/
    mod.rs     # Renderer trait: render(&[Node]) -> String
    chart.rs   # box/elbow layout engine
    tree.rs    # vertical tree layout engine
    collapse.rs # summary-box helpers (used by both engines)
  watch.rs     # --watch loop
```

Dependencies: `clap` (CLI), `serde` + `serde_json` (JSON export), `notify`
(file watching). The DSL parser is hand-rolled: the format is
line-oriented, and bespoke parsing gives the best human-facing error
messages (combinator libraries fight indentation-based formats and
produce worse errors).

Error handling: parse errors carry line numbers and are printed to
stderr; the process exits non-zero. Rendering cannot fail for a valid
tree.

## Testing

- **Parser**: unit tests for indentation consistency (mixed units, wrong
  multiples), tab rejection, bullets, comments, blank lines, missing
  closing `]`, empty org, multiple roots, tags with/without commas.
  Every error message asserted exactly.
- **Renderers**: golden-file tests — small `.txt` fixtures in
  `tests/fixtures/`, expected outputs committed alongside
  (`tests/golden/`), compared in integration tests. Covers: deep trees,
  wide trees (one team of 8+), long names, tags wrapping, `--ascii`,
  `--max-depth` in both layouts.
- **JSON**: round-trip a fixture through `--json` and re-parse.
- **CLI**: `assert_cmd`-style smoke tests for flag combinations and
  `check`.

## Open items deferred past v1

- Mermaid/Graphviz export (`--format mermaid`)
- Config file for default layout style
- Assistant nodes / dotted-line reporting (needs a second relation type)