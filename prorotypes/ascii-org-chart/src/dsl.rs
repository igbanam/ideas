use crate::model::Node;
use std::fmt;

/// A parse failure: message plus the 1-based physical line it occurred on,
/// if any.
#[derive(Debug)]
pub struct ParseError {
    pub line: Option<usize>,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "error: line {}: {}", line, self.message),
            None => write!(f, "error: {}", self.message),
        }
    }
}

fn err(line: usize, message: impl Into<String>) -> ParseError {
    ParseError {
        line: Some(line),
        message: message.into(),
    }
}

/// Parse org DSL text into a forest of nodes. Fail-fast: returns the first
/// error encountered.
pub fn parse(input: &str) -> Result<Vec<Node>, ParseError> {
    // Strip a leading UTF-8 BOM (\u{feff}) before anything else:
    // some editors emit it, and it would otherwise glue itself to
    // the first person's name.
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let mut flat: Vec<(usize, Node)> = Vec::new();
    // Indent unit, established by the first indented line.
    let mut unit: Option<usize> = None;
    let mut first_indented_line = 0;
    // (level, physical line) of the previous content line.
    let mut prev: Option<(usize, usize)> = None;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim_end();
        if line.is_empty() {
            continue;
        }
        let mut indent = 0usize;
        let mut has_tab = false;
        let mut body_start = line.len();
        for (i, c) in line.char_indices() {
            match c {
                ' ' => indent += 1,
                '\t' => has_tab = true,
                _ => {
                    body_start = i;
                    break;
                }
            }
        }
        let body = &line[body_start..];
        // Full-line comments are ignored anywhere, indent or not.
        if body.starts_with('#') {
            continue;
        }
        if has_tab {
            return Err(err(
                line_no,
                "tab character in indentation; replace tabs with spaces",
            ));
        }
        // The very first content line cannot be indented: no parent exists.
        let level = if indent > 0 && prev.is_none() {
            return Err(err(line_no, "indented line with no parent"));
        } else if indent > 0 {
            if unit.is_none() {
                unit = Some(indent);
                first_indented_line = line_no;
            }
            let u = unit.unwrap();
            if !indent.is_multiple_of(u) {
                return Err(err(
                    line_no,
                    format!(
                        "indent of {} spaces; expected a multiple of {} (unit established on line {})",
                        indent, u, first_indented_line
                    ),
                ));
            }
            indent / u
        } else {
            0
        };
        // Indentation may increase by at most one level per line.
        if let Some((prev_level, prev_line)) = prev
            && level > prev_level + 1
        {
            let limit = (prev_level + 1) * unit.unwrap_or(indent);
            return Err(err(
                line_no,
                format!(
                    "indent of {} spaces; expected at most {} (one level deeper than line {})",
                    indent, limit, prev_line
                ),
            ));
        }
        let node = parse_body(line_no, body)?;
        flat.push((level, node));
        prev = Some((level, line_no));
    }

    if flat.is_empty() {
        return Err(ParseError {
            line: None,
            message: "empty organization — at least one person is required".to_string(),
        });
    }

    Ok(build_forest(&flat))
}

/// Strip an optional `- ` or `* ` bullet from a line body.
fn strip_bullet(body: &str) -> &str {
    if let Some(rest) = body.strip_prefix("- ") {
        rest
    } else if let Some(rest) = body.strip_prefix("* ") {
        rest
    } else {
        body
    }
}

/// Parse one content line body (indent stripped) into a node.
fn parse_body(line_no: usize, body: &str) -> Result<Node, ParseError> {
    let body = strip_bullet(body);
    let (name, tags) = match body.find('[') {
        None => {
            let name = body.trim();
            if name.is_empty() {
                return Err(err(line_no, "missing person name"));
            }
            (name, Vec::new())
        }
        Some(pos) => {
            let name = body[..pos].trim();
            if name.is_empty() {
                return Err(err(line_no, "missing person name"));
            }
            let rest = &body[pos + 1..];
            let end = match rest.find(']') {
                Some(end) => end,
                None => return Err(err(line_no, "missing closing ']'")),
            };
            if !rest[end + 1..].trim().is_empty() {
                return Err(err(line_no, "unexpected text after ']'"));
            }
            let mut tags = Vec::new();
            for part in rest[..end].split(',') {
                let tag = part.trim();
                if tag.is_empty() {
                    return Err(err(line_no, "empty tag"));
                }
                tags.push(tag.to_string());
            }
            (name, tags)
        }
    };
    Ok(Node::new(name, tags))
}

