//! Inputs a real repository holds and a fixture directory cannot.
//!
//! `fixtures/documents/` is fifteen small, well-formed files. A tree
//! this tool is actually pointed at holds a byte-order mark somebody's
//! editor added, a file that is not UTF-8 at all, a Windows-written
//! catalogue in UTF-16, a socket left behind by a dev server, a file the
//! caller cannot read, a path longer than Windows admits, a symlink that
//! points at itself, an empty file, and a minified bundle that is one
//! line of two hundred thousand characters. None of those can be checked
//! into git and half of them cannot exist on Windows at all.
//!
//! So the tree is **built at runtime**, and each case the platform
//! cannot express says so by name — see `skipped()`. A skip is never
//! reported as a pass.
//!
//! Every case asserts the same floor: the process does not panic, does
//! not hang, and exits 0, 1 or 2 — never on a signal. Two more hold
//! throughout, because they are what this tool is for:
//!
//! - **Never silently absent.** A file the caller named and the tool
//!   could not read is in the report carrying a refusal. A file that
//!   vanishes reads to whoever ran it as a file that was clean.
//! - **Never a character it found.** Both streams stay ASCII whatever
//!   went in, so a report pasted into a terminal, a diff or a ticket
//!   cannot carry the attack onward.

use std::fmt::Write as _;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const BINARY: &str = env!("CARGO_BIN_EXE_unicode-le");

/// Generous enough for a shared runner scanning a 1 MB line, tight
/// enough that a blocking read on a FIFO is a failure rather than a
/// coffee break.
const LIMIT: Duration = Duration::from_secs(60);

/// U+202E, the right-to-left override. One finding, in a file that
/// otherwise looks ordinary.
const RLO: char = '\u{202E}';

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Tree {
    root: PathBuf,
}

impl Tree {
    fn new(name: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "unicode-le-hazard-{name}-{}-{unique}",
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
        self.write_bytes(relative, contents.as_bytes())
    }

    fn write_bytes(&self, relative: &str, contents: &[u8]) -> PathBuf {
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
    /// The whole of stdout, parsed. Doubles as the assertion that stdout
    /// carries one JSON document and nothing else.
    fn report(&self) -> serde_json::Value {
        serde_json::from_str(&self.stdout).unwrap_or_else(|error| {
            panic!("stdout is not one JSON document ({error}): {}", self.stdout)
        })
    }

    /// The report's entry for a file whose path ends in `suffix`.
    fn file(&self, suffix: &str) -> serde_json::Value {
        let report = self.report();
        let files = report["files"].as_array().expect("a file list").clone();
        files
            .into_iter()
            .find(|file| {
                file["file"]
                    .as_str()
                    .is_some_and(|name| name.replace('\\', "/").ends_with(suffix))
            })
            .unwrap_or_else(|| panic!("{suffix} is not in the report: {}", self.stdout))
    }
}

/// Runs the binary and **fails rather than blocks**. A hang is one of
/// the two failure modes this file exists to catch — a FIFO with no
/// writer is one `read` away from an eternal CI job — so the child is
/// killed and the case is named.
///
/// Holds both streams to ASCII on **every** case rather than in one
/// test: that is the rule the whole tool rests on, and it now covers the
/// path as well as the finding, so there is no input in this file that
/// is allowed to break it.
fn run(case: &str, args: &[&str]) -> Run {
    let mut child = Command::new(BINARY)
        .args(args)
        // Never inherit the terminal: `--stdin` reads to end of stream,
        // and a child waiting on a keyboard that is not there is a hang
        // with an innocent explanation.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");

    // Drained on threads: a child writing more than a pipe buffer would
    // deadlock against a parent that waits before reading, and a report
    // over a large tree is bigger than a pipe buffer.
    let mut out = child.stdout.take().expect("stdout");
    let mut err = child.stderr.take().expect("stderr");
    let out = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = out.read_to_end(&mut buffer);
        buffer
    });
    let err = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = err.read_to_end(&mut buffer);
        buffer
    });

    let deadline = Instant::now() + LIMIT;
    let status = loop {
        match child.try_wait().expect("the child is waitable") {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("{case}: hung for {LIMIT:?} on {args:?}");
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    };

    let code = status.code().unwrap_or_else(|| {
        panic!("{case}: died on a signal rather than exiting ({status:?}) on {args:?}")
    });
    assert!(
        (0..=2).contains(&code),
        "{case}: exit {code} is outside the documented 0/1/2 on {args:?}"
    );

    let stdout = String::from_utf8_lossy(&out.join().expect("stdout thread")).into_owned();
    let stderr = String::from_utf8_lossy(&err.join().expect("stderr thread")).into_owned();
    assert!(stdout.is_ascii(), "{case}: stdout is not ASCII: {stdout}");
    assert!(stderr.is_ascii(), "{case}: stderr is not ASCII: {stderr}");
    Run {
        code,
        stdout,
        stderr,
    }
}

