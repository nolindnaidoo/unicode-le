//! Bytes → text, or a refusal. Never a guess.
//!
//! Every other tool in this family may reasonably shrug at an encoding
//! it does not recognise, because a mis-decoded file yields no findings
//! and no harm. Here a mis-decode **invents** findings: read a UTF-16
//! file as if it were UTF-8 or Latin-1 and every second byte is a NUL or
//! a stray high byte, so a tool that guessed would report a document
//! full of invisible and unassigned characters that do not exist. That
//! is the worst failure available to a security screen — a confident,
//! detailed, entirely fabricated answer.
//!
//! So the order is: byte-order mark first, binary sniff second, UTF-8
//! last, and anything that is not UTF-8 is refused by name.
//!
//! **A leading UTF-8 BOM is not stripped.** Sibling crates drop it,
//! because for them it only shifts a column. Here the report carries
//! byte offsets into the file on disk, and silently deleting three bytes
//! from the front would make every offset in the report wrong by three.
//! It is excluded from the *findings* instead — see `characters.rs` —
//! which is a rule about meaning rather than a rewrite of the input.

use super::{Reason, Refusal};

/// How much of a file the binary test reads. Ripgrep's number: a file
/// that has made it 8 KB without a NUL byte is text as far as any tool
/// that greps it is concerned.
const BINARY_SNIFF_BYTES: usize = 8192;

#[derive(Debug)]
pub(crate) enum Decoded<'a> {
    Text(&'a str),
    Refused(Refusal),
}

/// The byte-order marks this refuses, longest first — `FF FE 00 00` is
/// UTF-32LE and also *starts* with the UTF-16LE mark, so testing the
/// two-byte marks first would name the wrong encoding.
const MARKS: [(&[u8], &str); 4] = [
    (&[0x00, 0x00, 0xFE, 0xFF], "UTF-32BE"),
    (&[0xFF, 0xFE, 0x00, 0x00], "UTF-32LE"),
    (&[0xFE, 0xFF], "UTF-16BE"),
    (&[0xFF, 0xFE], "UTF-16LE"),
];

pub(crate) fn decode(bytes: &[u8]) -> Decoded<'_> {
    if let Some(encoding) = byte_order_mark(bytes) {
        return Decoded::Refused(Refusal {
            reason: Reason::EncodingUnknown,
            detail: format!(
                "a {encoding} byte-order mark: this reads UTF-8 only, and decoding {encoding} \
                 as UTF-8 would invent invisible and unassigned characters that are not in the \
                 file"
            ),
        });
    }
    if let Some(offset) = first_nul(bytes) {
        return Decoded::Refused(Refusal {
            reason: Reason::BinaryOrUndecodable,
            detail: format!("a NUL byte at byte {offset}: this was never a text candidate"),
        });
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => Decoded::Text(text),
        Err(error) => Decoded::Refused(Refusal {
            reason: Reason::BinaryOrUndecodable,
            detail: format!(
                "not valid UTF-8 at byte {}: the encoding is unknown and is not guessed",
                error.valid_up_to()
            ),
        }),
    }
}

fn byte_order_mark(bytes: &[u8]) -> Option<&'static str> {
    MARKS
        .iter()
        .find(|(mark, _)| bytes.starts_with(mark))
        .map(|(_, encoding)| *encoding)
}

