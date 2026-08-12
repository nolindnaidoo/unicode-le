//! The corpus, embedded and run against this implementation.
//!
//! `fixtures/documents/` holds one document per finding kind, plus the
//! documents that must produce **nothing** — which are the ones that
//! matter most. A detector that only ever fires is as broken as one that
//! never does, and for this tool the failure mode is specific: a
//! confusable check that fires on ordinary Russian or Chinese text is a
//! check that gets switched off, and the Trojan Source screen goes off
//! with it.

use serde::Deserialize;

use super::{Finding, Kind, Options, Reason, Severity, encoding, examine, scripts::parse_script};

const DETECTION: &str = include_str!("../../fixtures/detection.json");

const TEXT_DOCUMENTS: [(&str, &str); 19] = [
    // One per key-path reader. Each holds a real finding in a real
    // structure, because the claim being pinned is that the reader names
    // where it sits and not merely that the reader runs.
    (
        "messages.json",
        include_str!("../../fixtures/documents/messages.json"),
    ),
    (
        "config.yaml",
        include_str!("../../fixtures/documents/config.yaml"),
    ),
    (
        "config.toml",
        include_str!("../../fixtures/documents/config.toml"),
    ),
    (
        "settings.ini",
        include_str!("../../fixtures/documents/settings.ini"),
    ),
    (
        "secrets.env",
        include_str!("../../fixtures/documents/secrets.env"),
    ),
    (
        "rows.csv",
        include_str!("../../fixtures/documents/rows.csv"),
    ),
    (
        "trojan-source.c",
        include_str!("../../fixtures/documents/trojan-source.c"),
    ),
    (
        "invisible.ts",
        include_str!("../../fixtures/documents/invisible.ts"),
    ),
    (
        "homoglyph.js",
        include_str!("../../fixtures/documents/homoglyph.js"),
    ),
    (
        "fullwidth.ts",
        include_str!("../../fixtures/documents/fullwidth.ts"),
    ),
    (
        "mixed-script.py",
        include_str!("../../fixtures/documents/mixed-script.py"),
    ),
    ("nfd.txt", include_str!("../../fixtures/documents/nfd.txt")),
    (
        "whitespace.md",
        include_str!("../../fixtures/documents/whitespace.md"),
    ),
    (
        "private-use.txt",
        include_str!("../../fixtures/documents/private-use.txt"),
    ),
    ("bom.ts", include_str!("../../fixtures/documents/bom.ts")),
    (
        "clean.ts",
        include_str!("../../fixtures/documents/clean.ts"),
    ),
    (
        "zh-cn.json",
        include_str!("../../fixtures/documents/zh-cn.json"),
    ),
    ("ru.json", include_str!("../../fixtures/documents/ru.json")),
    ("ja.json", include_str!("../../fixtures/documents/ja.json")),
];

/// The documents that are not text, and cannot be. They exist as bytes
/// because that is the whole point of them: a fixture stored as a string
/// would already have been decoded, which is the thing being refused.
const BYTE_DOCUMENTS: [(&str, &[u8]); 3] = [
    (
        "utf16le.txt",
        include_bytes!("../../fixtures/documents/utf16le.txt"),
    ),
    (
        "binary.dat",
        include_bytes!("../../fixtures/documents/binary.dat"),
    ),
    (
        "latin1.txt",
        include_bytes!("../../fixtures/documents/latin1.txt"),
    ),
];

/// The documents written in another writing system, which must produce
/// no confusable and no mixed-script finding whatever else they produce.
const NON_LATIN: [&str; 3] = ["zh-cn.json", "ru.json", "ja.json"];

pub(crate) fn document(name: &str) -> &'static str {
    TEXT_DOCUMENTS
        .iter()
        .find(|(file, _)| *file == name)
        .map_or_else(
            || panic!("the corpus refers to {name}, which is not embedded"),
            |(_, content)| *content,
        )
}