/// Scan a path and require a report. Exit 2 means the question was
/// malformed, which for a path that exists is a failure of this suite's
/// setup rather than a result.
fn scan(case: &str, path: &Path) -> Run {
    let run = run(case, &[&path.to_string_lossy()]);
    assert!(
        run.code < 2,
        "{case}: the run refused the question itself — {}",
        run.stderr
    );
    run
}

/// A case the platform cannot express. Named on stderr so a green run
/// still says what it did not check — a silent skip is a lie.
fn skipped(case: &str, why: &str) {
    eprintln!("SKIPPED {case}: {why}");
}

// ---------------------------------------------------------------- content

/// A byte-order mark is three invisible bytes Notepad, Excel and a
/// PowerShell redirect all add. Here it is not merely cosmetic: the
/// leading U+FEFF is the encoding and must not be a finding, the *same*
/// character later in the file must be, and the three bytes are **not**
/// stripped — so every offset after them is three higher than in the
/// file without one. A tool that quietly dropped them would report
/// offsets that address nothing.
#[test]
fn a_byte_order_mark_is_the_encoding_and_does_not_move_the_offsets() {
    let tree = Tree::new("bom");
    let body = format!("const label = \"{RLO}\";\n");
    let plain = tree.write("plain.ts", &body);
    let marked = tree.write("marked.ts", &format!("\u{feff}{body}"));

    let without = scan("bom-plain", &plain);
    let with = scan("bom-marked", &marked);
    assert_eq!(without.report()["summary"]["findings"], 1);
    assert_eq!(
        with.report()["summary"]["findings"],
        1,
        "a leading byte-order mark became a second finding: {}",
        with.stdout
    );

    let plain_offset = without.file("plain.ts")["findings"][0]["offset"]
        .as_u64()
        .expect("an offset");
    let marked_offset = with.file("marked.ts")["findings"][0]["offset"]
        .as_u64()
        .expect("an offset");
    assert_eq!(
        marked_offset,
        plain_offset + 3,
        "the byte-order mark was stripped, so every offset in the report is wrong by three"
    );
}

/// A U+FEFF that is not at the front is a zero-width no-break space
/// sitting inside a string, and that is the finding the leading one is
/// not.
#[test]
fn an_interior_byte_order_mark_is_a_finding() {
    let tree = Tree::new("interior-bom");
    let file = tree.write("a.ts", "const id = \"a\u{feff}b\";\n");
    let run = scan("interior-bom", &file);
    assert_eq!(run.code, 1);
    assert_eq!(run.file("a.ts")["findings"][0]["kind"], "invisible");
    assert_eq!(run.file("a.ts")["findings"][0]["codepoints"][0], "U+FEFF");
}

