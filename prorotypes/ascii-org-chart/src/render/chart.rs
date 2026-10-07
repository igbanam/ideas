use super::{Charset, Renderer};
use crate::model::Node;

/// Direction bitmasks for connector cells.
const L: u8 = 1;
const R: u8 = 2;
const U: u8 = 4;
const D: u8 = 8;

/// Mask → Unicode box-drawing character, indexed by the 4-bit mask.
/// L=1, R=2, U=4, D=8: e.g. mask 5 (L|U) is `┘`, mask 11 (L|R|D) is `┬`.
const MASK_CHARS: [char; 16] = [
    ' ', // 0
    '─', // L
    '─', // R
    '─', // L|R
    '│', // U
    '┘', // L|U
    '└', // R|U
    '┴', // L|R|U
    '│', // D
    '┐', // L|D
    '┌', // R|D
    '┬', // L|R|D
    '│', // U|D
    '┤', // L|U|D
    '├', // R|U|D
    '┼', // L|R|U|D
];

fn mask_char(mask: u8, charset: Charset) -> char {
    match charset {
        Charset::Unicode => MASK_CHARS[mask as usize],
        Charset::Ascii => {
            let horizontal = mask & (L | R) != 0;
            let vertical = mask & (U | D) != 0;
            match (horizontal, vertical) {
                (true, true) => '+',
                (true, false) => '-',
                (false, true) => '|',
                (false, false) => ' ',
            }
        }
    }
}

/// A node's computed layout: box geometry within its subtree, the width
/// and height of the whole subtree, and the children's layouts and
/// offsets within the children block. `title` is the node's box title
/// line (all tags joined with `", "`; empty when it has no tags),
/// computed once here so the draw pass can reuse it.
struct Layout {
    box_x: usize,
    box_w: usize,
    box_h: usize,
    block_start: usize,
    connector_rows: usize,
    subtree_w: usize,
    height: usize,
    child_offsets: Vec<usize>,
    children: Vec<Layout>,
    title: String,
}

/// The title line inside a box: all tags joined with `", "`; empty when
/// the node has no tags.
fn title_line(node: &Node) -> String {
    if node.person.tags.is_empty() {
        String::new()
    } else {
        node.person.tags.join(", ")
    }
}

/// Measure pass: compute this subtree's layout. Children are measured
/// first; the children block is `Σ child subtree widths + 2-column gaps`;
/// `subtree_w = max(box_w, block_w)`. Invariant: every node's box center
/// sits at `subtree_w / 2`.
fn measure(node: &Node) -> Layout {
    let name_w = node.person.name.chars().count();
    let title = title_line(node);
    let title_w = title.chars().count();
    let box_w = name_w.max(title_w) + 2 + 2;
    let box_h = if node.person.tags.is_empty() { 3 } else { 4 };

    let children: Vec<Layout> = node.children.iter().map(measure).collect();
    let n = children.len();
    let block_w = if n == 0 {
        0
    } else {
        children.iter().map(|c| c.subtree_w).sum::<usize>() + 2 * (n - 1)
    };
    let subtree_w = box_w.max(block_w);
    let (block_start, box_x) = if n == 0 {
        (0, 0)
    } else if block_w >= box_w {
        (0, subtree_w / 2 - box_w / 2)
    } else {
        (box_w / 2 - block_w / 2, 0)
    };
    let connector_rows = match n {
        0 => 0,
        1 => 1,
        _ => 2,
    };
    let height = box_h + connector_rows + children.iter().map(|c| c.height).max().unwrap_or(0);

    let mut child_offsets = Vec::with_capacity(n);
    let mut offset = 0;
    for child in &children {
        child_offsets.push(offset);
        offset += child.subtree_w + 2;
    }

    Layout {
        box_x,
        box_w,
        box_h,
        block_start,
        connector_rows,
        subtree_w,
        height,
        child_offsets,
        children,
        title,
    }
}

/// The draw pass canvas: a per-cell direction-bitmask grid merged with
/// `|=` plus a text overlay for box content characters.
struct Grid {
    width: usize,
    masks: Vec<Vec<u8>>,
    text: Vec<Vec<char>>,
}

impl Grid {
    fn new(width: usize, height: usize) -> Grid {
        Grid {
            width,
            masks: vec![vec![0; width]; height],
            text: vec![vec![' '; width]; height],
        }
    }

    /// Merge a direction mask into a cell.
    fn mask(&mut self, row: usize, col: usize, m: u8) {
        self.masks[row][col] |= m;
    }

    /// Write `s` into the text overlay, centered in `width` columns with
    /// any odd padding going to the right.
    fn text(&mut self, row: usize, col: usize, width: usize, s: &str) {
        debug_assert!(
            width >= s.chars().count(),
            "text {s:?} does not fit in {width} columns"
        );
        let pad = width - s.chars().count();
        let start = col + pad / 2;
        for (i, c) in s.chars().enumerate() {
            self.text[row][start + i] = c;
        }
    }

