//! How an offending character reaches the report: as `U+XXXX`, and
//! never as itself.
//!
//! **This is the safety rule the whole tool rests on.** Every other
//! scanner in this family can quote what it found — a regex, a path, a
//! masked secret — because quoting it is inert. Here it is not. A report
//! that pasted a raw U+202E carries the attack out of the file and into
//! the terminal, the pull request, the chat window and the log of
//! whoever reads the report. The same is true of a zero-width joiner
//! pasted into a diff and of a homoglyph pasted into a ticket title.
//!
//! So nothing in a finding is ever the source text. `codepoints` and
//! `resembles` are the only representation of the characters involved,
//! and both are `U+XXXX` — ASCII, inert, and greppable. `detail` is
//! English prose written here, never text taken from the document.
//! `detect::hazards` asserts it over the whole corpus.

/// `U+XXXX`, upper-case hex, at least four digits — the spelling every
/// Unicode document uses, so a finding can be pasted into a search.
pub(crate) fn render(c: char) -> String {
    format!("U+{:04X}", c as u32)
}

pub(crate) fn render_each<I: IntoIterator<Item = char>>(chars: I) -> Vec<String> {
    chars.into_iter().map(render).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spelling_is_the_one_unicode_documents_use() {
        assert_eq!(render('a'), "U+0061");
        assert_eq!(render('\u{202e}'), "U+202E");
        assert_eq!(render('\u{ad}'), "U+00AD");
    }

    /// Astral characters are five and six digits, not padded to four and
    /// not truncated to it.
    #[test]
    fn an_astral_character_keeps_all_its_digits() {
        assert_eq!(render('\u{1d41a}'), "U+1D41A");
        assert_eq!(render('\u{10fffd}'), "U+10FFFD");
    }

    /// The rendering is the barrier: whatever went in, what comes out is
    /// ASCII and carries none of it.
    #[test]
    fn nothing_rendered_carries_the_character_it_describes() {
        for hazard in ['\u{202e}', '\u{200b}', '\u{feff}', '\u{430}'] {
            let rendered = render(hazard);
            assert!(rendered.is_ascii(), "{rendered}");
            assert!(!rendered.contains(hazard), "{rendered}");
        }
    }

    #[test]
    fn a_sequence_renders_in_order() {
        assert_eq!(render_each(['a', '\u{202e}']), ["U+0061", "U+202E"]);
        assert!(render_each([]).is_empty());
    }
}
