//! The pure detection layer: document text in, located findings out.
//!
//! **Nothing in here touches the filesystem.** A `std::fs` call below
//! this line is a bug, and CI greps for one. Everything the tool decides
//! — including the two rules its usefulness rests on, the report-safety
//! rule in `codepoint` and the script-context refusal in `scripts` — is
//! therefore testable from a string with no disk and no flake.
//!
//! A `Finding` never carries the text it found. See `codepoint`.

pub(crate) mod characters;
pub(crate) mod codepoint;
pub(crate) mod encoding;
pub(crate) mod normalize;
pub(crate) mod scripts;

mod position;

#[cfg(test)]
pub(crate) mod corpus;

pub(crate) use position::Position;

use serde::{Deserialize, Serialize};
use unicode_script::Script;

use position::PositionIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Kind {
    /// The Trojan Source class, CVE-2021-42574.
    BidiControl,
    Invisible,
    Confusable,
    MixedScript,
    NonNfc,
    UnusualWhitespace,
    UnassignedOrPrivateUse,
}

/// Every kind, its name in the report, and the short word a caller may
/// filter on. One table, consulted by the parser, the usage text and the
/// tests — so a kind cannot exist that no filter can name, and a filter
/// cannot name a kind that does not exist.
pub(crate) const KINDS: [(Kind, &str, &str); 7] = [
    (Kind::BidiControl, "bidi-control", "bidi"),
    (Kind::Invisible, "invisible", "invisible"),
    (Kind::Confusable, "confusable", "confusable"),
    (Kind::MixedScript, "mixed-script", "mixed-script"),
    (Kind::NonNfc, "non-nfc", "non-nfc"),
    (Kind::UnusualWhitespace, "unusual-whitespace", "whitespace"),
    (
        Kind::UnassignedOrPrivateUse,
        "unassigned-or-private-use",
        "unassigned",
    ),
];

impl Kind {
    pub(crate) fn name(self) -> &'static str {
        KINDS
            .iter()
            .find(|(kind, _, _)| *kind == self)
            .map_or("", |(_, name, _)| *name)
    }
}

/// Resolve a kind from its report name or its short filter word.
pub(crate) fn parse_kind(token: &str) -> Result<Kind, String> {
    KINDS
        .iter()
        .find(|(_, name, short)| *name == token || *short == token)
        .map(|(kind, _, _)| *kind)
        .ok_or_else(|| {
            let offered: Vec<&str> = KINDS.iter().map(|(_, _, short)| *short).collect();
            format!("{token} is not a kind; one of: {}", offered.join(", "))
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Severity {
    Low,
    Medium,
    High,
}

/// Why a file, or part of one, was not judged.
///
/// A refusal is a first-class result rather than an error: the run
/// carries on, the reader is told what was not covered, and nothing is
/// reported as clean that was never looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Reason {
    /// Not text, or not UTF-8. No encoding is guessed.
    BinaryOrUndecodable,
    /// A byte-order mark for an encoding this does not read.
    EncodingUnknown,
    /// The file is written in another script and none was declared, so
    /// the confusable and mixed-script checks did not run on it.
    IntentionalScriptContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Refusal {
    pub(crate) reason: Reason,
    pub(crate) detail: String,
}

impl Refusal {
    /// Whether the rest of the file was still examined. Only the script
    /// context refuses *part* of a file; the other two mean nothing in
    /// it was read at all, and the difference decides whether a report
    /// with no findings means anything.
    pub(crate) fn is_partial(&self) -> bool {
        self.reason == Reason::IntentionalScriptContext
    }
}

/// One located finding.
///
/// `codepoints` and `resembles` are `U+XXXX` strings and are the **only**
/// representation of the characters involved. There is no field holding
/// the source text, and adding one is the single change that would turn
/// this report into a way of carrying the attack to whoever reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Finding {
    pub(crate) kind: Kind,
    pub(crate) severity: Severity,
    #[serde(flatten)]
    pub(crate) position: Position,
    pub(crate) offset: usize,
    pub(crate) codepoints: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) scripts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) resembles: Vec<String>,
    pub(crate) detail: String,
}

/// A finding before it knows where it is. Every scanner below returns
/// these; positions are attached in one place, so a module cannot invent
/// its own idea of a column.
pub(crate) struct Draft {
    pub(crate) offset: usize,
    pub(crate) kind: Kind,
    pub(crate) severity: Severity,
    pub(crate) codepoints: Vec<String>,
    pub(crate) scripts: Vec<String>,
    pub(crate) resembles: Vec<String>,
    pub(crate) detail: String,
}

