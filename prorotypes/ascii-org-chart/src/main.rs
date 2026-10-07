use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{self, ExitCode};

use clap::Parser;

use orgchart::cli::{Cli, Command};
use orgchart::pipeline;
use orgchart::watch;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

/// Read input (file path, `-` or absent → stdin), process it, and write
/// the result to the `-o` file or stdout. Errors are already fully
/// formatted (`error: ...`).
fn run(cli: &Cli) -> Result<(), String> {
    // check takes its own path; render modes read the positional arg.
    let source = match &cli.command {
        Some(Command::Check { path }) => Some(path.as_path()),
        None => cli.path.as_deref(),
    };

    // check is validate-only: render-mode flags that it would
    // silently ignore are rejected up front with a usage error
    // (exit 2) instead.
    if let Some(Command::Check { .. }) = &cli.command {
        if cli.watch {
            eprintln!("error: --watch cannot be combined with the check subcommand");
            process::exit(2);
        }
        if cli.output.is_some() {
            eprintln!("error: -o/--output cannot be combined with the check subcommand");
            process::exit(2);
        }
    }

    // Watch mode consumes a file live; there is nothing to watch on
    // stdin. Exit 2 (usage error) before touching stdin.
    if cli.watch {
        let path = match source {
            Some(path) if path != Path::new("-") => path,
            _ => {
                eprintln!("error: --watch requires a file argument");
                process::exit(2);
            }
        };
        return watch::watch(path, cli)
            .map_err(|e| format!("error: cannot watch {}: {e}", path.display()));
    }

    let input = read_input(source)?;

    let output = pipeline::process(cli, &input)?;

    // check prints nothing: skip the -o write path entirely (it would
    // otherwise truncate/create an empty output file). Redundant
    // with the exit-2 usage gate above (check + -o can't get this
    // far), but kept as a safety net for that contract.
    if matches!(cli.command, Some(Command::Check { .. })) {
        return Ok(());
    }

    match &cli.output {
        Some(path) => {
            if let Err(e) = fs::write(path, &output) {
                return Err(format!("error: cannot write {}: {e}", path.display()));
            }
        }
        None => write_stdout(&output)?,
    }
    Ok(())
}

/// Read the org text: a path (`-` means stdin), or stdin when no path
/// was given.
fn read_input(source: Option<&Path>) -> Result<String, String> {
    match source {
        Some(path) if path != Path::new("-") => {
            fs::read_to_string(path).map_err(|e| format!("error: cannot read {}: {e}", path.display()))
        }
        _ => {
            let mut buf = String::new();
            io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| format!("error: cannot read stdin: {e}"))?;
            Ok(buf)
        }
    }
}

fn write_stdout(text: &str) -> Result<(), String> {
    // Renderer output already ends with a trailing newline; don't add
    // another one.
    let mut stdout = io::stdout().lock();
    // A consumer closing the pipe (e.g. `orgchart big.org | head -1`)
    // is not an error: exit 0 silently rather than panicking on EPIPE.
    if let Err(e) = stdout.write_all(text.as_bytes())
        && e.kind() != io::ErrorKind::BrokenPipe
    {
        return Err(format!("error: cannot write stdout: {e}"));
    }
    Ok(())
}
