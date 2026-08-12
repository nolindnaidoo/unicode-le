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

/// A prepared index over one document. Building it is O(bytes); each
/// lookup is a binary search plus a UTF-16 count of the current line's
/// prefix, which is bounded by the line length rather than the file.
pub(crate) struct PositionIndex<'a> {
    content: &'a str,
    /// Byte offset of the first character of each line.
    line_starts: Vec<usize>,
}

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
        let column = self.content[line_start..clamped].encode_utf16().count() + 1;
        Position {
            line: line_index + 1,
            column,
        }
    }

    fn floor_to_boundary(&self, mut offset: usize) -> usize {
        while offset > 0 && !self.content.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }
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

    /// A carriage return is an ordinary character, not a line break.
    #[test]
    fn a_carriage_return_does_not_start_a_line() {
        let index = PositionIndex::new("a\r\nb");
        assert_eq!(index.at(1), Position { line: 1, column: 2 });
        assert_eq!(index.at(3), Position { line: 2, column: 1 });
    }
}
