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
pub(crate) mod format;
pub(crate) mod normalize;
pub(crate) mod scripts;

mod csv;
mod dotenv;
mod ini;
mod json;
mod locate;
mod position;
mod toml;
mod yaml;

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
    /// The document's own name for where this sits — a dotted key path.
    ///
    /// Present when the format supplies one and **absent otherwise**,
    /// which is the honest shape: a `.md` file has no keys, and an empty
    /// string would read as a key that is empty rather than as no key at
    /// all. A line in a five-thousand-line catalogue is a place in a
    /// file; `metrics.headline.eyebrow` is a place in the document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) key: Option<String>,
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
    /// Positions and key paths are both attached here, in one place, so
    /// no scanner can invent its own idea of a column or of a key.
    fn locate(self, index: &PositionIndex, spans: &[locate::KeySpan]) -> Finding {
        Finding {
            kind: self.kind,
            severity: self.severity,
            position: index.at(self.offset),
            offset: self.offset,
            // An empty path is the document's root, which names nothing
            // — reported as no key rather than as a key that is blank.
            key: locate::key_at(spans, self.offset)
                .filter(|path| !path.is_empty())
                .map(str::to_string),
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
    /// The format to read key paths with, when the caller knows better
    /// than the filename does. `None` resolves from the name, which is
    /// what the CLI always wants; an MCP caller handing over a document
    /// with no filename is the reason this exists.
    pub(crate) format: Option<String>,
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

/// Examine one document.
///
/// `format` decides **only how a finding is addressed**, never which
/// findings exist. Every scanner below runs over the same raw text
/// whatever it is, so a document whose format cannot be read loses its
/// key paths and keeps every one of its findings. See `format`.
pub(crate) fn examine(content: &str, format: &str, options: &Options) -> Examination {
    let mut drafts = characters::scan(content);
    drafts.extend(normalize::scan(content));

    // The script pair is asked for together or not at all: the refusal
    // exists because those two checks cannot be answered honestly here,
    // so a caller who wants neither should not be told about it.
    let mut refusals = Vec::new();
    if options.wants(Kind::Confusable) || options.wants(Kind::MixedScript) {
        match scripts::context(content, &options.expected_scripts) {
            Some(refusal) => refusals.push(refusal),
            None => drafts.extend(scripts::scan(
                content,
                &options.expected_scripts,
                &locate::escape_spans(content, format),
            )),
        }
    }

    drafts.retain(|draft| options.wants(draft.kind));
    // Document order, and a total order within an offset so two runs
    // over an unchanged file produce a byte-identical report.
    drafts.sort_by_key(|draft| (draft.offset, draft.kind));

    let index = PositionIndex::new(content);
    let spans = locate::key_spans(content, format);
    Examination {
        findings: drafts
            .into_iter()
            .map(|draft| draft.locate(&index, &spans))
            .collect(),
        refusals,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(content: &str) -> Vec<Kind> {
        examine(content, "text", &Options::default())
            .findings
            .into_iter()
            .map(|finding| finding.kind)
            .collect()
    }

    #[test]
    fn a_clean_document_yields_nothing() {
        let clean = examine("const total = 1 + 2;\n", "text", &Options::default());
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
        let findings = examine(content, "text", &Options::default()).findings;
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
        let first = examine(content, "text", &Options::default()).findings;
        let second = examine(content, "text", &Options::default()).findings;
        assert_eq!(first, second);
    }

    #[test]
    fn a_kind_filter_narrows_the_report() {
        let content = "let a = \"\u{202E}x\u{00A0}\";\n";
        assert_eq!(kinds(content).len(), 2);
        let only_bidi = examine(
            content,
            "text",
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
        assert_eq!(
            examine(content, "text", &Options::default()).refusals.len(),
            1
        );
        let only_bidi = examine(
            content,
            "text",
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
        let examination = examine(content, "text", &Options::default());
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

    /// **The inversion the whole format layer rests on.** A format
    /// decides how a finding is addressed and never whether it exists,
    /// so the same bytes read as JSON, as YAML, as a format that does
    /// not exist and as nothing at all must yield the same findings in
    /// the same places. Only the key differs.
    #[test]
    fn a_format_never_changes_which_findings_exist() {
        let content = "{\n  \"a\": \"x\u{202E}y\",\n  \"b\": \"p\u{00A0}q\"\n}\n";
        let baseline: Vec<(Kind, usize)> = examine(content, "text", &Options::default())
            .findings
            .into_iter()
            .map(|finding| (finding.kind, finding.offset))
            .collect();
        assert_eq!(baseline.len(), 2, "the document must find something");

        for format in ["json", "yaml", "toml", "ini", "env", "csv", "nonsense"] {
            let found: Vec<(Kind, usize)> = examine(content, format, &Options::default())
                .findings
                .into_iter()
                .map(|finding| (finding.kind, finding.offset))
                .collect();
            assert_eq!(found, baseline, "{format} changed the findings");
        }
    }

    /// And the half that makes it worth having: read as its own format,
    /// a finding is named by the document's own vocabulary rather than
    /// by a line number in a catalogue of five thousand of them.
    #[test]
    fn a_finding_carries_its_key_path_when_the_format_supplies_one() {
        let content = "{\n  \"metrics\": { \"headline\": \"x\u{202E}y\" }\n}\n";
        let keyed = examine(content, "json", &Options::default()).findings;
        assert_eq!(keyed[0].key.as_deref(), Some("metrics.headline"));

        // The same document with no format known: same finding, no key.
        let plain = examine(content, "text", &Options::default()).findings;
        assert_eq!(plain[0].key, None);
        assert_eq!(plain[0].offset, keyed[0].offset);
    }

    /// An empty path is the document's root, which names nothing. It is
    /// reported as no key rather than as a key that is the empty string,
    /// because a consumer branching on presence would take one for the
    /// other.
    #[test]
    fn a_root_scalar_carries_no_key_rather_than_an_empty_one() {
        let findings = examine("\"x\u{202E}y\"", "json", &Options::default()).findings;
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].key, None);
    }

    #[test]
    fn the_position_and_the_offset_are_both_carried() {
        let finding = &examine(
            "let a = 1;\nlet b = \"\u{202E}\";\n",
            "text",
            &Options::default(),
        )
        .findings[0];
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
        let examination = examine(PLANTED, "text", &Options::default());
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
        let rendered =
            serde_json::to_string(&examine(PLANTED, "text", &Options::default()).findings)
                .expect("serializes");
        assert!(rendered.is_ascii(), "{rendered}");
    }

    /// **A key path is text out of the document**, so it is the one
    /// field of a finding that this crate does not author — the same
    /// shape as the file name, and the same hazard. A locale catalogue
    /// can hold a key with a bidi control in it as easily as a value
    /// can, and a report naming that key raw would carry it onward.
    ///
    /// The finding itself is allowed to hold the real characters, so a
    /// consumer can match the key against its own document; what is
    /// asserted is that neither stream ever prints them. `escape` does
    /// that for both, and this pins the input that reaches it.
    #[test]
    fn a_key_path_is_carried_but_never_printed_raw() {
        let document = format!("{{\"a{}b\":\"x{}y\"}}", '\u{202E}', '\u{200B}');
        let findings = examine(&document, "json", &Options::default()).findings;
        // Two: the override hiding in the key's own name, which carries
        // no key path because a key is not a value region, and the
        // zero-width space in the value, which carries one.
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].key, None, "a key named itself");
        let key = findings[1].key.as_deref().expect("a key path");
        assert!(key.contains('\u{202E}'), "the key lost the document's text");

        let rendered = serde_json::to_string(&findings).expect("serializes");
        let escaped = crate::escape::json(&rendered);
        assert!(escaped.is_ascii(), "{escaped}");
        // And it still round-trips, so a consumer can match it.
        let parsed: Vec<Finding> = serde_json::from_str(&escaped).expect("still JSON");
        assert_eq!(parsed, findings);
    }

    /// Refusals are prose this crate wrote, and they quote nothing
    /// either — including the file that provoked them.
    #[test]
    fn a_refusal_is_ascii_and_nothing_else() {
        let examination = examine("你好世界你好世界你好世界", "text", &Options::default());
        assert_eq!(examination.refusals.len(), 1);
        assert!(examination.refusals[0].detail.is_ascii());
    }
}
