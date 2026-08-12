//! The four findings that a single character decides on its own.
//!
//! Bidirectional controls, invisibles, spaces that are not the space,
//! and codepoints with no assigned meaning. None of these needs a word,
//! a script or a neighbour to judge, so none of them is gated by the
//! script context — a right-to-left override is the same override in a
//! Chinese file as in an English one.
//!
//! The tables are written out by hand rather than derived from a
//! property, and that is deliberate. `Cf` (format) would sweep in the
//! interlinear annotation marks and the variation selectors; `Zs`
//! (space separator) would miss U+200B, which is not a space separator
//! at all. Each character below is here because it is *known* to do
//! something a reader cannot see, and the list is short enough to read.

use unicode_script::{Script, UNICODE_VERSION, UnicodeScript};

use super::{Draft, Kind, Severity, codepoint};

/// The Trojan Source class — CVE-2021-42574. These reorder how the rest
/// of the line renders, so source that a reviewer reads one way compiles
/// another way.
const BIDI_CONTROLS: [(char, &str); 10] = [
    ('\u{061C}', "arabic letter mark"),
    ('\u{202A}', "left-to-right embedding"),
    ('\u{202B}', "right-to-left embedding"),
    ('\u{202C}', "pop directional formatting"),
    ('\u{202D}', "left-to-right override"),
    ('\u{202E}', "right-to-left override"),
    ('\u{2066}', "left-to-right isolate"),
    ('\u{2067}', "right-to-left isolate"),
    ('\u{2068}', "first strong isolate"),
    ('\u{2069}', "pop directional isolate"),
];

/// Characters that occupy a position and render as nothing.
const INVISIBLES: [(char, &str); 7] = [
    ('\u{00AD}', "soft hyphen"),
    ('\u{180E}', "mongolian vowel separator"),
    ('\u{200B}', "zero width space"),
    ('\u{200C}', "zero width non-joiner"),
    ('\u{200D}', "zero width joiner"),
    ('\u{2060}', "word joiner"),
    ('\u{FEFF}', "zero width no-break space"),
];

/// Spaces that are not U+0020. Every one of them survives a trim, fails
/// an equality test and splits nothing.
const UNUSUAL_SPACES: [(char, &str); 16] = [
    ('\u{00A0}', "no-break space"),
    ('\u{1680}', "ogham space mark"),
    ('\u{2000}', "en quad"),
    ('\u{2001}', "em quad"),
    ('\u{2002}', "en space"),
    ('\u{2003}', "em space"),
    ('\u{2004}', "three-per-em space"),
    ('\u{2005}', "four-per-em space"),
    ('\u{2006}', "six-per-em space"),
    ('\u{2007}', "figure space"),
    ('\u{2008}', "punctuation space"),
    ('\u{2009}', "thin space"),
    ('\u{200A}', "hair space"),
    ('\u{202F}', "narrow no-break space"),
    ('\u{205F}', "medium mathematical space"),
    ('\u{3000}', "ideographic space"),
];

/// The private-use areas. Fixed by the standard and never extended, so
/// unlike everything else about Unicode these three ranges are safe to
/// write down.
const PRIVATE_USE: [(char, char); 3] = [
    ('\u{E000}', '\u{F8FF}'),
    ('\u{F0000}', '\u{FFFFD}'),
    ('\u{100000}', '\u{10FFFD}'),
];

pub(crate) fn scan(content: &str) -> Vec<Draft> {
    content
        .char_indices()
        .filter_map(|(offset, character)| draft(offset, character))
        .collect()
}

fn draft(offset: usize, character: char) -> Option<Draft> {
    // A byte-order mark at the very front is the file announcing its
    // encoding, which is the one job U+FEFF still officially has. The
    // *same* character anywhere else is a zero-width no-break space
    // sitting in the middle of a string, and that is a finding.
    if offset == 0 && character == '\u{FEFF}' {
        return None;
    }
    let (kind, severity, detail) = classify(character)?;
    Some(Draft {
        offset,
        kind,
        severity,
        codepoints: vec![codepoint::render(character)],
        scripts: vec![character.script().full_name().to_string()],
        resembles: Vec::new(),
        detail,
    })
}