impl std::fmt::Debug for Draft {
    /// Written out rather than derived so a `Draft` cannot be printed
    /// with the character it describes: the fields are already `U+XXXX`,
    /// and this keeps a future field from quietly changing that.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Draft")
            .field("offset", &self.offset)
            .field("kind", &self.kind)
            .field("severity", &self.severity)
            .field("codepoints", &self.codepoints)
            .field("scripts", &self.scripts)
            .field("resembles", &self.resembles)
            .finish_non_exhaustive()
    }
}

impl Draft {
    fn locate(self, index: &PositionIndex) -> Finding {
        Finding {
            kind: self.kind,
            severity: self.severity,
            position: index.at(self.offset),
            offset: self.offset,
            codepoints: self.codepoints,
            scripts: self.scripts,
            resembles: self.resembles,
            detail: self.detail,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Options {
    /// Which kinds to report. Empty means every kind — the default, and
    /// the only setting under which a clean report means "clean".
    pub(crate) kinds: Vec<Kind>,
    /// The non-Latin scripts this tree is expected to contain.
    pub(crate) expected_scripts: Vec<Script>,
}

impl Options {
    fn wants(&self, kind: Kind) -> bool {
        self.kinds.is_empty() || self.kinds.contains(&kind)
    }
}

#[derive(Debug, Default)]
pub(crate) struct Examination {
    pub(crate) findings: Vec<Finding>,
    pub(crate) refusals: Vec<Refusal>,
}

pub(crate) fn examine(content: &str, options: &Options) -> Examination {
    let mut drafts = characters::scan(content);
    drafts.extend(normalize::scan(content));

    // The script pair is asked for together or not at all: the refusal
    // exists because those two checks cannot be answered honestly here,
    // so a caller who wants neither should not be told about it.
    let mut refusals = Vec::new();
    if options.wants(Kind::Confusable) || options.wants(Kind::MixedScript) {
        match scripts::context(content, &options.expected_scripts) {
            Some(refusal) => refusals.push(refusal),
            None => drafts.extend(scripts::scan(content, &options.expected_scripts)),
        }
    }

    drafts.retain(|draft| options.wants(draft.kind));
    // Document order, and a total order within an offset so two runs
    // over an unchanged file produce a byte-identical report.
    drafts.sort_by_key(|draft| (draft.offset, draft.kind));

    let index = PositionIndex::new(content);
    Examination {
        findings: drafts
            .into_iter()
            .map(|draft| draft.locate(&index))
            .collect(),
        refusals,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(content: &str) -> Vec<Kind> {
        examine(content, &Options::default())
            .findings
            .into_iter()
            .map(|finding| finding.kind)
            .collect()
    }

    #[test]
    fn a_clean_document_yields_nothing() {
        let clean = examine("const total = 1 + 2;\n", &Options::default());
        assert!(clean.findings.is_empty());
        assert!(clean.refusals.is_empty());
    }

    #[test]
    fn every_kind_is_reachable_from_one_document() {
        let content = concat!(
            "let a = \"\u{202E}\";\n",    // bidi-control
            "let b = \"x\u{200B}y\";\n",  // invisible
            "let p\u{430}ypal = 1;\n",    // confusable + mixed-script
            "let c = \"cafe\u{301}\";\n", // non-nfc
            "let d = \"x\u{00A0}y\";\n",  // unusual-whitespace
            "let e = \"\u{E000}\";\n",    // unassigned-or-private-use
        );
        let mut found = kinds(content);
        found.sort_unstable();
        found.dedup();
        let mut every: Vec<Kind> = KINDS.iter().map(|(kind, _, _)| *kind).collect();
        every.sort_unstable();
        assert_eq!(found, every);
    }

    #[test]
    fn findings_come_back_in_document_order() {
        let content = "a\u{00A0}b\u{202E}c\u{200B}d";
        let findings = examine(content, &Options::default()).findings;
        assert!(
            findings
                .windows(2)
                .all(|pair| pair[0].offset <= pair[1].offset)
        );
        assert_eq!(
            findings.iter().map(|f| f.kind).collect::<Vec<_>>(),
            [Kind::UnusualWhitespace, Kind::BidiControl, Kind::Invisible]
        );
    }

    #[test]
    fn the_same_document_scans_identically_twice() {
        let content = "p\u{430}ypal \u{202E} cafe\u{301}";
        let first = examine(content, &Options::default()).findings;
        let second = examine(content, &Options::default()).findings;
        assert_eq!(first, second);
    }

    #[test]
    fn a_kind_filter_narrows_the_report() {
        let content = "let a = \"\u{202E}x\u{00A0}\";\n";
        assert_eq!(kinds(content).len(), 2);
        let only_bidi = examine(
            content,
            &Options {
                kinds: vec![Kind::BidiControl],
                ..Options::default()
            },
        );
        assert_eq!(only_bidi.findings.len(), 1);
        assert_eq!(only_bidi.findings[0].kind, Kind::BidiControl);
    }

    /// A caller who asked only for bidi controls has no use for a
    /// refusal about a check they did not ask to run.
    #[test]
    fn a_filter_that_excludes_the_script_checks_excludes_their_refusal() {
        let content = "ключ: значение\nдругой: текст";
        assert_eq!(examine(content, &Options::default()).refusals.len(), 1);
        let only_bidi = examine(
            content,
            &Options {
                kinds: vec![Kind::BidiControl],
                ..Options::default()
            },
        );
        assert!(only_bidi.refusals.is_empty());
    }

    /// A refused file is still examined for everything else, and that
    /// has to be true rather than merely claimed in the message.
    #[test]
    fn a_refused_file_is_still_scanned_for_the_other_kinds() {
        let content = "ключ: \u{202E}значение\nдругой: текст";
        let examination = examine(content, &Options::default());
        assert_eq!(examination.refusals.len(), 1);
        assert!(examination.refusals[0].is_partial());
        assert_eq!(
            examination
                .findings
                .iter()
                .map(|f| f.kind)
                .collect::<Vec<_>>(),
            [Kind::BidiControl]
        );
    }

    #[test]
    fn the_position_and_the_offset_are_both_carried() {
        let finding =
            &examine("let a = 1;\nlet b = \"\u{202E}\";\n", &Options::default()).findings[0];
        assert_eq!(finding.position.line, 2);
        assert_eq!(finding.position.column, 10);
        assert_eq!(finding.offset, 20);
    }

    /// The enum, the table and the JSON must all spell a kind the same
    /// way, or a caller filtering on the reported name gets nothing.
    #[test]
    fn every_kind_is_spelled_the_same_everywhere() {
        for (kind, name, short) in KINDS {
            let rendered = serde_json::to_string(&kind).expect("serializes");
            assert_eq!(rendered, format!("\"{name}\""));
            assert_eq!(kind.name(), name);
            assert_eq!(parse_kind(name).expect(name), kind);
            assert_eq!(parse_kind(short).expect(short), kind);
        }
    }

    #[test]
    fn an_unknown_kind_is_refused_by_name() {
        let error = parse_kind("homoglyph").expect_err("a refusal");
        assert!(error.contains("homoglyph"), "{error}");
        assert!(error.contains("confusable"), "{error}");
    }
}

/// The one property this tool cannot get wrong.
#[cfg(test)]
mod hazards {
    use super::*;

    /// Every character that must never appear in a report, and the
    /// document that plants one of each.
    const PLANTED: &str = concat!(
        "\u{202A}\u{202B}\u{202C}\u{202D}\u{202E}",
        "\u{2066}\u{2067}\u{2068}\u{2069}\u{061C}",
        "a\u{200B}b\u{200C}c\u{200D}d\u{2060}e\u{00AD}f\u{FEFF}g\u{180E}h",
        "p\u{430}ypal l\u{3bf}gin \u{FF26}\u{FF29} \u{1d41a}dmin",
        "cafe\u{301} x\u{00A0}y z\u{3000}w \u{E000}\u{0378}",
    );

    /// **The rule the tool exists under.** A report that pasted a raw
    /// bidi control would reorder the terminal, the diff and the ticket
    /// of whoever read it — the tool would be the delivery mechanism for
    /// the thing it detects.
    #[test]
    fn no_report_carries_a_character_it_found() {
        let examination = examine(PLANTED, &Options::default());
        assert!(
            !examination.findings.is_empty(),
            "the hazard document found nothing, so this proves nothing"
        );
        let rendered = serde_json::to_string(&examination.findings).expect("serializes");
        for character in PLANTED.chars().filter(|c| !c.is_ascii()) {
            assert!(
                !rendered.contains(character),
                "{} reached the report",
                codepoint::render(character)
            );
        }
    }

    /// Stronger and simpler than the character-by-character check, and
    /// the property worth keeping: the whole report is ASCII, so there
    /// is nothing in it that could render as anything.
    #[test]
    fn a_report_is_ascii_and_nothing_else() {
        let rendered = serde_json::to_string(&examine(PLANTED, &Options::default()).findings)
            .expect("serializes");
        assert!(rendered.is_ascii(), "{rendered}");
    }

    /// Refusals are prose this crate wrote, and they quote nothing
    /// either — including the file that provoked them.
    #[test]
    fn a_refusal_is_ascii_and_nothing_else() {
        let examination = examine("你好世界你好世界你好世界", &Options::default());
        assert_eq!(examination.refusals.len(), 1);
        assert!(examination.refusals[0].detail.is_ascii());
    }
}
