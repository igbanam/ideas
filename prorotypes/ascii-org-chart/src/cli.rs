use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Layout style used when rendering the chart.
#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum Layout {
    Chart,
    Tree,
}

/// Subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Validate an org file without rendering.
    Check { path: PathBuf },
}

/// orgchart — render ASCII org charts from an indentation-based DSL.
#[derive(Debug, Parser)]
#[command(name = "orgchart", version)]
pub struct Cli {
    /// Layout style for rendering.
    #[arg(long, value_enum, default_value = "chart")]
    pub layout: Layout,
    /// Render using pure ASCII characters only.
    #[arg(long)]
    pub ascii: bool,
    /// Limit rendering depth to the given level.
    #[arg(long)]
    pub max_depth: Option<usize>,
    /// Emit machine-readable JSON instead of a chart.
    #[arg(long, conflicts_with = "watch")]
    pub json: bool,
    /// Re-render on file changes.
    #[arg(long, conflicts_with_all = ["json", "output"])]
    pub watch: bool,
    /// Output file path.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Input org file ("-" for stdin).
    pub path: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