fn document_bytes(name: &str) -> &'static [u8] {
    BYTE_DOCUMENTS
        .iter()
        .find(|(file, _)| *file == name)
        .map_or_else(
            || panic!("the corpus refers to {name}, which is not embedded"),
            |(_, content)| *content,
        )
}

#[derive(Debug, Deserialize)]
struct Corpus {
    documents: Vec<DocumentCase>,
    encodings: Vec<EncodingCase>,
}

#[derive(Debug, Deserialize)]
struct DocumentCase {
    name: String,
    file: String,
    /// Scripts declared for this document, as a caller would declare
    /// them. Absent means none were, which is what makes the refusal
    /// cases refusals.
    #[serde(default)]
    scripts: Vec<String>,
    expected: Vec<Finding>,
    #[serde(default)]
    refusals: Vec<Reason>,
}

#[derive(Debug, Deserialize)]
struct EncodingCase {
    name: String,
    file: String,
    reason: Reason,
}

fn corpus() -> Corpus {
    serde_json::from_str(DETECTION).expect("the corpus is valid JSON")
}

fn options(scripts: &[String]) -> Options {
    Options {
        kinds: Vec::new(),
        expected_scripts: scripts
            .iter()
            .map(|tag| parse_script(tag).expect("a script the corpus names"))
            .collect(),
        format: None,
    }
}

/// The format a fixture is read as, from its own name — the same
/// resolution the CLI does, so the corpus pins what a caller gets rather
/// than a laboratory setting.
fn format_of(file: &str) -> &'static str {
    super::format::resolve_format(None, Some(file))
}

#[test]
fn every_document_case_reproduces() {
    let corpus = corpus();
    assert!(!corpus.documents.is_empty(), "the corpus is empty");

    for case in corpus.documents {
        let examination = examine(
            document(&case.file),
            format_of(&case.file),
            &options(&case.scripts),
        );
        assert_eq!(examination.findings, case.expected, "{}", case.name);
        assert_eq!(
            examination
                .refusals
                .iter()
                .map(|refusal| refusal.reason)
                .collect::<Vec<_>>(),
            case.refusals,
            "{}",
            case.name
        );
    }
}

#[test]
fn every_encoding_case_reproduces() {
    let corpus = corpus();
    assert!(!corpus.encodings.is_empty(), "no encoding is pinned");

    for case in corpus.encodings {
        match encoding::decode(document_bytes(&case.file)) {
            encoding::Decoded::Refused(refusal) => {
                assert_eq!(refusal.reason, case.reason, "{}", case.name);
            }
            encoding::Decoded::Text(_) => panic!("{} decoded instead of refusing", case.name),
        }
    }
}

/// **The false-positive guard, stated directly rather than inferred from
/// an expectation list.** These three documents are ordinary
/// translations. If either script check ever fires on one of them, this
/// tool is unusable on the internationalised repositories that most need
/// it, and the expectation lists above would have been quietly edited to
/// match.
#[test]
fn no_translation_produces_a_confusable_or_mixed_script_finding() {
    for name in NON_LATIN {
        let findings = examine(document(name), format_of(name), &Options::default()).findings;
        let script_findings: Vec<Kind> = findings
            .iter()
            .map(|finding| finding.kind)
            .filter(|kind| matches!(kind, Kind::Confusable | Kind::MixedScript))
            .collect();
        assert!(
            script_findings.is_empty(),
            "{name} produced {script_findings:?}"
        );
    }
}

/// Naming the script lifts the refusal, and the answer is still nothing.
/// Without this the guard above would pass for the wrong reason — a
/// refusal suppresses the checks, so "no findings" could mean "not
/// judged" rather than "judged and clean".
#[test]
fn a_declared_translation_is_judged_and_is_still_clean() {
    for (name, scripts) in [
        ("zh-cn.json", vec!["Han".to_string()]),
        ("ru.json", vec!["Cyrillic".to_string()]),
        (
            "ja.json",
            vec![
                "Han".to_string(),
                "Hiragana".to_string(),
                "Katakana".to_string(),
            ],
        ),
    ] {
        let examination = examine(document(name), format_of(name), &options(&scripts));
        assert!(examination.refusals.is_empty(), "{name} was still refused");
        assert!(
            examination.findings.is_empty(),
            "{name}: {:?}",
            examination.findings
        );
    }
}