fn classify(character: char) -> Option<(Kind, Severity, String)> {
    // ASCII carries none of these and is most of every file, so the
    // cheap test comes first.
    if character.is_ascii() {
        return None;
    }
    if let Some(name) = named(&BIDI_CONTROLS, character) {
        return Some((
            Kind::BidiControl,
            Severity::High,
            format!(
                "{name}: a bidirectional control reorders how the rest of the line renders, so \
                 the text a reviewer reads is not the text that runs"
            ),
        ));
    }
    if let Some(name) = named(&INVISIBLES, character) {
        return Some((
            Kind::Invisible,
            Severity::Medium,
            format!("{name}: takes up no width, so two strings that are not equal look identical"),
        ));
    }
    if let Some(name) = named(&UNUSUAL_SPACES, character) {
        return Some((
            Kind::UnusualWhitespace,
            Severity::Low,
            format!(
                "{name}: renders like a space and is not one, so a trim, a split or a comparison \
                 against U+0020 does not see it"
            ),
        ));
    }
    unassigned_or_private_use(character)
}

/// Unassigned, private-use, noncharacter and surrogate all resolve to
/// `Script::Unknown` in UAX #24 — which is exactly this finding, and is
/// why it is asked of the script table rather than of a range list this
/// crate would have to update every September.
fn unassigned_or_private_use(character: char) -> Option<(Kind, Severity, String)> {
    if character.script() != Script::Unknown {
        return None;
    }
    let detail = if in_private_use(character) {
        "a private-use codepoint: its meaning is whatever the system that produced it decided, \
         and no other system agrees"
            .to_string()
    } else {
        let (major, minor, patch) = UNICODE_VERSION;
        format!(
            "not assigned in Unicode {major}.{minor}.{patch}: no renderer, no collation and no \
             agreed meaning"
        )
    };
    Some((Kind::UnassignedOrPrivateUse, Severity::Medium, detail))
}

fn in_private_use(character: char) -> bool {
    PRIVATE_USE
        .iter()
        .any(|(first, last)| (*first..=*last).contains(&character))
}

