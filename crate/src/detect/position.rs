//! Byte offset → line/column, where **column is counted in UTF-16 code
//! units**.
//!
//! An editor reports UTF-16 columns, and the entire job of this tool is
//! pointing at one character in a file the reader is about to open. A
//! column that disagrees with the editor's ruler sends them to the wrong
//! place, which for a finding whose whole content is "the character
//! *here* is not what it looks like" is a failure of the report.
//!
//! Counting bytes and counting scalars both diverge, in opposite
//! directions and on exactly the characters this tool finds: an
//! invisible U+2060 is three bytes and one code unit; a mathematical
//! U+1D41A is four bytes, one scalar and **two** code units. UTF-16 is
//! the only count that matches what the reader sees.
//!
//! The byte offset is reported alongside, untouched, for the callers
//! that address the file rather than the editor.
//!
//! Lines and columns are 1-based.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Position {
    pub(crate) line: usize,
    pub(crate) column: usize,
}

/// A prepared index over one document. Building it is O(bytes). A lookup
/// is a binary search, then a column: arithmetic when the document is
/// ASCII, and a bounded scan from the nearest checkpoint when it is not.
pub(crate) struct PositionIndex<'a> {
    content: &'a str,
    /// Byte offset of the first character of each line.
    line_starts: Vec<usize>,
    /// `(byte offset, UTF-16 code units before it)`, every
    /// `CHECKPOINT_BYTES` or so.
    ///
    /// **Empty when the document is ASCII**, where a byte offset *is* a
    /// UTF-16 offset and a column is arithmetic. Without the
    /// checkpoints, a non-ASCII document re-counts code units from the
    /// line start on every lookup — invisible on a source file, and
    /// quadratic on the one shape this tool is pointed at most: a
    /// minified bundle, which is one line holding the whole document,
    /// and one lookup per finding in it. Measured on a debug build:
    /// 5,000 findings on such a line took 1.4s, 10,000 took 5.5s and
    /// 20,000 took 21.8s, which is the shape of a square. `ips-le` hit
    /// the same shape on a log with a megabyte line.
    checkpoints: Vec<(usize, usize)>,
}

/// How far a lookup may have to scan. Small enough that the scan is
/// irrelevant, large enough that the index is a rounding error on a
/// document's size.
const CHECKPOINT_BYTES: usize = 1024;

impl<'a> PositionIndex<'a> {
    pub(crate) fn new(content: &'a str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            content
                .bytes()
                .enumerate()
                .filter(|&(_, byte)| byte == b'\n')
                .map(|(index, _)| index + 1),
        );
        Self {
            content,
            line_starts,
            checkpoints: checkpoints(content),
        }
    }

    /// The position of a byte offset. Offsets past the end clamp to the
    /// end, and an offset landing inside a multi-byte character floors
    /// to that character's start — neither can happen from a scan of
    /// this document, but a silently wrong column would be worse than a
    /// defensive floor.
    pub(crate) fn at(&self, offset: usize) -> Position {
        let clamped = self.floor_to_boundary(offset.min(self.content.len()));
        let line_index = self.line_starts.partition_point(|&start| start <= clamped) - 1;
        let line_start = self.line_starts[line_index];
        Position {
            line: line_index + 1,
            column: self.units_before(clamped) - self.units_before(line_start) + 1,
        }
    }

    /// UTF-16 code units before a byte offset, from the nearest
    /// checkpoint at or below it.
    fn units_before(&self, offset: usize) -> usize {
        // ASCII: one byte, one code unit, no index needed.
        let Some(&(byte, units)) = self.checkpoints.get(
            self.checkpoints
                .partition_point(|(at, _)| *at <= offset)
                .wrapping_sub(1),
        ) else {
            return offset;
        };
        units + self.content[byte..offset].encode_utf16().count()
    }

    fn floor_to_boundary(&self, mut offset: usize) -> usize {
        while offset > 0 && !self.content.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }
}