/// **Never silently absent.** A file that is not UTF-8 is in the report
/// carrying a refusal that names why. What is not allowed is a run that
/// reports on the files it could decode and says nothing about the one
/// it could not — that reads as a clean tree.
#[test]
fn an_undecodable_file_is_refused_by_name_rather_than_dropped() {
    for (case, bytes, reason, named) in [
        // Latin-1: no mark, no NUL, and still not UTF-8.
        (
            "latin1",
            vec![b'c', b'a', b'f', 0xE9, b'\n'],
            "binary_or_undecodable",
            "byte 3",
        ),
        // What Notepad writes when asked for "Unicode".
        (
            "utf16le",
            vec![0xFF, 0xFE, b'a', 0x00, b'b', 0x00],
            "encoding_unknown",
            "UTF-16LE",
        ),
        (
            "utf16be",
            vec![0xFE, 0xFF, 0x00, b'a', 0x00, b'b'],
            "encoding_unknown",
            "UTF-16BE",
        ),
        // `FF FE 00 00` also starts with the UTF-16LE mark; naming the
        // wrong encoding here would be a confident wrong answer.
        (
            "utf32le",
            vec![0xFF, 0xFE, 0x00, 0x00, b'a', 0x00, 0x00, 0x00],
            "encoding_unknown",
            "UTF-32LE",
        ),
        // A PNG header carries no NUL at all, so it is refused by the
        // UTF-8 decode rather than by the binary sniff. Pinned as two
        // cases because the refusal names *which* test caught it, and a
        // reader acts on that.
        (
            "png-header",
            vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A],
            "binary_or_undecodable",
            "not valid UTF-8 at byte 0",
        ),
        (
            "nul-blob",
            vec![b'G', b'I', b'F', b'8', 0x00, 0x3B],
            "binary_or_undecodable",
            "NUL",
        ),
    ] {
        let tree = Tree::new(case);
        tree.write("ok.ts", "const total = 1;\n");
        tree.write_bytes("suspect.dat", &bytes);
        let scanned = scan(case, tree.path());

        let suspect = scanned.file("suspect.dat");
        assert_eq!(suspect["refusals"][0]["reason"], reason, "{case}");
        let detail = suspect["refusals"][0]["detail"]
            .as_str()
            .expect("a detail")
            .to_string();
        assert!(detail.contains(named), "{case}: {detail}");
        assert_eq!(scanned.report()["summary"]["unexamined"], 1, "{case}");
        // A refusal alone never fails a run; `--strict` is the only way
        // one reaches the exit code.
        assert_eq!(scanned.code, 0, "{case}: {}", scanned.stderr);
        let strict = run(case, &["--strict", &tree.path().to_string_lossy()]);
        assert_eq!(
            strict.code, 2,
            "{case}: --strict did not insist on coverage"
        );
    }
}

/// The two refusals that mean nothing was read must never be reported
/// alongside findings from the same file: a partial answer over a file
/// that was never decoded is a fabricated one.
#[test]
fn a_file_that_was_never_decoded_carries_no_findings() {
    let tree = Tree::new("undecoded");
    // Bidi controls in a UTF-16 file. Decoded as UTF-8 they would be
    // invisible and unassigned characters that are not in the document.
    let mut bytes = vec![0xFF, 0xFE];
    for unit in format!("x{RLO}y").encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    tree.write_bytes("catalogue.txt", &bytes);
    let run = scan("undecoded", tree.path());
    let file = run.file("catalogue.txt");
    assert_eq!(file["findings"].as_array().expect("a list").len(), 0);
    assert_eq!(file["refusals"][0]["reason"], "encoding_unknown");
    assert_eq!(run.report()["summary"]["findings"], 0);
}

/// An empty file is a file. It belongs in the report with no findings —
/// which is different from not being there at all.
#[test]
fn an_empty_file_is_reported_as_a_file() {
    let tree = Tree::new("empty");
    tree.write("empty.ts", "");
    let run = scan("empty", tree.path());
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.report()["summary"]["files"], 1);
    assert_eq!(
        run.file("empty.ts")["refusals"]
            .as_array()
            .expect("a list")
            .len(),
        0,
        "an empty file is decodable, not a refusal"
    );
}

/// A NUL byte is the binary sniff, and a file that reaches it is refused
/// rather than read. What must not happen is a panic on a slice that is
/// not a character boundary.
#[test]
fn a_nul_byte_mid_file_refuses_rather_than_panics() {
    let tree = Tree::new("nul");
    let mut bytes = format!("const label = \"{RLO}\";\n").into_bytes();
    bytes.push(0);
    bytes.extend_from_slice(b"more text\n");
    tree.write_bytes("mixed.ts", &bytes);
    let run = scan("nul", tree.path());
    assert_eq!(
        run.file("mixed.ts")["refusals"][0]["reason"],
        "binary_or_undecodable"
    );
}

