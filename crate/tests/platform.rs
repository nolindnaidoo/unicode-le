//! Behaviour that differs by operating system, asserted rather than
//! hoped.
//!
//! A sibling crate in this family shipped a release whose report used
//! `\` on Windows and `/` everywhere else — red on Windows CI for the
//! whole release before anyone looked, and a report that could not be
//! diffed against one produced on another machine. **This crate's report
//! carries a path per file and its stderr line carries one per finding**,
//! so that bug has more surface here than in most of the family, and
//! `scan::report_path` normalises to `/` on Windows for exactly this
//! reason.
//!
//! Two properties that sound contradictory and are not:
//!
//! - **Inside the report, separators are `/` everywhere.** That is what
//!   makes two machines' reports comparable.
//! - **In a refusal, the path comes back the way the caller typed it.** A
//!   message that rewrites its own path cannot be grepped for by the
//!   person who typed it.
//!
//! The rest is case folding, the Windows device names, line endings, and
//! stdin closing under the tool's feet.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

const BINARY: &str = env!("CARGO_BIN_EXE_unicode-le");

/// U+202E, the right-to-left override.
const RLO: char = '\u{202E}';

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Tree {
    root: PathBuf,
}

impl Tree {
    fn new(name: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "unicode-le-platform-{name}-{}-{unique}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a temporary directory");
        Self {
            root: std::fs::canonicalize(&root).expect("a canonical directory"),
        }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let target = self.root.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("a parent directory");
        }
        std::fs::write(&target, contents).expect("a file");
        target
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    fn report(&self) -> serde_json::Value {
        serde_json::from_str(&self.stdout).unwrap_or_else(|error| {
            panic!("stdout is not one JSON document ({error}): {}", self.stdout)
        })
    }
}