/// Running UTF-16 counts at char boundaries roughly `CHECKPOINT_BYTES`
/// apart. Empty for an ASCII document, which needs none.
fn checkpoints(content: &str) -> Vec<(usize, usize)> {
    if content.is_ascii() {
        return Vec::new();
    }
    let mut out = vec![(0, 0)];
    let mut units = 0;
    let mut next = CHECKPOINT_BYTES;
    for (offset, character) in content.char_indices() {
        if offset >= next {
            out.push((offset, units));
            next = offset + CHECKPOINT_BYTES;
        }
        units += character.len_utf16();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_character_is_line_one_column_one() {
        let index = PositionIndex::new("abc");
        assert_eq!(index.at(0), Position { line: 1, column: 1 });
    }

    #[test]
    fn a_newline_starts_the_next_line() {
        let index = PositionIndex::new("ab\ncd");
        assert_eq!(index.at(3), Position { line: 2, column: 1 });
        assert_eq!(index.at(4), Position { line: 2, column: 2 });
    }

    #[test]
    fn an_empty_document_still_answers() {
        let index = PositionIndex::new("");
        assert_eq!(index.at(0), Position { line: 1, column: 1 });
    }

    #[test]
    fn an_offset_past_the_end_clamps() {
        let index = PositionIndex::new("ab");
        assert_eq!(index.at(999), Position { line: 1, column: 3 });
    }

    /// A three-byte invisible is one UTF-16 code unit, so the column
    /// after it advances by one. Byte counting answers 4 here.
    #[test]
    fn a_three_byte_invisible_counts_as_one_column() {
        let index = PositionIndex::new("a\u{2060}b");
        assert_eq!(index.at(4), Position { line: 1, column: 3 });
    }

    /// A mathematical letter is a surrogate pair: two UTF-16 code units
    /// from four bytes. Counting scalars answers 2 here, and this is
    /// precisely a character the confusable check reports.
    #[test]
    fn an_astral_character_counts_as_two_columns() {
        let index = PositionIndex::new("\u{1d41a}!");
        assert_eq!(index.at(4), Position { line: 1, column: 3 });
    }

    #[test]
    fn utf16_counting_restarts_on_each_line() {
        let index = PositionIndex::new("é\né!");
        assert_eq!(index.at(5), Position { line: 2, column: 2 });
    }

    #[test]
    fn an_offset_inside_a_character_floors_to_its_start() {
        let index = PositionIndex::new("é!");
        assert_eq!(index.at(1), Position { line: 1, column: 1 });
    }

    /// The naive answer, written out once so the indexed one has
    /// something to be held to.
    fn counted(content: &str, offset: usize) -> Position {
        let line = content[..offset].bytes().filter(|b| *b == b'\n').count();
        let line_start = content[..offset].rfind('\n').map_or(0, |index| index + 1);
        Position {
            line: line + 1,
            column: content[line_start..offset].encode_utf16().count() + 1,
        }
    }

    /// The indexed path and the counted path must agree at **every**
    /// offset, or the checkpoint index is a second implementation with
    /// its own answers. Run over an ASCII document, which takes the
    /// no-index path, and a non-ASCII one long enough to cross several
    /// checkpoints — including one that is a single line, which is the
    /// shape the checkpoints exist for.
    #[test]
    fn the_indexed_path_agrees_with_the_counted_path_everywhere() {
        let ascii = "abc\ndef\nghi";
        let mut lines = String::new();
        let mut one_line = String::new();
        for n in 0..400 {
            use std::fmt::Write as _;
            let _ = writeln!(lines, "cafe\u{301} {n} \u{1d41a}");
            let _ = write!(one_line, "cafe\u{301} {n} \u{1d41a} ");
        }
        for content in [ascii, lines.as_str(), one_line.as_str()] {
            let index = PositionIndex::new(content);
            assert!(
                content.is_ascii() || index.checkpoints.len() > 3,
                "the long documents must cross several checkpoints"
            );
            for offset in 0..=content.len() {
                if !content.is_char_boundary(offset) {
                    continue;
                }
                assert_eq!(index.at(offset), counted(content, offset), "at {offset}");
            }
        }
    }

    /// An ASCII document builds no index at all — the cheapest path is
    /// also the common one.
    #[test]
    fn an_ascii_document_needs_no_checkpoints() {
        assert!(PositionIndex::new("abc\ndef").checkpoints.is_empty());
    }

    /// A carriage return is an ordinary character, not a line break.
    #[test]
    fn a_carriage_return_does_not_start_a_line() {
        let index = PositionIndex::new("a\r\nb");
        assert_eq!(index.at(1), Position { line: 1, column: 2 });
        assert_eq!(index.at(3), Position { line: 2, column: 1 });
    }
}