/// **The shape this tool is pointed at most, and the one that was
/// quadratic.** A minified bundle is one line holding the whole
/// document, and every finding on it needs a column counted along that
/// line. Non-ASCII on purpose: an ASCII line needs no counting at all,
/// so an ASCII case would prove nothing.
#[test]
fn one_very_long_line_with_a_finding_every_few_characters_completes() {
    let tree = Tree::new("long-line");
    let mut body = String::new();
    for index in 0..20_000 {
        let _ = write!(body, "caf\u{e9}{index}\u{200B}");
    }
    tree.write("bundle.min.js", &body);

    let started = Instant::now();
    let run = scan("long-line", tree.path());
    eprintln!(
        "hazards: 20,000 findings on one line in {:?}",
        started.elapsed()
    );
    assert_eq!(run.report()["summary"]["findings"], 20_000);
    // Every one of them is on line 1, which is the point of the case.
    assert_eq!(run.file("bundle.min.js")["findings"][0]["line"], 1);
}

/// The other end of the same axis: a hundred thousand short lines. The
/// line index is built once and binary searched, so this must be linear
/// too.
#[test]
fn a_hundred_thousand_lines_complete() {
    let tree = Tree::new("many-lines");
    let mut body = String::new();
    for line in 0..100_000 {
        if line % 100 == 0 {
            let _ = writeln!(body, "const x{line} = \"a\u{00A0}b\";");
            continue;
        }
        let _ = writeln!(body, "const y{line} = 1;");
    }
    tree.write("big.ts", &body);
    let run = scan("many-lines", tree.path());
    assert_eq!(run.report()["summary"]["findings"], 1_000);
}

/// A grapheme cluster of a thousand combining marks is one "character"
/// to a reader and a thousand codepoints to everything else. It must not
/// become a thousand findings, and it must not blow a stack.
#[test]
fn an_enormous_grapheme_cluster_is_one_line_and_does_not_recurse() {
    let tree = Tree::new("grapheme");
    let mut body = String::from("const e = \"e");
    for _ in 0..2_000 {
        body.push('\u{0301}');
    }
    body.push_str("\";\n");
    tree.write("a.ts", &body);
    let run = scan("grapheme", tree.path());
    // `non-nfc` is one finding per line, however long the cluster is.
    let findings = run.file("a.ts")["findings"].clone();
    let list = findings.as_array().expect("a list");
    assert_eq!(list.len(), 1, "{findings}");
    assert_eq!(list[0]["kind"], "non-nfc");
}

// ------------------------------------------------------------- filesystem

#[cfg(unix)]
fn symlink(original: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(original, link)
}

/// Windows needs Developer Mode or an elevated process to create one, so
/// a failure here is the platform refusing rather than the code being
/// wrong — the caller skips by name.
#[cfg(windows)]
fn symlink(original: &Path, link: &Path) -> std::io::Result<()> {
    if original.is_dir() {
        return std::os::windows::fs::symlink_dir(original, link);
    }
    std::os::windows::fs::symlink_file(original, link)
}

/// A symlink loop resolves to `ELOOP`. Named explicitly it is a
/// malformed question — the caller pointed at something that is not
/// there — and inside a walked tree it is skipped, because links are
/// never followed.
#[test]
fn a_symlink_loop_is_refused_rather_than_followed() {
    let tree = Tree::new("symlink-loop");
    let first = tree.path().join("loop-a.ts");
    let second = tree.path().join("loop-b.ts");
    if symlink(&second, &first).is_err() || symlink(&first, &second).is_err() {
        skipped("symlink-loop", "this platform refused to create a symlink");
        return;
    }

    let named = run("symlink-loop-named", &[&first.to_string_lossy()]);
    assert_eq!(named.code, 2, "{}", named.stderr);
    assert!(named.stdout.is_empty(), "a refusal wrote a report");

    // The same loop inside a tree does not stop the walk, and does not
    // become a file in the report.
    tree.write("ok.ts", "const total = 1;\n");
    let walked = scan("symlink-loop-walked", tree.path());
    assert_eq!(walked.report()["summary"]["files"], 1, "{}", walked.stdout);
}

/// A link out of the tree would have the scan reading files the caller
/// did not point it at, and reporting their paths. Never followed.
#[test]
fn a_symlink_out_of_the_tree_is_not_followed() {
    let outside = Tree::new("symlink-outside");
    let secret = outside.write("secret.ts", &format!("const x = \"{RLO}\";\n"));

    let tree = Tree::new("symlink-inside");
    tree.write("ok.ts", "const total = 1;\n");
    if symlink(&secret, &tree.path().join("link.ts")).is_err() {
        skipped(
            "symlink-outside",
            "this platform refused to create a symlink",
        );
        return;
    }
    let run = scan("symlink-outside", tree.path());
    assert_eq!(run.report()["summary"]["files"], 1, "{}", run.stdout);
    assert_eq!(
        run.code, 0,
        "a link out of the tree was read: {}",
        run.stderr
    );
}