fn first_nul(bytes: &[u8]) -> Option<usize> {
    bytes
        .iter()
        .take(BINARY_SNIFF_BYTES)
        .position(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refusal(bytes: &[u8]) -> Refusal {
        match decode(bytes) {
            Decoded::Refused(refusal) => refusal,
            Decoded::Text(text) => panic!("decoded {text:?} instead of refusing"),
        }
    }

    fn text(bytes: &[u8]) -> String {
        match decode(bytes) {
            Decoded::Text(text) => text.to_string(),
            Decoded::Refused(refusal) => panic!("refused: {}", refusal.detail),
        }
    }

    #[test]
    fn ordinary_utf8_decodes() {
        assert_eq!(text(b"hello\n"), "hello\n");
        assert_eq!(text(&[]), "");
        assert_eq!(text("naïve — 日本語".as_bytes()), "naïve — 日本語");
    }

    /// The three bytes stay, because the offsets in the report address
    /// the file on disk and a silent deletion would move all of them.
    #[test]
    fn a_utf8_byte_order_mark_is_kept_rather_than_stripped() {
        assert_eq!(text("\u{feff}let a = 1;".as_bytes()), "\u{feff}let a = 1;");
    }

    #[test]
    fn a_utf16_byte_order_mark_is_refused_by_name() {
        for (bytes, named) in [
            (vec![0xFF, 0xFE, b'a', 0x00], "UTF-16LE"),
            (vec![0xFE, 0xFF, 0x00, b'a'], "UTF-16BE"),
        ] {
            let refusal = refusal(&bytes);
            assert_eq!(refusal.reason, Reason::EncodingUnknown);
            assert!(refusal.detail.contains(named), "{}", refusal.detail);
        }
    }

    /// `FF FE 00 00` is UTF-32LE and also starts with the UTF-16LE mark.
    /// Testing the short marks first names the wrong encoding, which is
    /// the kind of confidently wrong answer this module exists to avoid.
    #[test]
    fn a_utf32_mark_is_not_mistaken_for_a_utf16_one() {
        assert!(
            refusal(&[0xFF, 0xFE, 0x00, 0x00, b'a', 0, 0, 0])
                .detail
                .contains("UTF-32LE")
        );
        assert!(
            refusal(&[0x00, 0x00, 0xFE, 0xFF, 0, 0, 0, b'a'])
                .detail
                .contains("UTF-32BE")
        );
    }

    /// A UTF-16 file usually carries NUL bytes too, so the mark has to be
    /// read before the binary sniff or the refusal names the wrong thing.
    #[test]
    fn the_mark_is_read_before_the_binary_sniff() {
        let utf16 = [0xFF, 0xFE, b'a', 0x00, b'b', 0x00];
        assert_eq!(refusal(&utf16).reason, Reason::EncodingUnknown);
    }

    #[test]
    fn a_nul_byte_is_a_file_that_was_never_text() {
        let refusal = refusal(&[0x89, 0x50, 0x4E, 0x47, 0x00, 0x1A]);
        assert_eq!(refusal.reason, Reason::BinaryOrUndecodable);
        assert!(refusal.detail.contains("NUL"), "{}", refusal.detail);
    }

    #[test]
    fn a_nul_byte_past_the_first_pages_is_not_sniffed() {
        let mut bytes = vec![b'a'; BINARY_SNIFF_BYTES];
        bytes.push(0);
        assert_eq!(first_nul(&bytes), None);
        assert_eq!(first_nul(&[b'a', 0]), Some(1));
    }

    /// Latin-1 has no NUL and no mark. It is still not UTF-8, and the
    /// refusal says where it stopped being one rather than guessing.
    #[test]
    fn invalid_utf8_is_refused_with_the_byte_it_failed_at() {
        let refusal = refusal(&[b'c', b'a', b'f', 0xE9]);
        assert_eq!(refusal.reason, Reason::BinaryOrUndecodable);
        assert!(refusal.detail.contains("byte 3"), "{}", refusal.detail);
    }

    /// No refusal may offer to try another encoding, because the whole
    /// rule is that it never does.
    #[test]
    fn no_refusal_offers_to_guess() {
        for bytes in [
            vec![0xFF, 0xFE, b'a', 0x00],
            vec![b'c', b'a', b'f', 0xE9],
            vec![0x00],
        ] {
            let detail = refusal(&bytes).detail;
            for word in ["latin-1", "assumed", "fell back", "probably"] {
                assert!(!detail.to_lowercase().contains(word), "{detail}");
            }
        }
    }
}
