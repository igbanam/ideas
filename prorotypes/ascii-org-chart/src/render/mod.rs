pub mod chart;
pub mod tree;

pub use chart::ChartRenderer;
pub use tree::TreeRenderer;

use crate::model::Node;

/// Character set for rendered output: Unicode box-drawing characters or a
/// plain-ASCII fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Charset {
    Unicode,
    Ascii,
}

/// A layout that renders a forest of org nodes to a string.
pub trait Renderer {
    fn render(&self, roots: &[Node], charset: Charset) -> String;
}