/// The hang this file exists for. A FIFO with no writer blocks a `read`
/// forever, and `scan_file` reads straight to a `Vec`.
///
/// **What is asserted is the floor, not the answer.** A named pipe is
/// not a regular file, so the walk does not select it and the run
/// finishes with nothing to say about it — which this pins as "not
/// reported as a clean file", the part that matters. That it is also not
/// reported *at all* when named explicitly is recorded in the repo's
/// AGENTS.md as a known gap rather than asserted here as correct.
#[cfg(unix)]
#[test]
fn a_fifo_does_not_block_the_run() {
    let tree = Tree::new("fifo");
    // Shelled out rather than called through libc: `unsafe` is forbidden
    // crate-wide and a test is not an exemption.
    let made = Command::new("mkfifo")
        .arg(tree.path().join("pipe.ts"))
        .status()
        .is_ok_and(|status| status.success());
    if !made {
        skipped("fifo", "mkfifo is not available on this runner");
        return;
    }
    tree.write("ok.ts", "const total = 1;\n");

    let walked = scan("fifo-walked", tree.path());
    assert_eq!(walked.report()["summary"]["files"], 1, "{}", walked.stdout);

    let named = run(
        "fifo-named",
        &[&tree.path().join("pipe.ts").to_string_lossy()],
    );
    assert!(
        !named.stdout.contains("pipe.ts"),
        "a named pipe was reported as a scanned file: {}",
        named.stdout
    );
}

#[cfg(not(unix))]
#[test]
fn a_fifo_does_not_block_the_run() {
    skipped("fifo", "Windows has no FIFO in a directory tree");
}

/// A file the caller cannot read is a refusal in the report with the
/// operating system's own message, not a gap.
#[cfg(unix)]
#[test]
fn a_permission_denied_file_is_refused_by_name() {
    use std::os::unix::fs::PermissionsExt;

    let tree = Tree::new("denied");
    let denied = tree.write("denied.ts", "const total = 1;\n");
    tree.write("ok.ts", "const total = 1;\n");
    std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o000)).expect("chmod");
    if std::fs::read(&denied).is_ok() {
        skipped(
            "permission-denied",
            "this runner reads a mode-000 file anyway (root)",
        );
        // Left readable for the tree's own cleanup.
        let _ = std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o644));
        return;
    }

    let run = scan("denied", tree.path());
    let refused = run.file("denied.ts");
    assert_eq!(refused["refusals"][0]["reason"], "binary_or_undecodable");
    assert!(
        refused["refusals"][0]["detail"]
            .as_str()
            .expect("a detail")
            .contains("could not be read"),
        "{refused}"
    );
    assert_eq!(run.report()["summary"]["unexamined"], 1);
    let _ = std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o644));
}

#[cfg(not(unix))]
#[test]
fn a_permission_denied_file_is_refused_by_name() {
    skipped(
        "permission-denied",
        "Windows ACLs are not chmod; the unix case covers the read failure",
    );
}

/// Where Windows differs: `MAX_PATH` is 260 characters unless long paths
/// are enabled, so the creation itself is the platform's answer.
#[test]
fn a_path_over_260_characters_is_scanned_or_skipped_cleanly() {
    let tree = Tree::new("long-path");
    let mut deep = String::new();
    while deep.len() < 300 {
        deep.push_str("a-directory-with-a-long-name/");
    }
    deep.push_str("deep.ts");
    let target = tree.path().join(&deep);
    if std::fs::create_dir_all(target.parent().expect("a parent")).is_err()
        || std::fs::write(&target, format!("const x = \"{RLO}\";\n")).is_err()
    {
        skipped(
            "long-path",
            "this platform refused a path over 260 characters",
        );
        return;
    }

    let run = scan("long-path", tree.path());
    assert_eq!(run.code, 1, "{}", run.stderr);
    assert_eq!(run.file("deep.ts")["findings"][0]["kind"], "bidi-control");
}

