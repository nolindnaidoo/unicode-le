//! The exit codes and the stdout contract, driven against the built
//! binary.
//!
//! These are the API: a shell branches on the exit code and parses
//! stdout, so both are pinned here rather than inferred from unit tests
//! of the functions behind them. Nothing here needs a network or a
//! privileged filesystem operation, so it runs everywhere on every push.
//!
//! A new refusal adds its case here.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

const BINARY: &str = env!("CARGO_BIN_EXE_unicode-le");
static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// U+202E, the right-to-left override — the character at the centre of
/// CVE-2021-42574.
const RLO: &str = "\u{202E}";
/// U+0430, Cyrillic small a, indistinguishable from Latin `a`.
const CYRILLIC_A: &str = "\u{430}";
/// U+00A0, the no-break space.
const NBSP: &str = "\u{00A0}";

struct Tree {
    root: PathBuf,
}

impl Tree {
    fn new(name: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "unicode-le-contract-{name}-{}-{unique}",
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

    fn write_bytes(&self, relative: &str, contents: &[u8]) -> PathBuf {
        let target = self.root.join(relative);
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

fn run(args: &[&str]) -> Run {
    let output = Command::new(BINARY)
        .args(args)
        .output()
        .expect("the binary runs");
    Run {
        code: output.status.code().expect("an exit code"),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// The whole of stdout, parsed. Doubles as the assertion that stdout is
/// one JSON document and nothing else — a stray human message there
/// would fail to parse.
fn report(run: &Run) -> serde_json::Value {
    serde_json::from_str(&run.stdout).expect("stdout carries one JSON document")
}

/// One bidi control, one homoglyph, one no-break space, and a file that
/// is entirely fine.
fn source_tree(name: &str) -> Tree {
    let tree = Tree::new(name);
    tree.write("src/render.ts", &format!("const label = \"{RLO}\";\n"));
    tree.write("src/auth.ts", &format!("const p{CYRILLIC_A}ypal = 1;\n"));
    tree.write("README.md", &format!("A price:{NBSP}10.\n"));
    tree.write("src/ok.ts", "const total = 1 + 2;\n");
    tree
}

#[test]
fn a_clean_tree_exits_zero() {
    let tree = Tree::new("clean");
    tree.write("src/a.ts", "const ok = 1;\n");
    let run = run(&[&tree.path().to_string_lossy()]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(report(&run)["summary"]["findings"], 0);
    assert!(run.stderr.contains("0 findings"), "{}", run.stderr);
}

#[test]
fn a_finding_exits_one() {
    let tree = source_tree("findings");
    let run = run(&[&tree.path().to_string_lossy()]);
    assert_eq!(run.code, 1, "{}", run.stderr);
    let report = report(&run);
    // The bidi control, the mixed word, the homoglyph in it, and the
    // no-break space.
    assert_eq!(report["summary"]["findings"], 4);
    assert_eq!(report["summary"]["bidi"], 1);
    assert_eq!(report["summary"]["files"], 4);
}

/// **The report-safety rule at the process boundary.** Everything this
/// writes is ASCII, so nothing it emits can render as anything — a
/// report pasted into a terminal, a diff or a ticket carries none of
/// what it found.
#[test]
fn neither_stream_carries_a_character_the_scan_found() {
    let tree = source_tree("safety");
    let run = run(&[&tree.path().to_string_lossy()]);
    assert!(run.stdout.is_ascii(), "{}", run.stdout);
    assert!(run.stderr.is_ascii(), "{}", run.stderr);
    for hazard in [RLO, CYRILLIC_A, NBSP] {
        assert!(!run.stdout.contains(hazard));
        assert!(!run.stderr.contains(hazard));
    }
    // And it did find them, so this proves something.
    assert!(run.stdout.contains("U+202E"), "{}", run.stdout);
    assert!(run.stdout.contains("U+0430"), "{}", run.stdout);
}

#[test]
fn stdout_carries_one_report_and_stderr_only_the_summary() {
    let tree = source_tree("streams");
    let run = run(&[&tree.path().to_string_lossy()]);
    assert_eq!(run.stdout.lines().count(), 1);
    assert_eq!(report(&run)["schema"], 1);
    assert!(!run.stderr.contains('{'), "{}", run.stderr);
    assert!(run.stderr.contains("findings in"), "{}", run.stderr);
}

/// `--fail-on bidi` is the security screen without the cleanliness pass,
/// and the exit code has to follow it or the flag means nothing.
#[test]
fn fail_on_bidi_ignores_everything_that_is_not_a_bidi_control() {
    let tree = Tree::new("failon");
    tree.write("src/a.md", &format!("A price:{NBSP}10.\n"));
    let path = tree.path().to_string_lossy().to_string();
    assert_eq!(run(&[&path]).code, 1, "any is the default");
    assert_eq!(run(&["--fail-on", "bidi", &path]).code, 0);

    tree.write("src/b.ts", &format!("const x = \"{RLO}\";\n"));
    assert_eq!(run(&["--fail-on", "bidi", &path]).code, 1);
}

#[test]
fn a_kind_filter_narrows_what_is_reported() {
    let tree = source_tree("kinds");
    let path = tree.path().to_string_lossy().to_string();
    let filtered = run(&["--kind", "bidi", &path]);
    assert_eq!(filtered.code, 1);
    assert_eq!(report(&filtered)["summary"]["findings"], 1);
}

/// **The false-positive guard, at the level a user meets it.** A tree of
/// translations produces no confusable findings and says why, rather
/// than producing one per word.
#[test]
fn a_tree_of_translations_is_refused_rather_than_flooded() {
    let tree = Tree::new("translations");
    tree.write(
        "locales/ru.json",
        "{\n  \"save\": \"\u{421}\u{43e}\u{445}\u{440}\u{430}\u{43d}\u{438}\u{442}\u{44c}\"\n}\n",
    );
    tree.write(
        "locales/zh.json",
        "{\n  \"save\": \"\u{4fdd}\u{5b58}\u{6587}\u{4ef6}\"\n}\n",
    );
    let path = tree.path().to_string_lossy().to_string();

    let undeclared = run(&[&path]);
    assert_eq!(undeclared.code, 0, "{}", undeclared.stderr);
    let counts = report(&undeclared);
    assert_eq!(counts["summary"]["findings"], 0);
    assert_eq!(counts["summary"]["refusals"], 2);
    // Refused for the script checks only: both files were still read.
    assert_eq!(counts["summary"]["unexamined"], 0);
    assert!(
        undeclared.stderr.contains("--script"),
        "the way out is not offered: {}",
        undeclared.stderr
    );

    // And declaring the scripts judges them, still finding nothing.
    let declared = run(&["--script", "Cyrillic,Han", &path]);
    assert_eq!(declared.code, 0, "{}", declared.stderr);
    let counts = report(&declared);
    assert_eq!(counts["summary"]["refusals"], 0);
    assert_eq!(counts["summary"]["findings"], 0);
}

/// **The key path, at the process boundary.** A bidi control in a
/// five-thousand-line locale catalogue is a line number a reader has to
/// go and look up; `metrics.headline.eyebrow` is a place in the
/// document. The format comes from the file's own name — nothing is
/// declared here — and the same bytes under a name the tool does not
/// recognise still report the finding, without a key.
#[test]
fn a_finding_carries_the_key_path_its_format_supplies() {
    let tree = Tree::new("keypath");
    let body = format!(
        "{{\n  \"metrics\": {{\n    \"headline\": {{\n      \"eyebrow\": \"{RLO}\"\n    }}\n  }}\n}}\n"
    );
    tree.write("locales/en.json", &body);
    tree.write("notes.md", &body);

    let run = run(&[&tree.path().to_string_lossy()]);
    assert_eq!(run.code, 1, "{}", run.stderr);
    let report = report(&run);

    let files = report["files"].as_array().expect("a file list");
    let named = |suffix: &str| {
        files
            .iter()
            .find(|file| {
                file["file"]
                    .as_str()
                    .is_some_and(|found| found.replace('\\', "/").ends_with(suffix))
            })
            .unwrap_or_else(|| panic!("{suffix} is not in the report: {}", run.stdout))
    };
    let json = named("locales/en.json");
    let markdown = named("notes.md");

    assert_eq!(
        json["findings"][0]["key"], "metrics.headline.eyebrow",
        "{}",
        run.stdout
    );
    // The same finding either way. A format that could not be read costs
    // a key path and never a finding.
    assert_eq!(markdown["findings"][0]["key"], serde_json::Value::Null);
    assert_eq!(
        markdown["findings"][0]["offset"],
        json["findings"][0]["offset"]
    );
    assert_eq!(report["summary"]["findings"], 2);
}

/// Every format the tool reads, end to end, each named only by its file
/// extension. A reader that stopped naming things would cost no finding
/// — which is the design, and is exactly why it needs asserting here.
#[test]
fn every_format_resolves_a_key_path_from_the_file_name_alone() {
    let tree = Tree::new("formats");
    for (name, body) in [
        ("a.json", format!("{{\"outer\":{{\"inner\":\"{RLO}\"}}}}\n")),
        ("a.yaml", format!("outer:\n  inner: \"{RLO}\"\n")),
        ("a.toml", format!("[outer]\ninner = \"{RLO}\"\n")),
        ("a.ini", format!("[outer]\ninner = {RLO}\n")),
        (".env", format!("INNER={RLO}\n")),
        ("a.csv", format!("inner\n{RLO}\n")),
    ] {
        tree.write(name, &body);
    }

    let run = run(&["--hidden", &tree.path().to_string_lossy()]);
    assert_eq!(run.code, 1, "{}", run.stderr);
    let report = report(&run);
    assert_eq!(report["summary"]["files"], 6, "{}", run.stdout);

    for (name, key) in [
        ("a.json", "outer.inner"),
        ("a.yaml", "outer.inner"),
        ("a.toml", "outer.inner"),
        ("a.ini", "outer.inner"),
        (".env", "INNER"),
        ("a.csv", "inner"),
    ] {
        let file = report["files"]
            .as_array()
            .expect("a file list")
            .iter()
            .find(|file| {
                file["file"]
                    .as_str()
                    .is_some_and(|found| found.replace('\\', "/").ends_with(name))
            })
            .unwrap_or_else(|| panic!("{name} is not in the report: {}", run.stdout));
        assert_eq!(file["findings"][0]["key"], key, "{name}: {}", run.stdout);
    }
}

/// A refusal never fails the run on its own. `--strict` is how a caller
/// insists that the scan covered what it was pointed at.
#[test]
fn a_refused_file_reaches_the_exit_code_only_under_strict() {
    let tree = Tree::new("strict");
    tree.write("src/a.ts", "const ok = 1;\n");
    tree.write_bytes("logo.png", &[0x89, 0x50, 0x4E, 0x47, 0x00, 0x1A]);
    let path = tree.path().to_string_lossy().to_string();

    let lenient = run(&[&path]);
    assert_eq!(lenient.code, 0, "{}", lenient.stderr);
    assert_eq!(report(&lenient)["summary"]["unexamined"], 1);

    assert_eq!(run(&["--strict", &path]).code, 2);
}

/// A UTF-16 file is refused by name rather than decoded as something
/// else. Decoding it as UTF-8 or Latin-1 would report a document full of
/// NUL bytes and unassigned characters that are not in the file.
#[test]
fn a_utf16_file_is_refused_rather_than_mis_decoded() {
    let tree = Tree::new("utf16");
    let mut bytes = vec![0xFF, 0xFE];
    for unit in "const id = 1;\n".encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    tree.write_bytes("config.txt", &bytes);

    let run = run(&[&tree.path().to_string_lossy()]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let report = report(&run);
    assert_eq!(
        report["files"][0]["refusals"][0]["reason"],
        "encoding_unknown"
    );
    assert_eq!(report["summary"]["findings"], 0);
    assert!(run.stderr.contains("UTF-16LE"), "{}", run.stderr);
}

#[test]
fn an_unreadable_input_exits_two() {
    let run = run(&["/no/such/place-xyz"]);
    assert_eq!(run.code, 2);
    assert!(run.stdout.is_empty(), "a refusal writes no report");
}

#[test]
fn an_unknown_flag_exits_two_and_names_itself() {
    let tree = source_tree("badflag");
    let run = run(&["--kinds", "bidi", &tree.path().to_string_lossy()]);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("--kinds"), "{}", run.stderr);
    assert!(run.stdout.is_empty(), "a refusal writes no report");
}

#[test]
fn an_unknown_kind_script_or_threshold_exits_two() {
    let tree = source_tree("badvalue");
    let path = tree.path().to_string_lossy().to_string();
    for args in [
        vec!["--kind", "homoglyph", &path],
        vec!["--script", "Klingon", &path],
        vec!["--fail-on", "everything", &path],
    ] {
        let run = run(&args);
        assert_eq!(run.code, 2, "{args:?}");
        assert!(run.stdout.is_empty(), "{args:?}");
    }
}

/// Nothing writes. If any of these is ever accepted, the line this tool
/// was drawn along has moved.
#[test]
fn no_flag_offers_to_change_a_file() {
    let tree = source_tree("nofix");
    for attempt in ["--fix", "--write", "--normalize", "--nfc", "--strip"] {
        assert_eq!(
            run(&[attempt, &tree.path().to_string_lossy()]).code,
            2,
            "{attempt} was accepted"
        );
    }
}

/// The tool must not have modified what it read, and a scan is the one
/// operation where that is easy to get wrong by "helpfully" normalizing.
#[test]
fn a_scanned_file_is_left_exactly_as_it_was() {
    let tree = Tree::new("readonly");
    let content = format!("const label = \"{RLO}\";\nconst e = \"e\u{301}\";\n");
    let file = tree.write("src/a.ts", &content);
    assert_eq!(run(&[&tree.path().to_string_lossy()]).code, 1);
    assert_eq!(std::fs::read_to_string(&file).expect("readable"), content);
}

#[test]
fn version_and_help_exit_clear() {
    let version = run(&["--version"]);
    assert_eq!(version.code, 0);
    assert!(version.stdout.contains("unicode-le"));
    let help = run(&["--help"]);
    assert_eq!(help.code, 0);
    assert!(help.stdout.contains("usage: unicode-le"));
    assert!(
        help.stdout.contains("never rewrites"),
        "the non-goal is unstated"
    );
}

/// **The report-safety rule reaches the help text too.** `--help` and
/// `--version` write straight to stdout with nothing between, so they are
/// the one output path no escape covers — and the usage text held an em
/// dash. Asserted at the process boundary as well as on the constant,
/// because what matters is the bytes a caller receives.
#[test]
fn help_and_version_write_nothing_but_ascii() {
    for args in [vec!["--help"], vec!["--version"], vec!["-h"], vec!["-V"]] {
        let run = run(&args);
        assert!(run.stdout.is_ascii(), "{args:?}: {}", run.stdout);
        assert!(run.stderr.is_ascii(), "{args:?}: {}", run.stderr);
    }
}

#[test]
fn a_document_on_stdin_is_scanned() {
    let mut child = Command::new(BINARY)
        .args(["--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(format!("const x = \"{RLO}\";\n").as_bytes())
        .expect("written");
    let output = child.wait_with_output().expect("finishes");
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout carries JSON");
    assert_eq!(report["files"][0]["file"], "<stdin>");
    assert_eq!(report["files"][0]["findings"][0]["kind"], "bidi-control");
    assert_eq!(report["files"][0]["findings"][0]["codepoints"][0], "U+202E");
}

#[test]
fn stdin_with_file_arguments_exits_two() {
    let tree = source_tree("stdin-and-files");
    assert_eq!(
        run(&["--stdin", &tree.path().to_string_lossy()]).code,
        2,
        "one input or the other, not both"
    );
}

/// **The cross-surface contract.** Both surfaces call one entry point,
/// so they must answer identically for the same tree.
#[test]
fn the_cli_and_the_mcp_server_report_the_same_thing() {
    let tree = source_tree("agreement");
    let cli = run(&[&tree.path().to_string_lossy()]);
    let from_cli = report(&cli);

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
    let output = child.wait_with_output().expect("finishes");
    let response: serde_json::Value = serde_json::from_slice(
        output
            .stdout
            .split(|byte| *byte == b'\n')
            .next()
            .expect("a line"),
    )
    .expect("the reply is JSON");

    let from_mcp = &response["result"]["structuredContent"]["data"]["report"];
    assert_eq!(from_mcp, &from_cli, "the two surfaces disagree");
}
