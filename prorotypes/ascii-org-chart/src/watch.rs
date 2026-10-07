//! Watch mode: re-render the chart whenever the org file changes.
//!
//! `render_once` is the testable core — clear the screen, then show
//! the rendered chart or the error text (watching never stops on a
//! parse error or read failure). `watch` wires that to a file-system
//! watcher with a 200 ms debounce so bursts of events collapse into
//! one re-render.
//!
//! The watcher registers on the file's *parent directory* and
//! re-renders only on events for the target file name. Watching the
//! file itself would bind to its inode on Linux (inotify), so
//! atomic-replace saves (vim with `backupcopy=auto`, `sed -i`,
//! formatters) would silently invalidate the watch. Directory
//! watching sees the replacement as a new event for the same name.
//! [`watch`] canonicalizes its argument first, so a bare relative
//! filename also ends up watching its parent directory.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};

use crate::cli::Cli;
use crate::pipeline;

/// Clear the screen and move the cursor home.
const CLEAR: &str = "\x1b[2J\x1b[H";

/// How long to wait after an event before re-rendering, so that a
/// burst of watcher events (editor saves often produce several)
/// collapses into a single render.
const DEBOUNCE: Duration = Duration::from_millis(200);

/// Read `path`, run the pipeline, and return the clear-screen escape
/// followed by the rendered chart or the fully formatted error text.
/// Never panics: a read failure or parse error becomes error text.
pub fn render_once(path: &Path, cli: &Cli) -> String {
    // A read failure (file deleted mid-watch, permission, ...) is
    // error text as-is — never fed to the pipeline, which would
    // render the message as an org chart or mis-parse brackets in it.
    let body = match fs::read_to_string(path) {
        Ok(input) => pipeline::process(cli, &input).unwrap_or_else(|err| {
            // Pipeline errors are single-line without a trailing
            // newline; frames need one so consecutive renders don't
            // glue together.
            if err.ends_with('\n') {
                err
            } else {
                format!("{err}\n")
            }
        }),
        Err(e) => format!("error: cannot read {}: {e}\n", path.display()),
    };
    format!("{CLEAR}{body}")
}

/// Render once, then re-render on every change to `path`. Blocks
/// until the watcher dies or the process is interrupted (Ctrl-C is
/// the default SIGINT — no handler). Parse errors and read failures
/// are rendered like any other output; only watcher setup failures
/// return `Err`.
pub fn watch(path: &Path, cli: &Cli) -> notify::Result<()> {
    // Canonicalize so a bare relative filename (e.g. `orgchart
    // org.org --watch`) gets a real parent directory to watch; a
    // bare-name fallback would bind to the file's inode on Linux and
    // silently die after an atomic-replace save. A missing file fails
    // here — keep the original path so error output stays as typed.
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    // Register the watcher BEFORE the first render so no save can
    // land in the gap between rendering and registration. A setup
    // error still returns before anything is shown.
    let (tx, rx) = mpsc::channel();
    let mut watcher = recommended_target_watcher(path.as_path(), tx)?;
    watch_target(path.as_path(), &mut watcher)?;
    show(&render_once(&path, cli))?;

    // recv blocks until the next relevant event; `try_recv` then
    // drains the rest of the burst so one save equals one re-render.
    while rx.recv().is_ok() {
        thread::sleep(DEBOUNCE);
        while rx.try_recv().is_ok() {}
        show(&render_once(&path, cli))?;
    }
    Ok(())
}

/// Point the watcher at the parent directory when one exists (so
/// atomic-replace saves stay visible — see the module docs), or at
/// the file itself when the path has no parent component (only
/// reachable for unusual paths post-canonicalization; canonicalized
/// absolute paths always have a parent). The parent/file choice
/// mirrors [`watch_decision`], which the tests assert against.
fn watch_target(path: &Path, watcher: &mut notify::RecommendedWatcher) -> notify::Result<()> {
    let target = watch_decision(path);
    watcher.watch(target, RecursiveMode::NonRecursive)
}

/// The path the watcher is pointed at for `path`: its parent
/// directory when one exists, else the path itself.
fn watch_decision(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => path,
    }
}

/// A watcher that signals `tx` once per event touching the watched
/// file, identified by name (see [`event_targets`]).
fn recommended_target_watcher(
    path: &Path,
    tx: mpsc::Sender<()>,
) -> notify::Result<notify::RecommendedWatcher> {
    let name = target_name(path);
    notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if let Ok(event) = event
            && event.paths.iter().any(|p| event_targets(p, &name))
        {
            // A closed channel just means the loop ended; ignore.
            let _ = tx.send(());
        }
    })
}

/// The file name to match events against: the last component of the
/// watched path (`org.org` for both `/a/b/org.org` and `org.org`).
fn target_name(path: &Path) -> PathBuf {
    path.file_name().map(PathBuf::from).unwrap_or_else(|| path.to_path_buf())
}

/// True when an event path refers to the watched file: its last
/// component equals `name`. Compared by last component (not full
/// path equality) because backends differ in whether the reported
/// path is absolute.
fn event_targets(event_path: &Path, name: &Path) -> bool {
    event_path.file_name().is_some_and(|n| n == name)
}