/// Awkward names, ASCII and not. The non-ASCII ones are here rather
/// than in a separate case because the report now escapes them: a name
/// that is full-width or accented costs the report nothing and must not
/// break the walk.
#[test]
fn awkward_file_names_are_scanned() {
    let tree = Tree::new("names");
    let mut checked = 0;
    for name in [
        "with space.ts",
        "trailing.dots..ts",
        "-leading-dash.ts",
        "a'quote.ts",
        "\u{fc}nicode.ts",
        "\u{FF26}\u{FF29}\u{FF2C}\u{FF25}.ts",
        "\u{1D41A}stral.ts",
    ] {
        if std::fs::write(tree.path().join(name), "const total = 1;\n").is_err() {
            skipped("awkward-names", name);
            continue;
        }
        checked += 1;
    }
    assert!(checked > 0, "this filesystem refused every awkward name");
    // `scan` holds both streams to ASCII, so this also asserts that
    // every one of those names came back escaped rather than raw.
    let run = scan("names", tree.path());
    assert_eq!(run.report()["summary"]["files"], checked);
}

/// The same rule on the other surface. A model is handed the MCP frame,
/// and a frame that pasted a hostile path raw would reorder whatever
/// renders it — the log, the transcript, the review comment.
#[test]
fn the_mcp_server_escapes_a_hostile_file_name_too() {
    use std::io::Write as _;

    let tree = Tree::new("mcp-hostile");
    let hostile = format!("invoice{RLO}fdp.ts");
    if std::fs::write(tree.path().join(&hostile), "const total = 1;\n").is_err() {
        skipped(
            "mcp-hostile",
            "this filesystem refused a bidi control in a name",
        );
        return;
    }

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "unicode_le_scan",
            "arguments": { "path": tree.path().to_string_lossy() },
        },
    });
    let mut child = Command::new(BINARY)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the server starts");
    writeln!(child.stdin.as_mut().expect("stdin"), "{request}").expect("written");
    let output = child.wait_with_output().expect("the server finishes");

    let raw = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(raw.is_ascii(), "the MCP frame is not ASCII: {raw}");
    assert!(raw.contains("\\u202E"), "{raw}");

    // And it still round-trips: the path a model reads back opens.
    let response: serde_json::Value =
        serde_json::from_str(raw.lines().next().expect("a line")).expect("the reply is JSON");
    let named = response["result"]["structuredContent"]["data"]["report"]["files"][0]["file"]
        .as_str()
        .expect("a path")
        .to_string();
    assert!(named.ends_with(&hostile), "{named:?}");
    assert!(std::fs::read_to_string(&named).is_ok(), "{named:?}");
}

/// **The hole this suite found, now closed.**
///
/// The report-safety rule used to cover only what the scan *found* — a
/// finding carries `U+XXXX` and prose this crate wrote. The `file` field
/// was exempt because the crate does not author it: it is the path the
/// caller handed in, echoed back so it can be opened and grepped for.
/// That exemption is precisely what made it exploitable. A repository
/// holding a file whose *name* carries a right-to-left override produced
/// a report that reordered the terminal of whoever read it, through the
/// one string the rule did not reach.
///
/// The fix keeps both properties rather than trading one for the other,
/// because `\uXXXX` is JSON's own escape:
///
/// - **inert on the wire** — the raw bytes of stdout, a diff or a pull
///   request hold no character that can reorder anything;
/// - **unchanged for a machine** — a parser decodes the escape back to
///   the identical string, and this asserts that the decoded path still
///   opens the file it names.
#[test]
fn a_hostile_file_name_is_escaped_and_still_opens() {
    let tree = Tree::new("hostile-name");
    let hostile = format!("invoice{RLO}fdp.ts");
    let body = format!("const x = \"{RLO}\";\n");
    if std::fs::write(tree.path().join(&hostile), &body).is_err() {
        skipped(
            "hostile-name",
            "this filesystem refused a bidi control in a name",
        );
        return;
    }

    // `run` already holds both streams to ASCII, which is the first
    // half: nothing on the wire can render as anything.
    let run = scan("hostile-name", tree.path());
    assert_eq!(run.code, 1, "{}", run.stderr);
    assert!(
        run.stdout.contains("\\u202E"),
        "the hostile name is not escaped in the report: {}",
        run.stdout
    );
    assert!(
        run.stderr.contains("\\u202E"),
        "the hostile name is not escaped on stderr: {}",
        run.stderr
    );

    // The second half, and the one that makes the first affordable: a
    // parser gives the path back byte for byte, so it still opens.
    let report = run.report();
    let named = report["files"][0]["file"]
        .as_str()
        .expect("a path")
        .to_string();
    assert!(
        named.contains(RLO),
        "the escape did not decode back to the real name: {named:?}"
    );
    assert!(
        named.ends_with(&hostile),
        "the decoded path is not the file that was written: {named:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&named).expect("the reported path opens"),
        body,
        "the path in the report does not open the file it names"
    );

    // And the finding itself is still the finding.
    assert_eq!(report["files"][0]["findings"][0]["codepoints"][0], "U+202E");
}

