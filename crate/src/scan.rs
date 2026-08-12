//! One file end to end — the only path either surface calls.
//!
//! `cli.rs` and `mcp/` both come through here, so a rule can only be
//! written once. `tests/contracts.rs` asserts the two agree.

use std::path::Path;

use serde::Serialize;

use crate::detect::{self, Finding, Kind, Options, Refusal};

/// The version of the report shape on stdout. Bumped only when a
/// consumer would have to change to keep reading it.
const SCHEMA: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct Counts {
    pub(crate) findings: usize,
    /// The Trojan Source class on its own, because it is the one a
    /// pipeline is most likely to fail on and counting it from the
    /// finding list is the caller's job to get wrong.
    pub(crate) bidi: usize,
    pub(crate) refusals: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct FileReport {
    pub(crate) file: String,
    pub(crate) findings: Vec<Finding>,
    pub(crate) refusals: Vec<Refusal>,
    pub(crate) summary: Counts,
}

impl FileReport {
    /// Whether nothing in this file was examined. A partial refusal —
    /// the script context — does not count: the rest of the file was
    /// read, so "no findings" still means something there.
    pub(crate) fn was_unexamined(&self) -> bool {
        self.refusals.iter().any(|refusal| !refusal.is_partial())
    }
}

/// The whole run, as one document.
///
/// One report rather than one line per file, because the interesting
/// numbers here are totals: how many files were refused, how many
/// findings there are, whether any of them is a bidi control. A consumer
/// reading JSON Lines has to accumulate those itself and every consumer
/// would accumulate them slightly differently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Report {
    pub(crate) schema: u8,
    pub(crate) files: Vec<FileReport>,
    pub(crate) summary: RunCounts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct RunCounts {
    pub(crate) files: usize,
    pub(crate) findings: usize,
    pub(crate) bidi: usize,
    pub(crate) refusals: usize,
    /// Files where **nothing** was read. Separate from `refusals`
    /// because the two mean different things to whoever reads the
    /// report: a file refused for its script context was still screened
    /// for bidi controls and invisibles, and a file that was never
    /// decoded was not screened for anything. Without the split, a
    /// reader has to open the report to tell "clean" from "unread".
    pub(crate) unexamined: usize,
}

/// What makes the run fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum FailOn {
    #[default]
    Any,
    /// Only the Trojan Source class. For a pipeline that wants the
    /// security screen without the cleanliness pass.
    Bidi,
}

pub(crate) fn report(files: Vec<FileReport>) -> Report {
    let summary = RunCounts {
        files: files.len(),
        findings: files.iter().map(|file| file.summary.findings).sum(),
        bidi: files.iter().map(|file| file.summary.bidi).sum(),
        refusals: files.iter().map(|file| file.summary.refusals).sum(),
        unexamined: files
            .iter()
            .filter(|file| FileReport::was_unexamined(file))
            .count(),
    };
    Report {
        schema: SCHEMA,
        files,
        summary,
    }
}

/// The path as the report spells it: **separated by `/` on every
/// platform**.
///
/// A report is diffed against one produced on another machine and read
/// by someone who does not have the tree. A sibling in this family
/// shipped `\` on Windows for a whole release, which made every path in
/// a Windows report differ from the same path in a Linux one for no
/// reason a reader could see — and here it would also make the stderr
/// line unfindable by the grep that works everywhere else.
#[cfg(windows)]
fn report_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// The path as the report spells it. Nothing to rewrite here: `\` is a
/// legal character in a Unix filename, and replacing it would rename the
/// file in the report.
#[cfg(not(windows))]
fn report_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub(crate) fn scan_file(path: &Path, options: &Options) -> FileReport {
    let file = report_path(path);
    match std::fs::read(path) {
        Ok(bytes) => scan_bytes(&bytes, file, options),
        // Reported rather than dropped. A file that vanishes from the
        // report reads to whoever ran it as a file that was clean, and
        // that is the one thing this must never say.
        Err(error) => refused(
            file,
            Refusal {
                reason: detect::Reason::BinaryOrUndecodable,
                detail: format!("could not be read: {error}"),
            },
        ),
    }
}

