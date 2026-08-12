//! The tier that needs a document far larger than any editor opens.
//!
//! Gated behind `UNICODE_LE_SCENARIOS` and run by CI. Nothing here
//! substitutes for the unit tests, which run everywhere on every push.
//!
//! **A skipped scenario is never reported as a pass.**

use std::io::Write as _;

fn enabled(name: &str) -> bool {
    if std::env::var_os("UNICODE_LE_SCENARIOS").is_some() {
        return true;
    }
    eprintln!("SKIPPED {name}: set UNICODE_LE_SCENARIOS to run it");
    false
}

fn scan(content: &str) -> serde_json::Value {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_unicode-le"))
        .args(["--stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .as_mut()
                .expect("stdin")
                .write_all(content.as_bytes())?;
            child.wait_with_output()
        })
        .expect("the binary runs");
    serde_json::from_slice(&output.stdout).expect("stdout carries JSON")
}

fn findings(report: &serde_json::Value) -> u64 {
    report["summary"]["findings"].as_u64().expect("a count")
}

/// A minified bundle is one very long line. The position index binary
/// searches for the line and then counts UTF-16 units along it, so a
/// single line holding the whole document is the shape where that count
/// is longest — and doing it per finding rather than per document is the
/// difference between linear and quadratic.
#[test]
fn a_single_long_line_completes() {
    if !enabled("a_single_long_line_completes") {
        return;
    }
    let content = "const a = 1; const b = 2; ".repeat(40_000);
    assert_eq!(findings(&scan(&content)), 0);
}

/// The other end: a document that is almost entirely findings. Every
/// one of them allocates its codepoint strings and its detail, so this
/// is where a per-finding cost shows up.
#[test]
fn a_document_that_is_almost_all_findings_completes() {
    if !enabled("a_document_that_is_almost_all_findings_completes") {
        return;
    }
    let content = "a\u{200B}b\u{202E}c\u{00A0}\n".repeat(20_000);
    assert_eq!(findings(&scan(&content)), 60_000);
}

/// The script checks are the expensive half: a resolved script set per
/// word, and a UTS #39 skeleton per candidate character. A large
/// translated document is the realistic worst case, and it must also
/// still refuse rather than judge.
#[test]
fn a_large_translated_document_completes_and_is_refused() {
    if !enabled("a_large_translated_document_completes_and_is_refused") {
        return;
    }
    let content = "\"ключ\": \"значение которое надо проверить\",\n".repeat(20_000);
    let report = scan(&content);
    assert_eq!(findings(&report), 0);
    assert_eq!(
        report["files"][0]["refusals"][0]["reason"],
        "intentional_script_context"
    );
}

/// One word the length of a document. Word segmentation collects a
/// slice per word, and the script set is resolved over the whole slice,
/// so this is the shape where a single word carries the entire cost.
#[test]
fn one_enormous_word_completes() {
    if !enabled("one_enormous_word_completes") {
        return;
    }
    let content = "a".repeat(2_000_000);
    assert_eq!(findings(&scan(&content)), 0);
}