/// Assemble the flat (level, node) list into a forest. Succeeding entries at
/// `parent_level + 1` become children; a dedent by any number of levels ends
/// the current subtree and bubbles back up to the ancestor at that level.
fn build_forest(flat: &[(usize, Node)]) -> Vec<Node> {
    let mut pos = 0;
    let mut roots = Vec::new();
    while pos < flat.len() && flat[pos].0 == 0 {
        let mut node = flat[pos].1.clone();
        pos += 1;
        node.children = build_children(flat, &mut pos, 0);
        roots.push(node);
    }
    roots
}

fn build_children(flat: &[(usize, Node)], pos: &mut usize, parent_level: usize) -> Vec<Node> {
    let mut children = Vec::new();
    while *pos < flat.len() && flat[*pos].0 == parent_level + 1 {
        let mut node = flat[*pos].1.clone();
        *pos += 1;
        node.children = build_children(flat, pos, parent_level + 1);
        children.push(node);
    }
    children
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::model::Node;

    fn with_children(mut node: Node, children: Vec<Node>) -> Node {
        node.children = children;
        node
    }

    #[test]
    fn parses_simple_org() {
        let input = concat!(
            "# comments start with '#'\n",
            "Ada Lovelace [CEO]\n",
            "  Grace Hopper [VP Engineering]\n",
            "    Alan Turing [Staff Eng]\n",
            "    Edsger Dijkstra [Staff Eng]\n",
            "  Katherine Johnson [VP Data]\n",
            "    Margaret Hamilton [Eng Manager]\n",
        );
        let org = parse(input).unwrap();
        let expected = vec![with_children(
            Node::new("Ada Lovelace", vec!["CEO".into()]),
            vec![
                with_children(
                    Node::new("Grace Hopper", vec!["VP Engineering".into()]),
                    vec![
                        Node::new("Alan Turing", vec!["Staff Eng".into()]),
                        Node::new("Edsger Dijkstra", vec!["Staff Eng".into()]),
                    ],
                ),
                with_children(
                    Node::new("Katherine Johnson", vec!["VP Data".into()]),
                    vec![Node::new("Margaret Hamilton", vec!["Eng Manager".into()])],
                ),
            ],
        )];
        assert_eq!(org, expected);
    }

    #[test]
    fn parses_bullets() {
        let org = parse("- Ada [CEO]\n  * Bob [Staff]\n").unwrap();
        let expected = vec![with_children(
            Node::new("Ada", vec!["CEO".into()]),
            vec![Node::new("Bob", vec!["Staff".into()])],
        )];
        assert_eq!(org, expected);
    }

    #[test]
    fn ignores_comments_and_blanks() {
        let input = concat!(
            "# header\n",
            "\n",
            "Ada [CEO]\n",
            "   \n",
            "  # indented note\n",
            "  Grace [VP]\n",
            "\n",
            "# footer\n",
        );
        let org = parse(input).unwrap();
        let expected = vec![with_children(
            Node::new("Ada", vec!["CEO".into()]),
            vec![Node::new("Grace", vec!["VP".into()])],
        )];
        assert_eq!(org, expected);
    }

    #[test]
    fn parses_multiple_roots() {
        let org = parse("Ada [CEO]\nBob [VP]\n").unwrap();
        assert_eq!(org.len(), 2);
        assert_eq!(org[0].person.name, "Ada");
        assert_eq!(org[1].person.name, "Bob");
        assert!(org[0].children.is_empty());
        assert!(org[1].children.is_empty());
    }

    #[test]
    fn trims_tag_whitespace() {
        let org = parse("Ada [ CEO , Founder ]\n").unwrap();
        assert_eq!(org.len(), 1);
        assert_eq!(org[0].person.name, "Ada");
        assert_eq!(
            org[0].person.tags,
            vec!["CEO".to_string(), "Founder".to_string()]
        );
    }

    #[test]
    fn rejects_tab_indent() {
        let err = parse("Ada\n\tBob\n").unwrap_err();
        assert_eq!(
            err.to_string(),
            "error: line 2: tab character in indentation; replace tabs with spaces"
        );
    }

    #[test]
    fn rejects_non_multiple_indent() {
        let err = parse("Ada\n  Bob\n Cy\n").unwrap_err();
        assert_eq!(
            err.to_string(),
            "error: line 3: indent of 1 spaces; expected a multiple of 2 (unit established on line 2)"
        );
    }

    #[test]
    fn rejects_level_skip() {
        let err = parse("Ada\n  Bob\n      Cy\n").unwrap_err();
        assert_eq!(
            err.to_string(),
            "error: line 3: indent of 6 spaces; expected at most 4 (one level deeper than line 2)"
        );
    }

    #[test]
    fn rejects_indented_first_line() {
        let err = parse("  Bob\n").unwrap_err();
        assert_eq!(err.to_string(), "error: line 1: indented line with no parent");
    }

    #[test]
    fn infers_odd_unit() {
        let org = parse("Ada\n   Bob\n      Cy\n").unwrap();
        let expected = vec![with_children(
            Node::new("Ada", vec![]),
            vec![with_children(
                Node::new("Bob", vec![]),
                vec![Node::new("Cy", vec![])],
            )],
        )];
        assert_eq!(org, expected);
    }

    #[test]
    fn allows_multi_level_dedent() {
        // Indents 0 -> 2 -> 4 -> 6 -> 2: from depth 6, dedenting two units
        // at once (back to Bob's level) is legal.
        let input = "Ada\n  Bob\n    Cy\n      Dee\n  Eve\n";
        let org = parse(input).unwrap();
        let expected = vec![with_children(
            Node::new("Ada", vec![]),
            vec![
                with_children(
                    Node::new("Bob", vec![]),
                    vec![with_children(
                        Node::new("Cy", vec![]),
                        vec![Node::new("Dee", vec![])],
                    )],
                ),
                Node::new("Eve", vec![]),
            ],
        )];
        assert_eq!(org, expected);
    }

    #[test]
    fn rejects_missing_close_bracket() {
        let err = parse("Ada [CEO\n").unwrap_err();
        assert_eq!(err.to_string(), "error: line 1: missing closing ']'");
    }

    #[test]
    fn rejects_text_after_bracket() {
        let err = parse("Ada [CEO] extra\n").unwrap_err();
        assert_eq!(err.to_string(), "error: line 1: unexpected text after ']'");
    }

    #[test]
    fn rejects_missing_name() {
        let err = parse("[CEO]\n").unwrap_err();
        assert_eq!(err.to_string(), "error: line 1: missing person name");
    }

    #[test]
    fn rejects_empty_tags() {
        let err = parse("Ada []\n").unwrap_err();
        assert_eq!(err.to_string(), "error: line 1: empty tag");
        let err = parse("Ada [CEO, ]\n").unwrap_err();
        assert_eq!(err.to_string(), "error: line 1: empty tag");
    }

    #[test]
    fn rejects_empty_org() {
        let err = parse("").unwrap_err();
        assert_eq!(err.line, None);
        assert_eq!(
            err.to_string(),
            "error: empty organization — at least one person is required"
        );
        let err = parse("# only a comment\n\n").unwrap_err();
        assert_eq!(
            err.to_string(),
            "error: empty organization — at least one person is required"
        );
    }

    #[test]
    fn parses_bom() {
        // A leading UTF-8 BOM must be stripped, not glued to the
        // first person's name (it would also poison --json output).
        let input = "\u{feff}Ada [CEO]\n  Bob [Sales]\n";
        let org = parse(input).unwrap();
        let expected = vec![with_children(
            Node::new("Ada", vec!["CEO".into()]),
            vec![Node::new("Bob", vec!["Sales".into()])],
        )];
        assert_eq!(org, expected);
        for node in &org {
            assert!(
                !node.person.name.contains('\u{feff}'),
                "no name may contain the BOM: {:?}",
                node.person.name
            );
        }

        // No BOM in the input: parsing is unchanged.
        let plain = parse("Ada [CEO]\n").unwrap();
        assert_eq!(plain[0].person.name, "Ada");
    }

    #[test]
    fn parses_crlf() {
        let org = parse("Ada [CEO] \r\n  Bob [X]   \r\n").unwrap();
        let expected = vec![with_children(
            Node::new("Ada", vec!["CEO".into()]),
            vec![Node::new("Bob", vec!["X".into()])],
        )];
        assert_eq!(org, expected);
    }
}
