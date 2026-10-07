use super::{Charset, Renderer};
use crate::model::Node;

/// Renders a forest in `tree`-command style: names listed vertically with
/// `├─` / `└─` connectors and `│` continuations.
pub struct TreeRenderer;

/// The four prefix pieces a row can start with, resolved for a charset.
struct Pieces {
    /// Prefix for a child that has siblings after it.
    branch: &'static str,
    /// Prefix for the last child of a parent.
    last: &'static str,
    /// Continuation indent under a non-last ancestor.
    vertical: &'static str,
    /// Continuation indent under a last ancestor.
    blank: &'static str,
}

const UNICODE: Pieces = Pieces {
    branch: "├─ ",
    last: "└─ ",
    vertical: "│  ",
    blank: "   ",
};

const ASCII: Pieces = Pieces {
    branch: "|- ",
    last: "`- ",
    vertical: "|  ",
    blank: "   ",
};

fn pieces_for(charset: Charset) -> Pieces {
    match charset {
        Charset::Unicode => UNICODE,
        Charset::Ascii => ASCII,
    }
}

/// One rendered row: its accumulated prefix plus the label text.
struct Row {
    prefix: String,
    label: String,
}

impl Row {
    /// The row's text, right-trimmed.
    fn text(&self) -> String {
        let full = format!("{}{}", self.prefix, self.label);
        full.trim_end().to_string()
    }
}

/// A person's label: name plus ` [tag, tag]` when tags are non-empty.
fn label(node: &Node) -> String {
    if node.person.tags.is_empty() {
        node.person.name.clone()
    } else {
        format!("{} [{}]", node.person.name, node.person.tags.join(", "))
    }
}

impl Renderer for TreeRenderer {
    fn render(&self, roots: &[Node], charset: Charset) -> String {
        let pieces = pieces_for(charset);
        let mut rows: Vec<Row> = Vec::new();
        for root in roots {
            // Roots carry no prefix; their children start at the first stem.
            walk(root, "", "", &pieces, &mut rows);
        }
        render_rows(&rows)
    }
}

/// Join the laid-out rows into final output, one per line, each right-trimmed.
fn render_rows(rows: &[Row]) -> String {
    let mut out = String::new();
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&row.text());
    }
    if !rows.is_empty() {
        out.push('\n');
    }
    out
}

/// Recursively lay out a subtree. `prefix` is this row's full prefix
/// (empty for roots); `child_indent` is the continuation string its
/// children build on (`vertical` per non-last ancestor, `blank` per last).
fn walk(node: &Node, prefix: &str, child_indent: &str, pieces: &Pieces, rows: &mut Vec<Row>) {
    rows.push(Row {
        prefix: prefix.to_string(),
        label: label(node),
    });
    for (i, child) in node.children.iter().enumerate() {
        let last = i + 1 == node.children.len();
        let stem = if last { pieces.last } else { pieces.branch };
        let cont = if last { pieces.blank } else { pieces.vertical };
        let child_prefix = format!("{}{}", child_indent, stem);
        let grandchild_indent = format!("{}{}", child_indent, cont);
        walk(child, &child_prefix, &grandchild_indent, pieces, rows);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_children(mut node: Node, children: Vec<Node>) -> Node {
        node.children = children;
        node
    }

    /// The brief's flat org: Ada[CEO] with children Bob[Sales], Cy[Legal].
    fn flat_org() -> Vec<Node> {
        vec![with_children(
            Node::new("Ada", vec!["CEO".into()]),
            vec![
                Node::new("Bob", vec!["Sales".into()]),
                Node::new("Cy", vec!["Legal".into()]),
            ],
        )]
    }

    #[test]
    fn renders_flat_org() {
        let expected = "Ada [CEO]\n├─ Bob [Sales]\n└─ Cy [Legal]\n";
        assert_eq!(TreeRenderer.render(&flat_org(), Charset::Unicode), expected);
    }

    #[test]
    fn renders_nested_org() {
        let org = vec![with_children(
            Node::new("Ada", vec!["CEO".into()]),
            vec![
                with_children(
                    Node::new("Bob", vec!["Sales".into()]),
                    vec![Node::new("Dan", vec!["Rep".into()])],
                ),
                Node::new("Cy", vec!["Legal".into()]),
            ],
        )];
        let expected = concat!(
            "Ada [CEO]\n",
            "├─ Bob [Sales]\n",
            "│  └─ Dan [Rep]\n",
            "└─ Cy [Legal]\n",
        );
        assert_eq!(TreeRenderer.render(&org, Charset::Unicode), expected);
    }

    #[test]
    fn renders_without_tags() {
        let org = vec![Node::new("Ada", vec![])];
        let rendered = TreeRenderer.render(&org, Charset::Unicode);
        assert_eq!(rendered, "Ada\n");
        assert!(!rendered.contains("[]"));
    }

    #[test]
    fn renders_forest() {
        let org = vec![
            Node::new("Ada", vec!["CEO".into()]),
            Node::new("Bob", vec!["VP".into()]),
        ];
        let expected = "Ada [CEO]\nBob [VP]\n";
        assert_eq!(TreeRenderer.render(&org, Charset::Unicode), expected);
    }

    #[test]
    fn renders_ascii_charset() {
        let expected = "Ada [CEO]\n|- Bob [Sales]\n`- Cy [Legal]\n";
        assert_eq!(TreeRenderer.render(&flat_org(), Charset::Ascii), expected);
    }

    #[test]
    fn renders_ascii_continuation() {
        // A nested org in ASCII: the continuation under a non-last child is
        // `|  `, and under a last child `   ` (three spaces).
        let org = vec![with_children(
            Node::new("Ada", vec!["CEO".into()]),
            vec![
                with_children(
                    Node::new("Bob", vec!["Sales".into()]),
                    vec![Node::new("Dan", vec!["Rep".into()])],
                ),
                with_children(
                    Node::new("Cy", vec!["Legal".into()]),
                    vec![Node::new("Eve", vec!["Rep".into()])],
                ),
            ],
        )];
        let expected = concat!(
            "Ada [CEO]\n",
            "|- Bob [Sales]\n",
            "|  `- Dan [Rep]\n",
            "`- Cy [Legal]\n",
            "   `- Eve [Rep]\n",
        );
        assert_eq!(TreeRenderer.render(&org, Charset::Ascii), expected);
    }

    #[test]
    fn right_trims_rows_and_joins_multiple_tags() {
        let org = vec![with_children(
            Node::new("Ada", vec!["CEO".into(), "Founder".into()]),
            vec![Node::new("Bob", vec![])],
        )];
        let rendered = TreeRenderer.render(&org, Charset::Unicode);
        assert_eq!(rendered, "Ada [CEO, Founder]\n└─ Bob\n");
        for line in rendered.lines() {
            assert_eq!(line, line.trim_end());
        }
    }
}