/// Every embedded document must be used by a case, and every case must
/// name an embedded document.
#[test]
fn the_corpus_and_the_embedded_documents_match() {
    let corpus = corpus();
    for (name, _) in TEXT_DOCUMENTS {
        assert!(
            corpus.documents.iter().any(|case| case.file == name),
            "{name} is embedded but no case uses it"
        );
    }
    for (name, _) in BYTE_DOCUMENTS {
        assert!(
            corpus.encodings.iter().any(|case| case.file == name),
            "{name} is embedded but no case uses it"
        );
    }
    for case in &corpus.documents {
        assert!(
            TEXT_DOCUMENTS.iter().any(|(name, _)| *name == case.file),
            "{} names {}, which is not embedded",
            case.name,
            case.file
        );
    }
}

/// Every kind must be pinned by at least one document. A kind with no
/// corpus case can stop working without anything noticing.
#[test]
fn every_kind_appears_somewhere_in_the_corpus() {
    let corpus = corpus();
    let found: Vec<Kind> = corpus
        .documents
        .iter()
        .flat_map(|case| case.expected.iter().map(|finding| finding.kind))
        .collect();
    for (kind, name, _) in super::KINDS {
        assert!(found.contains(&kind), "no corpus case pins {name}");
    }
}

// ------------------------------------------------------------ the matrix
//
// **Does this crate open what it claims?**
//
// The extractor crates in this family answer that with one fixture per
// extension in their alias table. This crate has no format table — it
// reads every text file and has no opinion about the name — so the
// question is asked of the other thing it publishes: its vocabulary.
// `kind`, `severity` and `reason` are the three enums a caller filters,
// sorts and branches on, and every value in each of them is a claim that
// this tool can produce it.
//
// A value nothing can reach is the same failure as a format in the table
// with no corpus document: it inflates what the tool says it covers, and
// a consumer writing a `match` over the JSON handles a case that never
// arrives. So each is reached from a **real fixture**, not from a
// hand-built string — the fixture is the thing a reader can open.
//
// The marker lines are not decoration. `cargo test <filter>` exits 0
// when the filter matches nothing, so a renamed test would leave the CI
// job green and checking nothing; the job greps for these instead.

/// Every severity a finding can carry.
///
/// Kept beside the exhaustive `match` in `severity_name` below: adding a
/// variant fails to compile there, which is what stops this list going
/// stale while the enum grows.
const SEVERITIES: [Severity; 3] = [Severity::Low, Severity::Medium, Severity::High];

/// Every reason a file, or part of one, can go unjudged for.
const REASONS: [Reason; 3] = [
    Reason::BinaryOrUndecodable,
    Reason::EncodingUnknown,
    Reason::IntentionalScriptContext,
];

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Low => "low",
        Severity::Medium => "medium",
        Severity::High => "high",
    }
}

fn reason_name(reason: Reason) -> &'static str {
    match reason {
        Reason::BinaryOrUndecodable => "binary_or_undecodable",
        Reason::EncodingUnknown => "encoding_unknown",
        Reason::IntentionalScriptContext => "intentional_script_context",
    }
}

/// Every finding produced by running the corpus, as the code actually
/// produces it — not as `detection.json` says it should. The expectation
/// lists could be edited to match a regression; a fixture that no longer
/// produces its finding cannot be.
fn findings_from_every_fixture() -> Vec<Finding> {
    TEXT_DOCUMENTS
        .iter()
        .flat_map(|(name, content)| examine(content, format_of(name), &Options::default()).findings)
        .collect()
}

