//! Key paths in `.env` files: one key, one value, one line.
//!
//! The simplest reader here, and one where a finding's key earns its
//! keep: a no-break space or a zero-width joiner inside a secret is a
//! value that compares unequal to the one that was meant, and
//! `STRIPE_SECRET_KEY` says which one far faster than a line number in a
//! file of ninety of them.

use super::locate::{KeySpan, lines};

pub(crate) fn key_spans(text: &str) -> Vec<KeySpan> {
    lines(text).filter_map(span_of).collect()
}

fn span_of((offset, line): (usize, &str)) -> Option<KeySpan> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let equals = line.find('=')?;
    let declared = line[..equals].trim();
    let key = declared.strip_prefix("export ").unwrap_or(declared).trim();
    if key.is_empty() {
        return None;
    }

    let raw = &line[equals + 1..];
    Some(KeySpan {
        start: offset + equals + 1,
        end: offset + equals + 1 + value_length(raw),
        path: key.to_string(),
    })
}

/// How far the value runs.
///
/// A `#` inside a quoted value is part of the value — that is the whole
/// reason quoting exists in these files — so the comment is only cut off
/// an unquoted one.
fn value_length(raw: &str) -> usize {
    let leading = raw.len() - raw.trim_start().len();
    let body = raw.trim_start();
    if body.starts_with('"') || body.starts_with('\'') {
        return raw.len();
    }
    leading + body.find(" #").unwrap_or(body.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyed(text: &str, expected: &[(&str, &str)]) {
        let spans = key_spans(text);
        let found: Vec<(&str, &str)> = spans
            .iter()
            .map(|span| (text[span.start..span.end].trim(), span.path.as_str()))
            .collect();
        assert_eq!(found, expected, "{text}");
    }

    #[test]
    fn a_value_is_named_by_its_key() {
        keyed("TOKEN=abc\n", &[("abc", "TOKEN")]);
    }

    #[test]
    fn an_export_prefix_is_not_part_of_the_key() {
        keyed("export TOKEN=abc\n", &[("abc", "TOKEN")]);
    }

    #[test]
    fn a_comment_line_has_no_key() {
        keyed("# TOKEN=1\nTOKEN=2\n", &[("2", "TOKEN")]);
    }

    /// The quoting rule: a `#` inside quotes is value, not comment.
    #[test]
    fn a_trailing_comment_is_cut_from_an_unquoted_value_only() {
        keyed("A=one two # a note\n", &[("one two", "A")]);
        keyed("B=\"one # two\"\n", &[("\"one # two\"", "B")]);
    }

    #[test]
    fn a_line_with_no_equals_has_no_key() {
        assert!(key_spans("NOT_A_PAIR\n").is_empty());
        assert!(key_spans("=novalue\n").is_empty());
    }

    #[test]
    fn the_spans_are_ordered_and_do_not_overlap() {
        let spans = key_spans("A=1\nB=2\nC=3\n");
        assert_eq!(spans.len(), 3);
        for pair in spans.windows(2) {
            assert!(pair[0].end <= pair[1].start, "{pair:?}");
        }
    }
}