fn run_with(args: &[&str], environment: &[(&str, Option<&str>)]) -> Run {
    let mut command = Command::new(BINARY);
    command.args(args).stdin(Stdio::null());
    for (name, value) in environment {
        match value {
            Some(value) => command.env(name, value),
            None => command.env_remove(name),
        };
    }
    let output = command.output().expect("the binary runs");
    Run {
        code: output.status.code().expect("an exit code, never a signal"),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn run(args: &[&str]) -> Run {
    run_with(args, &[])
}

fn skipped(case: &str, why: &str) {
    eprintln!("SKIPPED {case}: {why}");
}

// ------------------------------------------------------------------ paths

/// **The bug the family now asserts everywhere.** A report produced on
/// Windows and a report produced on Linux over the same tree must spell
/// the same file the same way, or every line of a cross-machine diff is
/// noise.
#[test]
fn every_path_in_the_report_is_separated_by_forward_slashes() {
    let tree = Tree::new("separators");
    tree.write("src/deep/nested/a.ts", &format!("const x = \"{RLO}\";\n"));
    tree.write("src/deep/b.ts", "const total = 1;\n");

    let run = run(&[&tree.path().to_string_lossy()]);
    assert_eq!(run.code, 1, "{}", run.stderr);

    let report = run.report();
    let files = report["files"].as_array().expect("a file list");
    assert_eq!(files.len(), 2);
    for file in files {
        let named = file["file"].as_str().expect("a path");
        assert!(
            !named.contains('\\'),
            "the report spells a path with a backslash: {named}"
        );
    }
    assert!(
        files.iter().any(|file| file["file"]
            .as_str()
            .is_some_and(|name| name.ends_with("src/deep/nested/a.ts"))),
        "{}",
        run.stdout
    );

    // The human line is a projection of the report and carries the same
    // path, so it has to agree.
    assert!(
        run.stderr.contains("src/deep/nested/a.ts:1:12"),
        "the summary line does not carry the report's path: {}",
        run.stderr
    );
}

/// And the other half. A path the caller typed comes back spelled the
/// way the caller typed it: a refusal that helpfully normalised
/// separators would be impossible to grep for with the string that
/// produced it.
///
/// **Spelling, not encoding.** A non-ASCII codepoint in the path is
/// escaped as `\uXXXX` on both streams — see `escape` — because the
/// caller can hand this tool any name at all, including one whose own
/// characters reorder the line it is printed on. The two rules do not
/// collide: this one is about `/` versus `\` and about not rewriting
/// segments, and the temp path here is ASCII, so it comes back byte for
/// byte.
#[test]
fn a_refusal_names_the_path_the_caller_gave_it() {
    let tree = Tree::new("refusal-path");
    let missing = tree.path().join("no").join("such").join("file.ts");
    let given = missing.to_string_lossy().into_owned();

    let run = run(&[&given]);
    assert_eq!(run.code, 2, "{}", run.stderr);
    assert!(
        run.stderr.contains(&given),
        "the refusal rewrote the path it was given\n  given: {given}\n  said:  {}",
        run.stderr
    );
    assert!(run.stdout.is_empty(), "a refusal wrote to stdout");
}

/// `Catalogue.ts` and `catalogue.ts` are one file on macOS and Windows
/// and two on Linux. Either answer is correct; what must not happen is a
/// crash, or a report that disagrees with the filesystem about how many
/// files there are.
#[test]
fn a_case_folding_filesystem_gives_a_consistent_answer() {
    let tree = Tree::new("case-fold");
    tree.write("catalogue.ts", &format!("const x = \"{RLO}\";\n"));
    let folded = tree.path().join("CATALOGUE.TS");

    let run = run(&[&folded.to_string_lossy()]);
    let exists = folded.exists();
    assert_eq!(
        run.code < 2,
        exists,
        "the scan disagreed with the filesystem about whether {} exists: {}",
        folded.display(),
        run.stderr
    );
    if !exists {
        assert!(run.stdout.is_empty(), "a refusal wrote a report");
        return;
    }
    assert_eq!(run.report()["summary"]["findings"], 1);
}

/// `CON`, `PRN`, `AUX`, `NUL` and `COM1` are device names on Windows and
/// ordinary files everywhere else. The test asserts the tool **survives
/// the creation failing**, never that the files exist.
#[test]
fn reserved_windows_names_do_not_break_the_run() {
    let tree = Tree::new("reserved");
    let mut created = 0;
    for name in ["CON", "PRN", "AUX", "NUL", "COM1"] {
        if std::fs::write(tree.path().join(format!("{name}.ts")), "const x = 1;\n").is_err() {
            continue;
        }
        created += 1;
    }
    if created == 0 {
        skipped("reserved-names", "this platform refused every device name");
        return;
    }

    let run = run(&[&tree.path().to_string_lossy()]);
    assert!(
        (0..=1).contains(&run.code),
        "a device name broke the walk: {} {}",
        run.code,
        run.stderr
    );
    assert_eq!(run.report()["summary"]["files"], created);
}

// ------------------------------------------------------------ line endings

/// A CRLF file is what a Windows editor writes, and a lone CR is what a
/// pre-OS-X Mac wrote. The line a finding is on must not move because of
/// either — a carriage return is an ordinary character to this tool, not
/// a line break, and that is a decision rather than an oversight.
#[test]
fn line_endings_do_not_move_a_finding() {
    let tree = Tree::new("line-endings");
    for (case, separator, line) in [("lf", "\n", 2), ("crlf", "\r\n", 2), ("cr", "\r", 1)] {
        let body = format!("const a = 1;{separator}const b = \"{RLO}\";{separator}");
        let file = tree.write(&format!("{case}.ts"), &body);
        let run = run(&[&file.to_string_lossy()]);
        assert_eq!(run.code, 1, "{case}: {}", run.stderr);

        let report = run.report();
        let finding = &report["files"][0]["findings"][0];
        assert_eq!(finding["kind"], "bidi-control", "{case}");
        assert_eq!(
            finding["line"], line,
            "{case}: a carriage return changed which line the finding is on"
        );
    }
}

/// A CRLF file is not in NFD and must not be reported as though it were:
/// the normalization check splits on `\n` and the trailing `\r` rides
/// along on every line.
#[test]
fn crlf_alone_is_not_a_normalization_finding() {
    let tree = Tree::new("crlf-nfc");
    let file = tree.write("a.ts", "const a = 1;\r\nconst b = 2;\r\n");
    let run = run(&[&file.to_string_lossy()]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.report()["summary"]["findings"], 0);
}

// -------------------------------------------------------------- time zone

/// The report reads no clock at all — there is no timestamp in it, by
/// design, so two runs over an unchanged tree produce identical bytes.
/// Which means the answer cannot depend on `TZ`, and that matters
/// because Windows ignores the variable entirely: a suite that quietly
/// depended on it would be red there and nowhere else.
#[test]
fn the_answer_does_not_depend_on_the_time_zone() {
    let tree = Tree::new("tz");
    tree.write("src/a.ts", &format!("const x = \"{RLO}\";\n"));
    tree.write("src/b.md", "A price:\u{00A0}10.\n");
    let here = tree.path().to_string_lossy().into_owned();
    let args = [here.as_str()];

    let utc = run_with(&args, &[("TZ", Some("UTC"))]);
    let unset = run_with(&args, &[("TZ", None)]);
    let far = run_with(&args, &[("TZ", Some("Pacific/Kiritimati"))]);

    assert_eq!(utc.stdout, unset.stdout, "TZ=UTC differs from TZ unset");
    assert_eq!(utc.stdout, far.stdout, "the report moved with the clock");
    assert_eq!(utc.stderr, unset.stderr);
    assert_eq!(utc.stderr, far.stderr);
    assert_eq!(utc.code, unset.code);
    assert_eq!(utc.code, far.code);
}

// ------------------------------------------------------------------ stdin

/// **Assert the exit code, never the write.** A child that refuses
/// before draining stdin closes the pipe under the parent's feet; a test
/// that asserted the write succeeded was red on one platform and green
/// on the others in a sibling repo, for reasons that had nothing to do
/// with the code.
#[test]
fn a_child_that_refuses_early_does_not_fail_on_the_write() {
    let mut child = Command::new(BINARY)
        .arg("--not-a-flag")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");

    // Deliberately more than a pipe buffer, and deliberately unchecked.
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(&vec![b'x'; 1024 * 1024]);
        let _ = stdin.flush();
    }
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("the child finishes");
    assert_eq!(
        output.status.code(),
        Some(2),
        "a malformed question is exit 2 whatever happened to stdin"
    );
}

/// `--stdin` reads to end of stream. Closing it immediately is a caller
/// that went away, and the answer is an empty document rather than a
/// hang or a broken pipe.
#[test]
fn a_document_from_a_closed_stdin_is_an_empty_document() {
    let mut child = Command::new(BINARY)
        .arg("--stdin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("the child finishes");
    assert_eq!(output.status.code(), Some(0));
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout carries JSON");
    assert_eq!(report["files"][0]["file"], "<stdin>");
    assert_eq!(report["summary"]["findings"], 0);
}

/// The MCP server reads stdin to end of stream. Closing it immediately
/// is a client that went away, and the answer is a clean exit.
#[test]
fn the_mcp_server_exits_cleanly_when_stdin_closes_immediately() {
    let mut child = Command::new(BINARY)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the server starts");
    drop(child.stdin.take());

    let mut stdout = String::new();
    if let Some(pipe) = child.stdout.as_mut() {
        let _ = pipe.read_to_string(&mut stdout);
    }
    let status = child.wait().expect("the child finishes");
    assert_eq!(status.code(), Some(0), "stdout was {stdout:?}");
}