pub(crate) fn scan_bytes(bytes: &[u8], file: String, options: &Options) -> FileReport {
    match detect::encoding::decode(bytes) {
        detect::encoding::Decoded::Text(content) => scan_content(content, file, options),
        detect::encoding::Decoded::Refused(refusal) => refused(file, refusal),
    }
}

pub(crate) fn scan_content(content: &str, file: String, options: &Options) -> FileReport {
    let examination = detect::examine(content, options);
    let summary = Counts {
        findings: examination.findings.len(),
        bidi: examination
            .findings
            .iter()
            .filter(|finding| finding.kind == Kind::BidiControl)
            .count(),
        refusals: examination.refusals.len(),
    };
    FileReport {
        file,
        findings: examination.findings,
        refusals: examination.refusals,
        summary,
    }
}

fn refused(file: String, refusal: Refusal) -> FileReport {
    FileReport {
        file,
        findings: Vec::new(),
        refusals: vec![refusal],
        summary: Counts {
            findings: 0,
            bidi: 0,
            refusals: 1,
        },
    }
}

/// 0 clean, 1 findings, 2 the question could not be answered.
///
/// `--strict` is the only way a refusal reaches the exit code. Left on
/// by default it would exit 2 on every repository holding a PNG, which
/// is a check nobody can put in CI; left out entirely there would be no
/// way to insist that a scan covered what it was pointed at.
pub(crate) fn exit_code(report: &Report, fail_on: FailOn, strict: bool) -> u8 {
    if strict && report.summary.refusals > 0 {
        return 2;
    }
    let counted = match fail_on {
        FailOn::Any => report.summary.findings,
        FailOn::Bidi => report.summary.bidi,
    };
    u8::from(counted > 0)
}

/// One finding as a human line. Every field is ASCII by construction —
/// see `detect::codepoint` — so this cannot render anything either.
pub(crate) fn describe(report: &FileReport, finding: &Finding) -> String {
    format!(
        "{}:{}:{}  [{}] {} {}  {}",
        report.file,
        finding.position.line,
        finding.position.column,
        severity_name(finding.severity),
        finding.kind.name(),
        finding.codepoints.join(" "),
        finding.detail
    )
}