/// Every kind is produced by running a fixture through `examine`.
#[test]
fn coverage_matrix_every_kind_is_reachable_from_a_fixture() {
    let findings = findings_from_every_fixture();
    for (kind, name, short) in super::KINDS {
        assert!(
            findings.iter().any(|finding| finding.kind == kind),
            "{name} is offered as a filter (`{short}`) and no fixture produces it"
        );
    }
    eprintln!(
        "coverage-matrix: {} kinds reachable from fixtures",
        super::KINDS.len()
    );
}

/// Every severity is produced by running a fixture, and spells itself
/// the same way in the JSON a caller reads.
#[test]
fn coverage_matrix_every_severity_is_reachable_from_a_fixture() {
    let findings = findings_from_every_fixture();
    for severity in SEVERITIES {
        assert!(
            findings.iter().any(|finding| finding.severity == severity),
            "no fixture produces a {} finding",
            severity_name(severity)
        );
        let rendered = serde_json::to_string(&severity).expect("serializes");
        assert_eq!(rendered, format!("\"{}\"", severity_name(severity)));
    }
    eprintln!(
        "coverage-matrix: {} severities reachable from fixtures",
        SEVERITIES.len()
    );
}

/// Every refusal reason is produced by a fixture, and spells itself the
/// same way in the JSON. A documented reason nothing can reach is a
/// state a consumer handles and never sees.
#[test]
fn coverage_matrix_every_refusal_reason_is_reachable_from_a_fixture() {
    let mut reached: Vec<Reason> = TEXT_DOCUMENTS
        .iter()
        .flat_map(|(name, content)| examine(content, format_of(name), &Options::default()).refusals)
        .map(|refusal| refusal.reason)
        .collect();
    reached.extend(
        BYTE_DOCUMENTS
            .iter()
            .filter_map(|(_, bytes)| match encoding::decode(bytes) {
                encoding::Decoded::Refused(refusal) => Some(refusal.reason),
                encoding::Decoded::Text(_) => None,
            }),
    );

    for reason in REASONS {
        assert!(
            reached.contains(&reason),
            "no fixture produces a {} refusal",
            reason_name(reason)
        );
        let rendered = serde_json::to_string(&reason).expect("serializes");
        assert_eq!(rendered, format!("\"{}\"", reason_name(reason)));
    }
    eprintln!(
        "coverage-matrix: {} refusal reasons reachable from fixtures",
        REASONS.len()
    );
}

/// **Does the crate open what it claims?**
///
/// Every format the tool schema offers must be *productive*: a real
/// fixture is read as that format and comes back with a finding that
/// carries a key path. A reader offered and never exercised inflates
/// what the tool says it covers, and a reader that silently stopped
/// naming anything would otherwise pass every other test in this file —
/// because a lost key path costs no finding, which is the whole point of
/// the design and also exactly what makes it easy to lose quietly.
///
/// `text` is productive in the opposite way, and is asserted the
/// opposite way: it must produce findings and **no** key at all.
#[test]
fn coverage_matrix_every_format_reader_is_reachable_from_a_fixture() {
    let mut keyed: Vec<&str> = Vec::new();
    let mut plain = 0usize;

    for (name, content) in TEXT_DOCUMENTS {
        let format = format_of(name);
        let findings = examine(content, format, &Options::default()).findings;
        if format == super::format::FALLBACK_FORMAT {
            plain += findings.iter().filter(|f| f.key.is_some()).count();
            continue;
        }
        if findings.iter().any(|finding| finding.key.is_some()) {
            keyed.push(format);
        }
    }
    assert_eq!(plain, 0, "the plain-text reader invented a key path");

    for format in super::format::SUPPORTED_FORMATS {
        if format == super::format::FALLBACK_FORMAT {
            // Reachable by construction: everything unrecognised is it,
            // and the corpus is mostly `.ts`, `.c`, `.py` and `.md`.
            continue;
        }
        assert!(
            keyed.contains(&format),
            "no fixture document is read as {format} and produces a keyed finding: {keyed:?}"
        );
    }

    eprintln!(
        "coverage-matrix: {} format readers reachable from fixtures",
        super::format::SUPPORTED_FORMATS.len()
    );
}