    fn finish(&self, charset: Charset) -> String {
        let mut out = String::new();
        for row in 0..self.masks.len() {
            let mut line = String::new();
            for col in 0..self.width {
                let c = if self.text[row][col] != ' ' {
                    self.text[row][col]
                } else {
                    mask_char(self.masks[row][col], charset)
                };
                line.push(c);
            }
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }
}

/// Draw a subtree whose top-left corner of its bounding box is at
/// (`x`, `y`), laying out its box, connectors, and children recursively.
fn draw(node: &Node, l: &Layout, x: usize, y: usize, g: &mut Grid) {
    let bx = x + l.box_x;
    let inner = l.box_w - 2;

    // Box borders, as direction masks.
    for col in 0..l.box_w {
        let top = if col == 0 {
            R | D
        } else if col + 1 == l.box_w {
            L | D
        } else {
            L | R
        };
        g.mask(y, bx + col, top);
        let bottom = if col == 0 {
            R | U
        } else if col + 1 == l.box_w {
            L | U
        } else {
            L | R
        };
        g.mask(y + l.box_h - 1, bx + col, bottom);
    }
    for row in 1..l.box_h - 1 {
        g.mask(y + row, bx, U | D);
        g.mask(y + row, bx + l.box_w - 1, U | D);
    }

    // Box content, centered with extra pad to the right.
    g.text(y + 1, bx + 1, inner, &node.person.name);
    if l.box_h == 4 {
        g.text(y + 2, bx + 1, inner, &l.title);
    }

    if l.connector_rows == 0 {
        return;
    }

    // Connectors from this box down to its children.
    let n = l.children.len();
    let parent_c = bx + l.box_w / 2;
    g.mask(y + l.box_h - 1, parent_c, D);
    let elbow_y = y + l.box_h;
    let child_y = elbow_y + l.connector_rows;
    let centers: Vec<usize> = l
        .child_offsets
        .iter()
        .zip(&l.children)
        .map(|(&off, child)| x + l.block_start + off + child.subtree_w / 2)
        .collect();

    if n == 1 {
        // Single child: one vertical row at the shared center.
        g.mask(elbow_y, parent_c, U | D);
    } else {
        // Elbow row: horizontal from the first to the last child center,
        // corners at the extremes, `┬` at interior child centers, and the
        // parent's drop point at its own center.
        let first = centers[0];
        let last = centers[n - 1];
        for col in first + 1..last {
            g.mask(elbow_y, col, L | R);
        }
        g.mask(elbow_y, first, R | D);
        g.mask(elbow_y, last, L | D);
        for &c in &centers[1..n - 1] {
            g.mask(elbow_y, c, L | R | D);
        }
        if centers.contains(&parent_c) {
            g.mask(elbow_y, parent_c, L | R | U | D);
        } else {
            g.mask(elbow_y, parent_c, L | R | U);
        }
        // Drop row: one vertical at each child center.
        for &c in &centers {
            g.mask(elbow_y + 1, c, U | D);
        }
    }

    // Children, then `┴` where each child's top border meets its drop.
    for (i, child) in node.children.iter().enumerate() {
        let cl = &l.children[i];
        let cx = x + l.block_start + l.child_offsets[i];
        draw(child, cl, cx, child_y, g);
        g.mask(child_y, cx + cl.subtree_w / 2, U);
    }
}

/// Renders a forest as a classic corporate org chart: each person in a
/// box, children in a horizontal row beneath the parent, connected by
/// elbow lines. Roots sit side by side with a 2-column gap, tops aligned.
pub struct ChartRenderer;

impl Renderer for ChartRenderer {
    fn render(&self, roots: &[Node], charset: Charset) -> String {
        if roots.is_empty() {
            return String::new();
        }
        let layouts: Vec<Layout> = roots.iter().map(measure).collect();
        let total_w = layouts.iter().map(|l| l.subtree_w).sum::<usize>() + 2 * (layouts.len() - 1);
        let total_h = layouts.iter().map(|l| l.height).max().unwrap();
        let mut g = Grid::new(total_w, total_h);
        let mut x = 0;
        for (root, l) in roots.iter().zip(&layouts) {
            draw(root, l, x, 0, &mut g);
            x += l.subtree_w + 2;
        }
        g.finish(charset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_single_node_without_tags() {
        let org = vec![Node::new("Ada", vec![])];
        let expected = concat!("┌─────┐\n", "│ Ada │\n", "└─────┘\n",);
        assert_eq!(ChartRenderer.render(&org, Charset::Unicode), expected);
    }

    #[test]
    fn renders_parent_single_child() {
        let org = vec![Node::new("Ada", vec!["CEO".into()])];
        let mut ada = org.into_iter().next().unwrap();
        ada.children = vec![Node::new("Grace", vec!["VP Eng".into()])];
        let expected = concat!(
            "  ┌─────┐\n",
            "  │ Ada │\n",
            "  │ CEO │\n",
            "  └──┬──┘\n",
            "     │\n",
            "┌────┴───┐\n",
            "│ Grace  │\n",
            "│ VP Eng │\n",
            "└────────┘\n",
        );
        assert_eq!(ChartRenderer.render(&[ada], Charset::Unicode), expected);
    }

    #[test]
    fn renders_parent_two_children() {
        let org = vec![Node::new("Ada", vec!["CEO".into()])];
        let mut ada = org.into_iter().next().unwrap();
        ada.children = vec![
            Node::new("Bob", vec!["Sales".into()]),
            Node::new("Cy", vec!["Legal".into()]),
        ];
        let expected = concat!(
            "       ┌─────┐\n",
            "       │ Ada │\n",
            "       │ CEO │\n",
            "       └──┬──┘\n",
            "    ┌─────┴────┐\n",
            "    │          │\n",
            "┌───┴───┐  ┌───┴───┐\n",
            "│  Bob  │  │  Cy   │\n",
            "│ Sales │  │ Legal │\n",
            "└───────┘  └───────┘\n",
        );
        assert_eq!(ChartRenderer.render(&[ada], Charset::Unicode), expected);
    }

    #[test]
    fn renders_ascii_charset() {
        let org = vec![Node::new("Ada", vec![])];
        let expected = concat!("+-----+\n", "| Ada |\n", "+-----+\n",);
        assert_eq!(ChartRenderer.render(&org, Charset::Ascii), expected);
    }

    #[test]
    fn renders_multibyte_name_aligned() {
        // Width math must use chars().count(), not bytes: José is 5 bytes
        // but 4 chars, and the box must stay aligned regardless.
        let org = vec![Node::new("José", vec!["Eng".into()])];
        let rendered = ChartRenderer.render(&org, Charset::Unicode);
        let lines: Vec<&str> = rendered.lines().collect();
        assert_eq!(lines.len(), 4);
        let border_width = lines[0].chars().count();
        for line in &lines {
            assert_eq!(
                line.chars().count(),
                border_width,
                "row has wrong display width: {line:?}"
            );
        }
        assert!(rendered.contains("José"));
    }

    #[test]
    fn renders_cross_junction_when_parent_center_meets_child_center() {
        // Three equal leaf children ("Bob", box width 7, centers at
        // columns 3/12/21) under a parent whose box center lands on
        // column 12 too: the middle child's `┬` and the parent's drop
        // merge into `┼` at the elbow row's column 12.
        let org = vec![Node::new("Ada", vec![])];
        let mut ada = org.into_iter().next().unwrap();
        ada.children = vec![
            Node::new("Bob", vec![]),
            Node::new("Bob", vec![]),
            Node::new("Bob", vec![]),
        ];
        let rendered = ChartRenderer.render(&[ada], Charset::Unicode);
        let expected = concat!(
            "         ┌─────┐\n",
            "         │ Ada │\n",
            "         └──┬──┘\n",
            "   ┌────────┼────────┐\n",
            "   │        │        │\n",
            "┌──┴──┐  ┌──┴──┐  ┌──┴──┐\n",
            "│ Bob │  │ Bob │  │ Bob │\n",
            "└─────┘  └─────┘  └─────┘\n",
        );
        assert_eq!(rendered, expected);
        // And the `┼` sits exactly at display column 12 — the shared
        // parent/middle-child center (count chars, not bytes: `┌` is
        // multi-byte).
        let elbow = rendered.lines().nth(3).unwrap();
        let col = elbow.chars().take_while(|&c| c != '┼').count();
        assert_eq!(col, 12);
        assert!(elbow.chars().filter(|&c| c == '┼').count() == 1);
    }

    #[test]
    fn renders_ascii_junctions() {
        // ASCII degradation of the two-children case: `┬`/`┴`/elbow
        // corners all render as `+`, drops and box sides as `|`.
        let org = vec![Node::new("Ada", vec!["CEO".into()])];
        let mut ada = org.into_iter().next().unwrap();
        ada.children = vec![
            Node::new("Bob", vec!["Sales".into()]),
            Node::new("Cy", vec!["Legal".into()]),
        ];
        let rendered = ChartRenderer.render(&[ada], Charset::Ascii);
        let expected = concat!(
            "       +-----+\n",
            "       | Ada |\n",
            "       | CEO |\n",
            "       +--+--+\n",
            "    +-----+----+\n",
            "    |          |\n",
            "+---+---+  +---+---+\n",
            "|  Bob  |  |  Cy   |\n",
            "| Sales |  | Legal |\n",
            "+-------+  +-------+\n",
        );
        assert_eq!(rendered, expected);
    }

    #[test]
    fn renders_collapsed_summary_node() {
        // A collapsed summary node is an ordinary childless node: it
        // renders as a plain 3-line box.
        let org = vec![Node::new("VP Engineering (3 people)", vec![])];
        let rendered = ChartRenderer.render(&org, Charset::Unicode);
        let lines: Vec<&str> = rendered.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with('┌'));
        assert!(lines[1].contains("VP Engineering (3 people)"));
        assert!(lines[2].starts_with('└'));
    }
}
