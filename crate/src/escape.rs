//! Making the one string this crate does not author safe to look at.
//!
//! Every field in a finding is written here — `U+XXXX` codepoints and
//! English prose — so `detect::codepoint` can guarantee the whole of it
//! is ASCII. **The `file` field is the exception**, and it was the
//! exploitable one: it is the path the caller handed in, echoed back so
//! that it can be opened and grepped for. A repository holding a file
//! named `invoice`, U+202E, `fdp.ts` therefore produced a report that
//! reordered the terminal, the diff and the pull request of whoever read
//! it — the delivery mechanism this tool exists not to be, reached
//! through the one string it does not write.
//!
//! `rustc` refuses that character in this file's own doc comments, for
//! the same reason and since the same CVE. This module is the crate
//! doing for its output what the compiler does for its input.
//!
//! Two escapes, because the two streams promise different things:
//!
//! - [`json`] rewrites a **serialized JSON document** so that every
//!   non-ASCII codepoint is a `\uXXXX` escape. This is JSON's own
//!   escape, so a parser decodes it back to the identical string and the
//!   path still opens: the machine contract does not move, and only the
//!   raw bytes on the wire become inert.
//! - [`text`] rewrites a **plain string** the same way, for stderr,
//!   where there is no parser and nothing to round-trip to.
//!
//! Both leave ASCII untouched, so the ordinary case allocates nothing a
//! reader would notice and every existing path is spelled exactly as it
//! was.

use std::fmt::Write as _;

/// Every non-ASCII codepoint in a **serialized JSON document**, rewritten
/// as a `\uXXXX` escape.
///
/// Applied to the finished document rather than to the field, and that
/// is what makes it correct rather than merely clever. Escaping the
/// string *before* serialization would not survive: `serde_json` escapes
/// the backslash it found, so a path holding U+202E would ship as
/// `\\u202E` and parse back as six literal characters rather than as the
/// path that opens the file.
///
/// Sound because **JSON's own syntax is ASCII**. Outside a string
/// literal a JSON document holds only structural punctuation, digits,
/// the three bare words and whitespace; a non-ASCII codepoint can
/// therefore only occur inside a string, which is exactly where `\uXXXX`
/// is defined and equivalent.
pub(crate) fn json(document: &str) -> String {
    rewrite(document)
}

/// Every non-ASCII codepoint in a plain string, rewritten as `\uXXXX`.
///
/// The same spelling as [`json`] on purpose: a path in the human summary
/// and the same path in the report read identically, so one can be
/// grepped for with the other. Nothing decodes this one — stderr is
/// prose, and a reader who needs the real bytes has the report.
pub(crate) fn text(value: &str) -> String {
    rewrite(value)
}

/// Astral codepoints become a **surrogate pair**, because JSON's
/// `\uXXXX` is sixteen bits: U+1D41A is `𝐚`. Emitting
/// `ᵁA` instead would be five hex digits where the grammar allows
/// four, and a parser would read the fifth as an ordinary character.
fn rewrite(value: &str) -> String {
    // The overwhelmingly common case, and worth the branch: a path is
    // almost always ASCII, and this keeps it byte-identical rather than
    // rebuilt.
    if value.is_ascii() {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_ascii() {
            out.push(character);
            continue;
        }
        let mut buffer = [0u16; 2];
        for unit in character.encode_utf16(&mut buffer) {
            let _ = write!(out, "\\u{unit:04X}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_is_returned_unchanged() {
        for value in ["", "src/a.ts", "{\"file\":\"src/a.ts\"}", "a\\b"] {
            assert_eq!(text(value), value);
            assert_eq!(json(value), value);
        }
    }

    #[test]
    fn a_bidi_control_becomes_an_escape() {
        assert_eq!(text("invoice\u{202E}fdp.ts"), "invoice\\u202Efdp.ts");
    }

    /// Upper-case hex and four digits, matching `U+XXXX` elsewhere in
    /// the report so the two can be grepped for together.
    #[test]
    fn the_spelling_is_four_upper_case_hex_digits() {
        assert_eq!(text("\u{e9}"), "\\u00E9");
        assert_eq!(text("\u{200B}"), "\\u200B");
        assert_eq!(text("\u{FEFF}"), "\\uFEFF");
    }

    /// The one that a naive implementation gets wrong. JSON's escape is
    /// sixteen bits, so anything above the basic plane is two of them.
    #[test]
    fn an_astral_character_becomes_a_surrogate_pair() {
        assert_eq!(text("\u{1D41A}"), "\\uD835\\uDC1A");
        assert_eq!(text("\u{10FFFD}"), "\\uDBFF\\uDFFD");
    }

    #[test]
    fn everything_it_returns_is_ascii() {
        for value in [
            "invoice\u{202E}fdp.ts",
            "\u{4e2d}\u{6587}/\u{1D41A}.json",
            "caf\u{e9}",
        ] {
            assert!(text(value).is_ascii(), "{value}");
            assert!(json(value).is_ascii(), "{value}");
        }
    }

    /// **The property the whole design rests on.** A parser handed the
    /// escaped document gives back the original string, byte for byte,
    /// so the path in the report is still the path that opens.
    #[test]
    fn an_escaped_document_parses_back_to_the_original_string() {
        for path in [
            "invoice\u{202E}fdp.ts",
            "src/\u{4e2d}\u{6587}/a.ts",
            "\u{1D41A}\u{10FFFD}.md",
            "caf\u{e9} name.ts",
            "with\"quote\\and\u{202E}.ts",
        ] {
            let document =
                serde_json::to_string(&serde_json::json!({ "file": path })).expect("serializes");
            let escaped = json(&document);
            assert!(escaped.is_ascii(), "{escaped}");
            let parsed: serde_json::Value =
                serde_json::from_str(&escaped).expect("the escaped document is still JSON");
            assert_eq!(parsed["file"], path, "{escaped}");
        }
    }

    /// A document whose *structure* is ASCII and whose strings are not
    /// keeps its structure: the rewrite touches string contents only,
    /// because that is the only place a non-ASCII byte can be.
    #[test]
    fn the_structure_of_a_document_survives_the_rewrite() {
        let value = serde_json::json!({
            "files": [{ "file": "a\u{202E}.ts", "findings": [], "n": 1, "ok": true }],
        });
        let escaped = json(&serde_json::to_string(&value).expect("serializes"));
        let parsed: serde_json::Value = serde_json::from_str(&escaped).expect("still JSON");
        assert_eq!(parsed, value);
    }
}