// ---------------------------------------------------------------- stdin

/// A document arriving on a pipe can be UTF-16 as easily as a file can.
/// Read as a `String` that would be an I/O error rather than the named
/// refusal it is.
#[test]
fn a_utf16_document_on_stdin_is_refused_by_name() {
    let mut bytes = vec![0xFF, 0xFE];
    for unit in "const id = 1;\n".encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let run = piped("stdin-utf16", &bytes);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.report()["files"][0]["refusals"][0]["reason"],
        "encoding_unknown"
    );
}

#[test]
fn invalid_utf8_on_stdin_is_refused_by_name() {
    let run = piped("stdin-latin1", &[b'c', b'a', b'f', 0xE9, b'\n']);
    assert_eq!(
        run.report()["files"][0]["refusals"][0]["reason"],
        "binary_or_undecodable"
    );
}

/// An empty stdin is a document with nothing in it, not a hang and not a
/// malformed question.
#[test]
fn an_empty_stdin_is_a_clean_document() {
    let run = piped("stdin-empty", b"");
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.report()["summary"]["files"], 1);
    assert_eq!(run.report()["summary"]["findings"], 0);
}

/// Writes bytes to `--stdin` and reads the answer, with the same
/// deadline and the same ASCII floor as `run`.
fn piped(case: &str, bytes: &[u8]) -> Run {
    use std::io::Write as _;

    let mut child = Command::new(BINARY)
        .arg("--stdin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");
    if let Some(stdin) = child.stdin.as_mut() {
        // Deliberately unchecked: a child that refuses before draining
        // stdin closes the pipe under this write, and the exit code is
        // the answer either way.
        let _ = stdin.write_all(bytes);
    }
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("the child finishes");
    let code = output
        .status
        .code()
        .unwrap_or_else(|| panic!("{case}: died on a signal ({:?})", output.status));
    assert!((0..=2).contains(&code), "{case}: exit {code}");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(stdout.is_ascii(), "{case}: stdout is not ASCII: {stdout}");
    assert!(stderr.is_ascii(), "{case}: stderr is not ASCII: {stderr}");
    Run {
        code,
        stdout,
        stderr,
    }
}

// ------------------------------------------------------------- exit codes

/// Exit 2 is for a malformed question. A path the caller named and the
/// tool cannot resolve **is** one — there is no partial answer to give
/// when the input itself is the thing that failed.
#[test]
fn every_malformed_question_exits_two_and_leaves_stdout_empty() {
    let tree = Tree::new("exit-two");
    tree.write("a.ts", "const total = 1;\n");
    let here = tree.path().to_string_lossy().into_owned();
    let missing = tree
        .path()
        .join("not-here.ts")
        .to_string_lossy()
        .into_owned();

    for args in [
        vec![missing.as_str()],
        vec!["--not-a-flag", here.as_str()],
        vec!["--kind", "homoglyph", here.as_str()],
        vec!["--script", "Klingon", here.as_str()],
        vec!["--fail-on", "everything", here.as_str()],
        vec!["--kind", "", here.as_str()],
        vec!["--stdin", here.as_str()],
        vec![],
    ] {
        let refused = run("exit-two", &args);
        assert_eq!(refused.code, 2, "{args:?}: {}", refused.stderr);
        assert!(
            refused.stdout.is_empty(),
            "{args:?} wrote to the protocol stream"
        );
    }
}
