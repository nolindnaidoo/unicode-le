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

use super::{Finding, Kind, Options, Reason, encoding, examine, scripts::parse_script};

const DETECTION: &str = include_str!("../../fixtures/detection.json");

const TEXT_DOCUMENTS: [(&str, &str); 13] = [
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
    }
}

#[test]
fn every_document_case_reproduces() {
    let corpus = corpus();
    assert!(!corpus.documents.is_empty(), "the corpus is empty");

    for case in corpus.documents {
        let examination = examine(document(&case.file), &options(&case.scripts));
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
        let findings = examine(document(name), &Options::default()).findings;
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
        let examination = examine(document(name), &options(&scripts));
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
