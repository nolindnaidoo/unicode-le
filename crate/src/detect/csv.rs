//! Key paths in CSV: the header row names the columns.
//!
//! The header is the only key material a CSV has, and it is worth
//! taking: a column of names in which one row carries a Cyrillic `a` is
//! exactly the shape this tool exists to find, and `customer_name` says
//! more about it than "field 4" does.
//!
//! A column with no name in the header is `[n]`, matching the array
//! convention everywhere else. The first row is treated as a header
//! unconditionally — a file with no header loses one row's key paths and
//! not one row's findings, because the character scan runs over the raw
//! text either way.

use super::locate::{KeySpan, lines};

/// The byte between fields. Tab-separated files are the same grammar
/// with a different one, and reading a tab row on commas made the whole
/// header one column name — so a hazard in the `name` column was
/// reported under the key `id\tname\tcity`, which names no column.
pub(crate) const COMMA: u8 = b',';
pub(crate) const TAB: u8 = b'\t';

pub(crate) fn key_spans(text: &str, delimiter: u8) -> Vec<KeySpan> {
    let mut rows = lines(text).filter(|(_, line)| !line.trim().is_empty());
    let Some((_, header_line)) = rows.next() else {
        return Vec::new();
    };
    let headers: Vec<String> = fields(header_line, delimiter)
        .into_iter()
        .map(|field| header_line[field].trim().trim_matches('"').to_string())
        .collect();

    let mut spans = Vec::new();
    for (offset, line) in rows {
        for (column, field) in fields(line, delimiter).into_iter().enumerate() {
            spans.push(KeySpan {
                start: offset + field.start,
                end: offset + field.end,
                path: headers
                    .get(column)
                    .filter(|name| !name.is_empty())
                    .cloned()
                    .unwrap_or_else(|| format!("[{column}]")),
            });
        }
    }
    spans
}

/// The byte range of each field in one row.
///
/// A quoted field keeps its quotes in the range. Trimming them would
/// mean two sets of offsets to keep straight, and the offsets are what
/// the report is indexed by.
fn fields(line: &str, delimiter: u8) -> Vec<std::ops::Range<usize>> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    let mut quoted = false;

    for (at, byte) in bytes.iter().enumerate() {
        match byte {
            b'"' => quoted = !quoted,
            _ if *byte == delimiter && !quoted => {
                out.push(start..at);
                start = at + 1;
            }
            _ => {}
        }
    }
    out.push(start..bytes.len());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyed(text: &str, expected: &[(&str, &str)]) {
        let spans = key_spans(text, COMMA);
        let found: Vec<(&str, &str)> = spans
            .iter()
            .map(|span| (text[span.start..span.end].trim(), span.path.as_str()))
            .collect();
        assert_eq!(found, expected, "{text}");
    }

    /// The delimiter is the whole fix: read on commas, a tab row is one
    /// field, so the entire header became the key of every hazard in the
    /// row — a name that names no column.
    #[test]
    fn a_tab_row_is_columns_under_tab_and_one_column_under_comma() {
        let text = "id\tname\tcity\n1\tvalue\tparis\n";
        let tabbed: Vec<String> = key_spans(text, TAB)
            .iter()
            .map(|s| s.path.clone())
            .collect();
        assert_eq!(tabbed, ["id", "name", "city"]);
        let comma: Vec<String> = key_spans(text, COMMA)
            .iter()
            .map(|s| s.path.clone())
            .collect();
        assert_eq!(comma, ["id\tname\tcity"]);
    }

    #[test]
    fn a_field_is_named_by_its_column_header() {
        keyed(
            "name,city\nAda,Paris\n",
            &[("Ada", "name"), ("Paris", "city")],
        );
    }

    #[test]
    fn every_row_after_the_header_is_keyed() {
        keyed("id\n1\n2\n", &[("1", "id"), ("2", "id")]);
    }

    #[test]
    fn a_quoted_field_may_hold_a_comma() {
        keyed(
            "name,note\nAda,\"one, two\"\n",
            &[("Ada", "name"), ("\"one, two\"", "note")],
        );
    }

    #[test]
    fn a_column_with_no_header_name_is_indexed() {
        keyed("a,\nx,y\n", &[("x", "a"), ("y", "[1]")]);
    }

    #[test]
    fn a_header_alone_yields_nothing() {
        assert!(key_spans("name,city\n", COMMA).is_empty());
        assert!(key_spans("", COMMA).is_empty());
    }

    #[test]
    fn blank_lines_are_not_rows() {
        keyed("id\n\n1\n", &[("1", "id")]);
    }

    #[test]
    fn the_spans_are_ordered_and_do_not_overlap() {
        let spans = key_spans("a,b\n1,2\n3,4\n", COMMA);
        assert_eq!(spans.len(), 4);
        for pair in spans.windows(2) {
            assert!(pair[0].end <= pair[1].start, "{pair:?}");
        }
    }
}
