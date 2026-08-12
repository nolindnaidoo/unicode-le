//! Which normalization form the document is in — reported, never fixed.
//!
//! Two strings that are canonically equivalent can have different bytes,
//! and almost every consumer compares bytes: a hash, a database unique
//! index, a `Map` key, a git path. `é` written as one codepoint and `é`
//! written as `e` plus a combining acute are the same character to a
//! reader and two different keys to everything else. That is the
//! data-corruption half of this tool, and it is a real one — a macOS
//! filesystem hands out NFD, most editors write NFC, and a file that
//! carries both is a lookup that silently misses.
//!
//! **It reports the form and does not rewrite the file.** Normalizing is
//! a content change with consequences this tool cannot see: a fixture
//! may be NFD *on purpose*, and a test that pins a decomposed sequence
//! is a test that fails the moment something helpfully composes it.
//!
//! One finding per line, at the first character where the line and its
//! NFC form part company. Per line rather than per file because a
//! position that says "somewhere in this file" is not a position; per
//! line rather than per character because a decomposed document would
//! otherwise report every letter in it.

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::is_nfc;
use unicode_normalization::is_nfd;

use super::{Draft, Kind, Severity, codepoint};

pub(crate) fn scan(content: &str) -> Vec<Draft> {
    let mut drafts = Vec::new();
    let mut line_start = 0;
    for line in content.split_inclusive('\n') {
        if let Some(draft) = draft(line, line_start) {
            drafts.push(draft);
        }
        line_start += line.len();
    }
    drafts
}

fn draft(line: &str, line_start: usize) -> Option<Draft> {
    if is_nfc(line) {
        return None;
    }
    let (offset, characters) = first_divergence(line);
    // NFKD is a subset of NFD, so a line that is NFD may or may not also
    // be NFKD and saying which buys nothing. What a reader needs is
    // whether this is a decomposed document — one systematic difference
    // with one fix — or a mixture, which is a different problem.
    let form = if is_nfd(line) {
        "NFD"
    } else {
        "neither NFC nor NFD"
    };
    Some(Draft {
        offset: line_start + offset,
        kind: Kind::NonNfc,
        severity: Severity::Low,
        codepoints: codepoint::render_each(characters),
        scripts: Vec::new(),
        resembles: Vec::new(),
        detail: format!(
            "this line is in {form}, not NFC: it compares unequal, hashes differently and \
             indexes differently from the same text written in NFC"
        ),
    })
}

/// The byte offset of the first character that differs from the line's
/// NFC form, and the characters at that point.
///
/// The offset is into the *original*, which is the only offset a reader
/// can act on. The characters reported are the original's, from the
/// divergence to the end of the combining sequence — for `e` + U+0301
/// that is both of them, which is what makes the finding readable.
fn first_divergence(line: &str) -> (usize, Vec<char>) {
    let mut normalized = line.nfc();
    for (offset, character) in line.char_indices() {
        if normalized.next() == Some(character) {
            continue;
        }
        let sequence = line[offset..]
            .chars()
            .take(1)
            .chain(
                line[offset..]
                    .chars()
                    .skip(1)
                    .take_while(|next| is_combining(*next)),
            )
            .collect();
        return (offset, sequence);
    }
    // Only reachable if the line is NFC, which the caller has excluded.
    (0, Vec::new())
}

fn is_combining(character: char) -> bool {
    unicode_normalization::char::canonical_combining_class(character) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composed_text_is_not_a_finding() {
        assert!(scan("café\n").is_empty());
        assert!(scan("plain ascii\n").is_empty());
        assert!(scan("").is_empty());
        assert!(scan("日本語 Привет\n").is_empty());
    }

    #[test]
    fn a_decomposed_line_names_the_form_and_the_sequence() {
        let found = scan("cafe\u{301}\n");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, Kind::NonNfc);
        assert_eq!(found[0].severity, Severity::Low);
        assert!(found[0].detail.contains("NFD"), "{}", found[0].detail);
        assert_eq!(found[0].codepoints, ["U+0065", "U+0301"]);
        assert_eq!(found[0].offset, 3, "the base letter, not the mark");
    }

    /// A line that is neither NFC nor NFD is a mixture, and saying "NFD"
    /// there would send someone to the wrong fix.
    #[test]
    fn a_mixed_line_is_named_as_neither() {
        let found = scan("café + e\u{301}\n");
        assert_eq!(found.len(), 1);
        assert!(found[0].detail.contains("neither"), "{}", found[0].detail);
    }

    #[test]
    fn each_line_is_judged_on_its_own() {
        let found = scan("cafe\u{301}\nclean\nre\u{301}sume\u{301}\n");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].offset, 3);
        // Line one is seven bytes and line two six, so line three starts
        // at 13 and its `r` is fine — the divergence is the `e` after it.
        assert_eq!(found[1].offset, 14);
    }

    /// A file with no trailing newline still has a last line.
    #[test]
    fn the_last_line_needs_no_newline() {
        assert_eq!(scan("cafe\u{301}").len(), 1);
    }

    /// Composition can cross a starter boundary — Hangul jamo are all
    /// starters — so a per-starter segmentation would miss this, which
    /// is why the check is per line.
    #[test]
    fn decomposed_hangul_is_found() {
        let found = scan("\u{1100}\u{1161}\n");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].codepoints, ["U+1100"]);
    }

    #[test]
    fn nothing_is_rewritten() {
        let content = "cafe\u{301}\n";
        let before = content.to_string();
        let _ = scan(content);
        assert_eq!(content, before);
    }
}