pub(crate) fn severity_name(severity: detect::Severity) -> &'static str {
    match severity {
        detect::Severity::Low => "low",
        detect::Severity::Medium => "medium",
        detect::Severity::High => "high",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TempTree;

    fn one(content: &str) -> FileReport {
        scan_content(content, "a.ts".into(), &Options::default())
    }

    #[test]
    fn a_clean_file_exits_zero() {
        let report = report(vec![one("const total = 1;\n")]);
        assert_eq!(report.summary.findings, 0);
        assert_eq!(exit_code(&report, FailOn::Any, false), 0);
        assert_eq!(exit_code(&report, FailOn::Any, true), 0);
    }

    #[test]
    fn the_schema_is_carried_and_pinned() {
        let rendered = serde_json::to_value(report(vec![one("x")])).expect("serializes");
        assert_eq!(rendered["schema"], 1);
    }

    /// The report is a claim about a scan, not about when it ran. A
    /// timestamp would make two identical runs produce different bytes,
    /// which is most of what a report in CI is for.
    #[test]
    fn the_report_carries_no_timestamp() {
        let rendered = serde_json::to_string(&report(vec![one("x")])).expect("serializes");
        for field in ["time", "date", "generated", "at\":"] {
            assert!(!rendered.contains(field), "{rendered}");
        }
    }

    #[test]
    fn a_finding_exits_one() {
        let report = report(vec![one("let a = \"\u{202E}\";\n")]);
        assert_eq!(report.summary.findings, 1);
        assert_eq!(report.summary.bidi, 1);
        assert_eq!(exit_code(&report, FailOn::Any, false), 1);
    }

    /// `--fail-on bidi` is how a pipeline takes the security screen
    /// without the cleanliness pass.
    #[test]
    fn failing_on_bidi_ignores_the_cleanliness_findings() {
        let report = report(vec![one("let a = \"x\u{00A0}y\";\n")]);
        assert_eq!(report.summary.findings, 1);
        assert_eq!(report.summary.bidi, 0);
        assert_eq!(exit_code(&report, FailOn::Any, false), 1);
        assert_eq!(exit_code(&report, FailOn::Bidi, false), 0);
    }

    #[test]
    fn a_binary_file_is_refused_rather_than_read() {
        let tree = TempTree::new("scan-binary");
        let file = tree.path().join("logo.png");
        std::fs::write(&file, [0x89, 0x50, 0x4E, 0x47, 0x00, 0x1A]).expect("a file");
        let scanned = scan_file(&file, &Options::default());
        assert!(scanned.was_unexamined());
        assert_eq!(
            scanned.refusals[0].reason,
            detect::Reason::BinaryOrUndecodable
        );
        assert_eq!(exit_code(&report(vec![scanned]), FailOn::Any, false), 0);
    }

    /// A refusal never fails the run on its own, and always fails it
    /// under `--strict`. Both halves matter: the first keeps the tool
    /// runnable, the second is how a caller insists on coverage.
    #[test]
    fn a_refusal_reaches_the_exit_code_only_under_strict() {
        let tree = TempTree::new("scan-strict");
        let file = tree.path().join("notes.txt");
        std::fs::write(&file, [b'c', b'a', b'f', 0xE9]).expect("a file");
        let scanned = report(vec![scan_file(&file, &Options::default())]);
        assert_eq!(exit_code(&scanned, FailOn::Any, false), 0);
        assert_eq!(exit_code(&scanned, FailOn::Any, true), 2);
    }

    /// A report is diffed across machines. A path that changes its
    /// separator with the operating system makes every line of a Windows
    /// report differ from the same line of a Linux one, and makes the
    /// stderr line unfindable by a grep that works everywhere else.
    #[test]
    fn a_reported_path_is_separated_by_forward_slashes_everywhere() {
        let tree = TempTree::new("scan-separator");
        let file = tree.write("src/nested/a.ts", "const ok = 1;\n");
        let reported = scan_file(&file, &Options::default()).file;
        assert!(
            reported.ends_with("src/nested/a.ts"),
            "the report spells the path {reported}"
        );
        assert!(!reported.contains('\\'), "{reported}");
    }

    #[test]
    fn a_missing_file_is_named_rather_than_dropped() {
        let tree = TempTree::new("scan-missing");
        let scanned = scan_file(&tree.path().join("gone.ts"), &Options::default());
        assert!(scanned.was_unexamined());
        assert!(scanned.refusals[0].detail.contains("could not be read"));
    }

    /// A file refused for its script context was still read for
    /// everything else, so it is not an unexamined file.
    #[test]
    fn a_script_context_refusal_is_not_an_unexamined_file() {
        let scanned = one("ключ: значение\nдругой: текст");
        assert!(!scanned.was_unexamined());
        assert_eq!(scanned.summary.refusals, 1);
        assert_eq!(scanned.summary.findings, 0);
    }

    #[test]
    fn the_run_summary_adds_up_the_files() {
        let report = report(vec![
            one("let a = \"\u{202E}\";\n"),
            one("let b = \"x\u{00A0}y\";\n"),
            one("clean\n"),
        ]);
        assert_eq!(report.summary.files, 3);
        assert_eq!(report.summary.findings, 2);
        assert_eq!(report.summary.bidi, 1);
    }

    #[test]
    fn the_human_line_carries_the_codepoint_and_the_verdict() {
        let scanned = one("let a = \"\u{202E}\";\n");
        let line = describe(&scanned, &scanned.findings[0]);
        assert!(line.contains("a.ts:1:10"), "{line}");
        assert!(line.contains("[high] bidi-control U+202E"), "{line}");
        assert!(line.is_ascii(), "{line}");
    }

    /// The human half is a projection of the report, and the report's
    /// safety rule has to survive the projection.
    #[test]
    fn the_human_line_carries_no_character_it_describes() {
        let content = "p\u{430}ypal \u{202E} x\u{200B}y";
        let scanned = one(content);
        for finding in &scanned.findings {
            let line = describe(&scanned, finding);
            assert!(line.is_ascii(), "{line}");
        }
    }
}