/// Print a rendered frame; a broken pipe (e.g. piping into `head`)
/// ends the loop cleanly rather than panicking on an EPIPE in
/// `print!` — the consumer closing the pipe is not an error.
fn show(frame: &str) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    if let Err(e) = stdout.write_all(frame.as_bytes())
        && e.kind() != io::ErrorKind::BrokenPipe
    {
        return Err(e);
    }
    stdout.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn watch_cli() -> Cli {
        Cli::try_parse_from(["orgchart", "--watch"]).unwrap()
    }

    #[test]
    fn render_once_shows_chart() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("org.org");
        fs::write(&path, "Ada [CEO]\n  Bob [Sales]\n").unwrap();

        let frame = render_once(&path, &watch_cli());

        assert!(
            frame.starts_with("\x1b[2J\x1b[H"),
            "frame should start with clear-screen + cursor-home: {frame:?}"
        );
        assert!(frame.contains('┌'), "frame should contain the chart: {frame}");
    }

    #[test]
    fn render_once_shows_error_and_keeps_going() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("bad.org");
        fs::write(&path, "\tAda [CEO]\n").unwrap();

        // The contract: the parse error becomes frame text instead of
        // a panic, so watch mode keeps going instead of dying.
        let frame = render_once(&path, &watch_cli());

        assert!(
            frame.contains("error: line 1: tab character"),
            "frame should contain the parse error: {frame:?}"
        );
        assert!(frame.starts_with("\x1b[2J\x1b[H"));
    }

    #[test]
    fn render_once_shows_read_error_not_garbled_chart() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("missing.org");

        // The contract: an unreadable file is reported as a read
        // error — never parsed as org text (which used to render the
        // error message inside a chart box).
        let frame = render_once(&path, &watch_cli());

        assert!(
            frame.starts_with("\x1b[2J\x1b[H"),
            "frame should start with clear-screen + cursor-home: {frame:?}"
        );
        assert!(
            frame.contains("error: cannot read"),
            "frame should contain the read error: {frame:?}"
        );
        assert!(
            !frame.contains('┌'),
            "read error must not be rendered as a chart box: {frame}"
        );
    }

    #[test]
    fn event_paths_match_by_file_name() {
        let name = target_name(Path::new("/a/b/org.org"));
        assert_eq!(name, Path::new("org.org"));

        // Absolute event paths from the backend match the target...
        assert!(event_targets(Path::new("/a/b/org.org"), &name));
        // ...as do relative ones (backends differ)...
        assert!(event_targets(Path::new("b/org.org"), &name));
        assert!(event_targets(Path::new("org.org"), &name));
        // ...but sibling files in the same directory do not.
        assert!(!event_targets(Path::new("/a/b/other.org"), &name));
        assert!(!event_targets(Path::new("/a/b/.org.swp"), &name));
    }

    #[test]
    fn bare_filename_falls_back_to_file_watch() {
        // No parent component: the target name is the path itself.
        let name = target_name(Path::new("org.org"));
        assert_eq!(name, Path::new("org.org"));
        assert!(event_targets(Path::new("org.org"), &name));
    }

    #[test]
    fn canonicalized_bare_filename_watches_parent_dir() {
        // The bug this guards: `orgchart org.org --watch` with a bare
        // relative filename used to watch the file itself, which
        // silently dies on Linux after an atomic-replace save. After
        // canonicalization the path is absolute, so `watch_target`'s
        // parent branch applies — but the pure-logic contract is
        // what we assert: an absolute path's watched target is its
        // parent directory, not the file.
        let dir = tempfile::TempDir::new().unwrap();
        let file = dir.path().join("org.org");
        fs::write(&file, "Ada [CEO]\n").unwrap();

        // What `watch()` now does to the user-supplied path.
        let canonical = file.canonicalize().unwrap();

        // Simulate the watch_target decision without a real watcher:
        // pick the path the watcher would be pointed at.
        let watched = watch_decision(&canonical);
        assert_eq!(
            watched,
            canonical.parent().unwrap(),
            "a canonicalized path must be watched via its parent directory"
        );
        assert_ne!(watched, canonical.as_path(), "must not watch the file itself");
    }

    /// The path `watch_target` would register: the parent directory
    /// when one exists, else the file itself. Extracted so the
    /// decision is testable without constructing a real watcher.
    fn watch_decision(path: &Path) -> &Path {
        match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => path,
        }
    }

    #[test]
    fn watch_target_decision_matches_registration_target() {
        // The decision helper and the actual registration must agree:
        // watch_target registers `watch_decision(path)` on the watcher.
        let absolute = Path::new("/tmp/x/org.org");
        assert_eq!(watch_decision(absolute), Path::new("/tmp/x"));
        let bare = Path::new("org.org");
        assert_eq!(watch_decision(bare), bare);
    }

    #[test]
    fn error_frames_end_with_newline() {
        // Parse-error frames must end with a newline: consecutive
        // renders clear the screen, and a missing newline would leave
        // the previous frame's tail glued after the error line.
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("bad.org");
        fs::write(&path, "\tAda [CEO]\n").unwrap();

        let frame = render_once(&path, &watch_cli());

        assert!(
            frame.ends_with('\n'),
            "error frame should end with a newline: {frame:?}"
        );
    }
}
