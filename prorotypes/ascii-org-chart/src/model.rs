use serde::Serialize;

/// A person in the org chart.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Person {
    pub name: String,
    pub tags: Vec<String>,
}

/// A node in the org tree: a person and their direct reports.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Node {
    #[serde(flatten)]
    pub person: Person,
    pub children: Vec<Node>,
}

impl Node {
    /// Creates a childless node for the given person.
    pub fn new(name: &str, tags: Vec<String>) -> Node {
        Node {
            person: Person {
                name: name.to_string(),
                tags,
            },
            children: Vec::new(),
        }
    }

    /// Whole subtree size, including this node.
    pub fn subtree_len(&self) -> usize {
        1 + self.children.iter().map(Node::subtree_len).sum::<usize>()
    }

    /// Summary label used when collapsing: the first tag if any, else the name.
    fn label(&self) -> &str {
        self.person.tags.first().map(String::as_str).unwrap_or(&self.person.name)
    }
}

/// Collapse every subtree that starts deeper than `max_depth` into a single
/// summary node. Roots sit at depth 0; a node with children at exactly
/// `max_depth` is replaced by a childless summary node named
/// `{label} ({count} people)` where `count` is the whole subtree size
/// including the node itself. A node that already has no children is left
/// untouched.
pub fn collapse_to_depth(roots: Vec<Node>, max_depth: usize) -> Vec<Node> {
    roots
        .into_iter()
        .map(|node| collapse_node(node, max_depth, 0))
        .collect()
}

fn collapse_node(node: Node, max_depth: usize, depth: usize) -> Node {
    // Summary nodes drop their tags: the summary name's first component
    // ({first-tag-or-name}) already carries the department/title.
    if depth == max_depth && !node.children.is_empty() {
        let name = format!("{} ({} people)", node.label(), node.subtree_len());
        Node {
            person: Person {
                name,
                tags: Vec::new(),
            },
            children: Vec::new(),
        }
    } else {
        Node {
            person: node.person,
            children: node
                .children
                .into_iter()
                .map(|child| collapse_node(child, max_depth, depth + 1))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec org: Ada[CEO] → Grace[VP Engineering] → {Alan[Staff Eng], Edsger[Staff Eng]},
    /// Kath[VP Data] → Margaret[Eng Manager].
    fn spec_org() -> Vec<Node> {
        let alan = Node::new("Alan", vec!["Staff Eng".into()]);
        let edsger = Node::new("Edsger", vec!["Staff Eng".into()]);
        let grace = with_children(
            Node::new("Grace", vec!["VP Engineering".into()]),
            vec![alan, edsger],
        );
        let margaret = Node::new("Margaret", vec!["Eng Manager".into()]);
        let kath = with_children(
            Node::new("Kath", vec!["VP Data".into()]),
            vec![margaret],
        );
        vec![with_children(Node::new("Ada", vec!["CEO".into()]), vec![grace, kath])]
    }

    fn with_children(mut node: Node, children: Vec<Node>) -> Node {
        node.children = children;
        node
    }

    #[test]
    fn collapse_depth_one() {
        let collapsed = collapse_to_depth(spec_org(), 1);
        assert_eq!(collapsed.len(), 1);
        let ada = &collapsed[0];
        assert_eq!(ada.person.name, "Ada");
        assert_eq!(ada.person.tags, vec!["CEO".to_string()]);
        assert_eq!(ada.children.len(), 2);
        assert_eq!(ada.children[0].person.name, "VP Engineering (3 people)");
        assert!(ada.children[0].children.is_empty());
        assert_eq!(ada.children[1].person.name, "VP Data (2 people)");
        assert!(ada.children[1].children.is_empty());
    }

    #[test]
    fn collapsed_nodes_have_no_tags() {
        let collapsed = collapse_to_depth(spec_org(), 1);
        assert_eq!(collapsed[0].children.len(), 2);
        for vp in &collapsed[0].children {
            assert!(vp.person.tags.is_empty());
        }
        // Non-collapsed nodes keep their tags untouched.
        assert_eq!(collapsed[0].person.tags, vec!["CEO".to_string()]);
    }

    #[test]
    fn collapse_depth_zero() {
        let ada = spec_org().into_iter().next().unwrap();
        let solo = Node::new("Zoe", vec!["Consultant".into()]);
        let collapsed = collapse_to_depth(vec![ada, solo], 0);
        assert_eq!(collapsed.len(), 2);
        assert_eq!(collapsed[0].person.name, "CEO (6 people)");
        assert!(collapsed[0].children.is_empty());
        assert_eq!(collapsed[1].person.name, "Zoe");
        assert_eq!(collapsed[1].person.tags, vec!["Consultant".to_string()]);
        assert!(collapsed[1].children.is_empty());
    }

    #[test]
    fn collapse_falls_back_to_name_when_no_tags() {
        let bob = with_children(Node::new("Bob", vec![]), vec![Node::new("Alice", vec![])]);
        let collapsed = collapse_to_depth(vec![bob], 0);
        assert_eq!(collapsed.len(), 1);
        assert_eq!(collapsed[0].person.name, "Bob (2 people)");
        assert!(collapsed[0].children.is_empty());
    }

    #[test]
    fn collapse_depth_larger_than_tree_is_noop() {
        let org = spec_org();
        let collapsed = collapse_to_depth(org.clone(), 99);
        assert_eq!(collapsed, org);
    }

    #[test]
    fn json_export_serializes_node() {
        let bob = Node::new("Bob", vec!["Sales".into()]);
        let ada = with_children(
            Node::new("Ada Lovelace", vec!["CEO".into()]),
            vec![bob],
        );
        let json = serde_json::to_string(&ada).unwrap();
        assert_eq!(
            json,
            r#"{"name":"Ada Lovelace","tags":["CEO"],"children":[{"name":"Bob","tags":["Sales"],"children":[]}]}"#
        );
    }

    #[test]
    fn subtree_len_counts_self_and_descendants() {
        let org = spec_org();
        let ada = &org[0];
        assert_eq!(ada.subtree_len(), 6);
        let grace = &ada.children[0];
        assert_eq!(grace.subtree_len(), 3);
        assert_eq!(grace.children[0].subtree_len(), 1);
        assert_eq!(ada.children[1].subtree_len(), 2);
    }
}
