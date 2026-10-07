# orgchart

Render ASCII org charts from a plain-text, indentation-based DSL.

```
$ orgchart org.org
```

## DSL syntax

One person per line. Indentation defines the hierarchy: the first
indented line establishes the indent unit (any consistent number of
spaces; 2 is the convention), children are indented one unit deeper,
siblings equally. Lines may start with an optional `- ` or `* ` bullet
prefix, stripped before parsing. Blank lines and `#` comments are
ignored. A person is a name, optionally followed by `[tag...]`.

```org
# The spec's example org.
Ada Lovelace [CEO]
  Grace Hopper [VP Engineering]
    Alan Turing [Staff Eng]
    Edsger Dijkstra [Staff Eng]
  Katherine Johnson [VP Data]
    Margaret Hamilton [Eng Manager]
```

Rendered (`--layout chart`, the default):

```
                     ┌──────────────┐
                     │ Ada Lovelace │
                     │     CEO      │
                     └───────┬──────┘
                  ┌──────────┴──────────────────┐
                  │                             │
         ┌────────┴───────┐           ┌─────────┴─────────┐
         │  Grace Hopper  │           │ Katherine Johnson │
         │ VP Engineering │           │      VP Data      │
         └────────┬───────┘           └─────────┬─────────┘
       ┌──────────┴───────┐                     │
       │                  │           ┌─────────┴─────────┐
┌──────┴──────┐  ┌────────┴────────┐  │ Margaret Hamilton │
│ Alan Turing │  │ Edsger Dijkstra │  │    Eng Manager    │
│  Staff Eng  │  │    Staff Eng    │  └───────────────────┘
└─────────────┘  └─────────────────┘
```

Tabs in indentation are a parse error; replace them with spaces.

## Usage

```
$ orgchart [OPTIONS] [PATH]
```

`PATH` is the org file; `-` or a missing path reads stdin.

| Flag | Meaning |
|------|---------|
| `--layout chart\|tree` | Layout style (default: `chart`). |
| `--ascii` | Pure ASCII (`+`, `-`, `|`) instead of box-drawing characters. |
| `--max-depth N` | Collapse everything at levels deeper than N into a summary box at level N. |
| `--json` | Emit the org as machine-readable JSON (array of node objects with `name`, `tags`, `children`) instead of a chart. |
| `--watch` | Re-render on every file change (see below). |
| `-o, --output FILE` | Write to FILE instead of stdout. |

Exit codes: `0` success, `1` error (parse or IO, message on stderr), `2`
usage error (e.g. `--watch` without a file argument).

## Watch mode

```
$ orgchart org.org --watch
```

Renders the chart, then re-renders whenever the file changes on disk:
each frame clears the screen first, then draws the chart (debounced
200 ms so an editor save produces one re-render, not several). Parse
errors are displayed in place — fix the file and the chart comes
back; watch mode never exits on a parse error. `Ctrl-C` (SIGINT)
stops it. `--watch` conflicts with `--json` and `-o` and requires a
real file path (not stdin).

The watcher registers on the file's parent directory, so saves that
atomically replace the file (`sed -i`, vim with `backupcopy=auto`,
formatters) are still picked up.

## Checking without rendering

```
$ orgchart check org.org
```

Validates the file and prints nothing on success; parse errors go to
stderr with exit code 1. check is validate-only: `--watch` and
`-o/--output` cannot be combined with it (usage error, exit 2).

## JSON output

```
$ orgchart --json org.org
[{"name":"Ada Lovelace","tags":["CEO"],"children":[...]}]
```

Each node object has `name`, `tags` (possibly empty), and `children`
(an array, possibly empty).

> **Consumer depth limit:** orgs deeper than ~63 levels serialize fine,
> but default-configured serde consumers cannot *deserialize* the
> result — serde's 128 recursion limit counts each nesting level twice
> (node objects plus their `children` arrays). The CLI exports such
> orgs without error.

## Installation

Requires Rust (rustup.rs). Then, from this directory:

```
$ cargo install --path .
```

`orgchart` installs to `~/.cargo/bin` and can be run from anywhere.