fn named(table: &[(char, &'static str)], character: char) -> Option<&'static str> {
    table
        .iter()
        .find(|(candidate, _)| *candidate == character)
        .map(|(_, name)| *name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(content: &str) -> Vec<Kind> {
        scan(content).into_iter().map(|draft| draft.kind).collect()
    }

    #[test]
    fn ordinary_text_yields_nothing() {
        assert!(kinds("let total = 1 + 2; // fine\n").is_empty());
        assert!(kinds("").is_empty());
        // Accented Latin, Cyrillic and Han are ordinary characters here:
        // nothing in this module has an opinion about a script.
        assert!(kinds("café Привет 日本語").is_empty());
    }

    #[test]
    fn every_bidi_control_is_found_and_is_high() {
        for (character, _) in BIDI_CONTROLS {
            let found = scan(&format!("a{character}b"));
            assert_eq!(found.len(), 1, "{}", codepoint::render(character));
            assert_eq!(found[0].kind, Kind::BidiControl);
            assert_eq!(found[0].severity, Severity::High);
            assert_eq!(found[0].offset, 1);
        }
    }

    #[test]
    fn every_invisible_is_found_and_is_medium() {
        for (character, _) in INVISIBLES {
            let found = scan(&format!("a{character}b"));
            assert_eq!(found.len(), 1, "{}", codepoint::render(character));
            assert_eq!(found[0].kind, Kind::Invisible);
            assert_eq!(found[0].severity, Severity::Medium);
        }
    }

    #[test]
    fn every_unusual_space_is_found_and_is_low() {
        for (character, _) in UNUSUAL_SPACES {
            let found = scan(&format!("a{character}b"));
            assert_eq!(found.len(), 1, "{}", codepoint::render(character));
            assert_eq!(found[0].kind, Kind::UnusualWhitespace);
            assert_eq!(found[0].severity, Severity::Low);
        }
    }

    /// The one position where U+FEFF means something other than a
    /// zero-width no-break space sitting inside a string.
    #[test]
    fn a_leading_byte_order_mark_is_the_encoding_and_not_a_finding() {
        assert!(kinds("\u{FEFF}let a = 1;").is_empty());
        assert_eq!(kinds("let a = \u{FEFF}1;"), [Kind::Invisible]);
    }

    /// The claim `unassigned_or_private_use` rests on: UAX #24 gives
    /// `Unknown` to exactly the codepoints with no assigned script, so
    /// this crate never has to carry a range table of its own.
    #[test]
    fn the_script_table_is_what_knows_a_codepoint_is_unassigned() {
        // U+0378 sits in the Greek block and has never been assigned.
        assert_eq!('\u{0378}'.script(), Script::Unknown);
        assert_eq!('\u{E000}'.script(), Script::Unknown);
        assert_eq!('a'.script(), Script::Latin);
        assert_eq!('\u{202E}'.script(), Script::Common);
    }

    #[test]
    fn private_use_and_unassigned_are_one_kind_and_two_reasons() {
        let private = scan("\u{E000}");
        assert_eq!(private[0].kind, Kind::UnassignedOrPrivateUse);
        assert!(private[0].detail.contains("private-use"));

        let unassigned = scan("\u{0378}");
        assert_eq!(unassigned[0].kind, Kind::UnassignedOrPrivateUse);
        assert!(unassigned[0].detail.contains("not assigned"));
    }

    #[test]
    fn all_three_private_use_areas_are_covered() {
        for character in ['\u{E000}', '\u{F8FF}', '\u{F0000}', '\u{10FFFD}'] {
            assert!(
                in_private_use(character),
                "{}",
                codepoint::render(character)
            );
        }
        // A noncharacter is `Script::Unknown` too, and is not private
        // use — the two share a kind and must not share a reason.
        assert!(!in_private_use('\u{FFFF}'));
        assert!(!in_private_use('a'));
    }

    #[test]
    fn the_offset_is_a_byte_offset_into_the_document() {
        // Two multi-byte characters ahead of the finding: "é" is two
        // bytes and "日" is three.
        let found = scan("é日\u{202E}");
        assert_eq!(found[0].offset, 5);
    }

    /// The report-safety rule, at the layer that builds the fields.
    #[test]
    fn no_draft_carries_the_character_it_reports() {
        for (character, _) in BIDI_CONTROLS.iter().chain(INVISIBLES.iter()) {
            let found = scan(&format!("a{character}b"));
            let rendered = format!("{found:?}");
            assert!(!rendered.contains(*character), "{rendered}");
        }
    }

    /// A table with a duplicate would report one character twice, and a
    /// character in two tables would report it under two kinds.
    #[test]
    fn the_tables_do_not_overlap() {
        let mut all: Vec<char> = BIDI_CONTROLS
            .iter()
            .chain(INVISIBLES.iter())
            .chain(UNUSUAL_SPACES.iter())
            .map(|(character, _)| *character)
            .collect();
        let total = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), total, "a character is listed twice");
    }

    /// Every table entry must actually be a character the classifier
    /// answers for — an entry that classified as nothing would be a
    /// silently dead row.
    #[test]
    fn every_table_entry_classifies() {
        for (character, name) in BIDI_CONTROLS
            .iter()
            .chain(INVISIBLES.iter())
            .chain(UNUSUAL_SPACES.iter())
        {
            let (_, _, detail) = classify(*character).expect(name);
            assert!(detail.starts_with(name), "{detail}");
        }
    }
}
