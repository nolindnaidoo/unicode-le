//! Mixed scripts, homoglyphs, and the refusal that makes both usable.
//!
//! This is the module that decides whether the tool is worth running on
//! a real repository. The naive version of a confusable check — "flag
//! every character that resembles an ASCII one" — reports every letter
//! of every Russian, Greek and Chinese string in the tree, which is
//! thousands of findings on exactly the internationalised codebases that
//! most need the check. It gets switched off within a day, and after
//! that the Trojan Source screen is off too.
//!
//! Three rules keep it honest, and they are the design, not a filter
//! bolted on afterwards:
//!
//! **A word is judged, never a file.** UTS #39's resolved script set is
//! computed per word. `Привет` is wholly Cyrillic and is Russian; the
//! Cyrillic `а` in `pаypal` sits in a word that is otherwise Latin, and
//! only that one is a finding. The check fires on *mixing*, so text that
//! is consistently one non-Latin script produces nothing at all. UTS #39
//! also augments the set — Han with Hiragana and Katakana is Japanese,
//! Han with Hangul is Korean — so ordinary Japanese prose is one script
//! rather than three.
//!
//! **A file that is plainly not Latin is refused, not guessed at.** A
//! `zh-CN` catalogue or a CJK fixture has no Latin baseline for a
//! homoglyph to hide against, and the compatibility check below would
//! start reporting full-width letters that are ordinary typography
//! there. Rather than answer badly, this says it did not answer and asks
//! for the script to be named. Everything else — bidi controls,
//! invisibles, spaces, normalization — still runs on that file. The
//! share that triggers it is measured over the scripts the caller did
//! **not** declare: a file the caller has accounted for has a baseline
//! again, and refusing it would make declaring a way of switching the
//! check off rather than of turning it on.
//!
//! **A declared script is an expected script.** Naming one is not just a
//! way past the refusal; it changes what counts. CJK is written without
//! spaces, so `CSVストリーミング` is one word under any segmentation, and
//! a caller who has said the tree contains Han should not then be told
//! that every string mentioning a product name mixes scripts. Mixing
//! Latin with a declared script is a translation. Mixing it with an
//! *undeclared* one is still a finding, in the same file, which is what
//! makes declaring worth doing rather than equivalent to switching the
//! check off.

use unicode_normalization::UnicodeNormalization;
use unicode_script::{Script, UnicodeScript};
use unicode_security::{MixedScript, is_potential_mixed_script_confusable_char, skeleton};

use super::{Draft, Kind, Reason, Refusal, Severity, codepoint};

/// The share of a file's letters that must belong to an undeclared
/// non-Latin script before this refuses to judge the file.
///
/// Ten percent, and the number was measured rather than picked. The
/// thinnest real case is a `zh-CN` UI catalogue, where the keys are
/// ASCII and the Han values are far shorter than the English they
/// replace: 16% of its letters are Han. A single foreign word quoted in
/// an English document is a tenth of one percent. Anywhere between those
/// two separates "this file is written in another script" from "this
/// file mentions one", and ten is comfortably inside it.
const FOREIGN_SHARE_PERCENT: usize = 10;

/// Notation that folds to an ASCII character and is not impersonating
/// one. `2ª`, `x²` and `Hₙ` are ordinary writing; reporting the ordinal
/// indicators would fire on every Spanish and Portuguese document with a
/// numbered list in it.
const NOT_IMPERSONATING: [(char, char); 4] = [
    ('\u{00AA}', '\u{00AA}'), // feminine ordinal indicator
    ('\u{00B2}', '\u{00B3}'), // superscript two and three
    ('\u{00B9}', '\u{00BA}'), // superscript one, masculine ordinal indicator
    ('\u{2070}', '\u{209F}'), // superscripts and subscripts
];

/// Whether this file may be judged for confusables and mixed script, or
/// a refusal saying why it may not.
///
/// **The share is over the undeclared scripts alone.** Measuring every
/// non-Latin letter and then naming only the undeclared ones told a
/// caller that "98% of this file's letters are Cyrillic" about a file
/// holding six Cyrillic letters and four hundred Japanese ones — and
/// refused it, *after* the caller had declared Han, Hiragana and
/// Katakana. That is the worst way for this to be wrong: declaring the
/// script a repository is written in turned the confusable check off for
/// every file in it, which is where a homoglyph would be hiding.
pub(crate) fn context(content: &str, expected: &[Script]) -> Option<Refusal> {
    let (letters, mut found) = survey(content);
    found.retain(|(script, _)| !expected.contains(script));
    if found.is_empty() {
        return None;
    }
    // Percentages, not floats: the comparison has to be the same on
    // every platform, and a report that flipped on a rounding difference
    // would be a report nobody could reproduce.
    let undeclared: usize = found.iter().map(|(_, count)| *count).sum();
    if undeclared * 100 < letters * FOREIGN_SHARE_PERCENT {
        return None;
    }

    found.sort_unstable_by_key(|(script, _)| script.full_name());
    let names: Vec<&str> = found.iter().map(|(script, _)| script.full_name()).collect();
    Some(Refusal {
        reason: Reason::IntentionalScriptContext,
        detail: format!(
            "{}% of this file's letters are {}, {}. The confusable and mixed-script checks did \
             not run here: in a file written in another script there is nothing to tell a forged \
             name from a translated one, and answering anyway would bury the real findings. Every \
             other check did run.",
            undeclared * 100 / letters.max(1),
            join_names(&names),
            declaration(expected)
        ),
    })
}

/// The clause after the script names. A caller who declared nothing is
/// told what to do; a caller who declared something and still got this
/// is told that what they declared does not cover what is in the file —
/// saying "no expected script was declared" to them is simply false, and
/// sends them to check a flag they already passed.
fn declaration(expected: &[Script]) -> &'static str {
    if expected.is_empty() {
        return "and no expected script was declared for it";
    }
    "and none of those is among the scripts declared for it"
}

/// `A`, `A and B`, `A, B and C`. Written out because a bare `join(" and
/// ")` reads as broken prose the moment a third script appears, and
/// Japanese always brings three.
fn join_names(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [only] => (*only).to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// How many letters the file holds, and how many of them belong to each
/// non-Latin script written in it.
///
/// Counted per script rather than summed: the caller subtracts the
/// scripts that were declared, and a single total cannot be subtracted
/// from.
fn survey(content: &str) -> (usize, Vec<(Script, usize)>) {
    let mut letters = 0;
    let mut found: Vec<(Script, usize)> = Vec::new();
    for character in content.chars().filter(|c| c.is_alphabetic()) {
        letters += 1;
        let script = character.script();
        if matches!(
            script,
            Script::Latin | Script::Common | Script::Inherited | Script::Unknown
        ) {
            continue;
        }
        match found.iter_mut().find(|(seen, _)| *seen == script) {
            Some((_, count)) => *count += 1,
            None => found.push((script, 1)),
        }
    }
    (letters, found)
}

pub(crate) fn scan(
    content: &str,
    expected: &[Script],
    escapes: &[std::ops::Range<usize>],
) -> Vec<Draft> {
    words(content, escapes)
        .into_iter()
        .flat_map(|(offset, word)| judge(offset, word, expected))
        .collect()
}

fn judge(offset: usize, word: &str, expected: &[Script]) -> Vec<Draft> {
    let scripts = scripts_in(word);
    // A declared script is an expected one, and mixing Latin with an
    // expected script is what a translation looks like. This is not a
    // softening: CJK is written without spaces, so `CSVストリーミング`
    // is one word by any segmentation, and a caller who has said the
    // tree contains Han would otherwise get a finding on every string
    // in it that mentions a product name. Measured, not guessed: on two
    // real locale trees that rule is the difference between 102
    // findings and none.
    let undeclared: Vec<Script> = scripts
        .iter()
        .copied()
        .filter(|script| *script != Script::Latin && !expected.contains(script))
        .collect();

    if !undeclared.is_empty() && !word.is_single_script() {
        return mixed(offset, word, &scripts, &undeclared);
    }
    // Nothing in this word is hiding as a neighbour. The one thing left
    // to ask is whether its Latin part is written in letters that only
    // *look* Latin.
    word.char_indices()
        .filter(|(_, character)| matches!(character.script(), Script::Latin | Script::Common))
        .filter_map(|(index, character)| compatibility_draft(offset + index, character))
        .collect()
}

/// Every script written in this word, excluding the three property
/// values that are not writing systems.
fn scripts_in(word: &str) -> Vec<Script> {
    let mut scripts: Vec<Script> = Vec::new();
    for script in word.chars().map(|character| character.script()) {
        if matches!(script, Script::Common | Script::Inherited | Script::Unknown) {
            continue;
        }
        if !scripts.contains(&script) {
            scripts.push(script);
        }
    }
    scripts.sort_unstable_by_key(|script| script.full_name());
    scripts
}

/// A word that no single script accounts for, and whose mixing is not
/// something the caller declared. That is the finding, and each
/// character from an undeclared script that resembles a character from
/// another is a second one.
fn mixed(offset: usize, word: &str, scripts: &[Script], undeclared: &[Script]) -> Vec<Draft> {
    let names: Vec<&str> = scripts.iter().map(|script| script.full_name()).collect();

    // Distinct, in the order they appear. A word is short and the list
    // is the reader's only view of what is in it, so a character that
    // occurs twice is listed once rather than padding the finding.
    let mut foreign: Vec<char> = Vec::new();
    for character in word.chars().filter(|c| !c.is_ascii()) {
        if !foreign.contains(&character) {
            foreign.push(character);
        }
    }

    let mut drafts = vec![Draft {
        offset,
        kind: Kind::MixedScript,
        severity: Severity::High,
        codepoints: codepoint::render_each(foreign),
        scripts: names.iter().map(|name| (*name).to_string()).collect(),
        resembles: Vec::new(),
        detail: format!(
            "one word written in {}: no single script accounts for it, which is how a name that \
             reads as familiar is forged",
            join_names(&names)
        ),
    }];

    drafts.extend(
        word.char_indices()
            .filter(|(_, character)| undeclared.contains(&character.script()))
            .filter_map(|(index, character)| homoglyph_draft(offset + index, character)),
    );
    drafts
}

/// A character inside a mixed word that UTS #39 knows is confusable
/// across scripts, and that really does reduce to something else.
fn homoglyph_draft(offset: usize, character: char) -> Option<Draft> {
    if character.is_ascii() || !is_potential_mixed_script_confusable_char(character) {
        return None;
    }
    let prototype = resembles(character)?;
    Some(Draft {
        offset,
        kind: Kind::Confusable,
        severity: Severity::High,
        codepoints: vec![codepoint::render(character)],
        scripts: vec![character.script().full_name().to_string()],
        resembles: codepoint::render_each(prototype),
        // Every character of this message is ASCII, deliberately. The
        // whole report is held to that (`detect::hazards`), and an em
        // dash typed into a detail string is the easiest way to break
        // an invariant nobody would think to check prose against.
        detail: format!(
            "a {script} character in a word that is not {script}, and it reduces to the codepoint \
             under `resembles`: the two are indistinguishable on screen",
            script = character.script().full_name()
        ),
    })
}

/// A letter or digit that is a compatibility form of an ASCII one:
/// full-width `ｆ`, mathematical `𝐟`, the Kelvin sign. Same script,
/// different codepoint, identical to a reader.
fn compatibility_draft(offset: usize, character: char) -> Option<Draft> {
    let plain = ascii_lookalike(character)?;
    Some(Draft {
        offset,
        kind: Kind::Confusable,
        severity: Severity::High,
        codepoints: vec![codepoint::render(character)],
        scripts: vec![character.script().full_name().to_string()],
        resembles: vec![codepoint::render(plain)],
        detail: "a compatibility form of an ASCII character: it reads as the codepoint under \
                 `resembles` and does not compare equal to it"
            .to_string(),
    })
}

fn ascii_lookalike(character: char) -> Option<char> {
    if character.is_ascii() || !(character.is_alphabetic() || character.is_numeric()) {
        return None;
    }
    if NOT_IMPERSONATING
        .iter()
        .any(|(first, last)| (*first..=*last).contains(&character))
    {
        return None;
    }
    let mut folded = std::iter::once(character).nfkc();
    let plain = folded.next()?;
    // Only a one-to-one fold. `ﬁ` becomes `fi` and `Ⅻ` becomes `XII`;
    // those are ligatures and numerals rather than one letter wearing
    // another's face.
    if folded.next().is_some() {
        return None;
    }
    (plain != character && plain.is_ascii_alphanumeric()).then_some(plain)
}

/// The UTS #39 skeleton of a single character, when it is not itself.
fn resembles(character: char) -> Option<Vec<char>> {
    let mut buffer = [0u8; 4];
    let prototype: Vec<char> = skeleton(character.encode_utf8(&mut buffer)).collect();
    (prototype != [character]).then_some(prototype)
}

/// Word boundaries, for the purpose of asking which script something is
/// written in.
///
/// Letters, digits, underscores and combining marks, which is an
/// identifier in every language this is likely to be pointed at, and a
/// word in ordinary prose. Not UAX #29 word segmentation: that splits
/// `snake_case` into three and would judge each fragment separately,
/// which is precisely the mixing this needs to see.
///
/// `escapes` are byte ranges that no word may be built across, supplied
/// by the format reader and empty for every format but JSON. That is
/// what resolves the crate's one documented false positive: in
/// `"Hello\nПривет"` the bytes really are a Latin `n` against Cyrillic,
/// and in a `.txt` file that is the honest reading, because a backslash
/// is an ordinary character there. In a JSON string the grammar says
/// `\n` is a line feed, so the `n` is not a letter and the word ends
/// before it. **Only a reader that knows the escape rule may claim
/// this**; nothing here guesses one.
fn words<'a>(content: &'a str, escapes: &[std::ops::Range<usize>]) -> Vec<(usize, &'a str)> {
    let mut words = Vec::new();
    let mut start: Option<usize> = None;
    for (offset, character) in content.char_indices() {
        if is_word_character(character) && !in_escape(escapes, offset) {
            let _ = start.get_or_insert(offset);
            continue;
        }
        if let Some(begin) = start.take() {
            words.push((begin, &content[begin..offset]));
        }
    }
    if let Some(begin) = start {
        words.push((begin, &content[begin..]));
    }
    words
}

/// Whether a byte offset falls inside an escape sequence. A binary
/// search: the ranges are non-overlapping and in document order, and a
/// linear scan per character would make the splitter quadratic on a
/// document full of escapes.
fn in_escape(escapes: &[std::ops::Range<usize>], offset: usize) -> bool {
    let after = escapes.partition_point(|span| span.start <= offset);
    after
        .checked_sub(1)
        .and_then(|index| escapes.get(index))
        .is_some_and(|span| offset < span.end)
}

fn is_word_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_' || character.script() == Script::Inherited
}

/// A script named by the caller. Full UAX #24 name or ISO 15924 tag,
/// case-insensitive: `Han`, `han`, `Hani`, `caucasian-albanian`.
pub(crate) fn parse_script(tag: &str) -> Result<Script, String> {
    let canonical = canonicalise(tag);
    let script = Script::from_full_name(&canonical)
        .or_else(|| Script::from_short_name(&canonical))
        .ok_or_else(|| {
            format!("{tag} is not a Unicode script; try a name like Han, Cyrillic or Devanagari")
        })?;
    // Naming these would parse and then do nothing, because they are not
    // writing systems and no character is refused on their account.
    if matches!(script, Script::Common | Script::Inherited | Script::Unknown) {
        return Err(format!(
            "{tag} is a Unicode script property value, not a writing system; name the script the \
             text is actually written in"
        ));
    }
    Ok(script)
}

fn canonicalise(tag: &str) -> String {
    tag.split(['-', '_'])
        .map(|segment| {
            let mut characters = segment.chars();
            match characters.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &characters.as_str().to_lowercase()
                }
            }
        })
        .collect::<Vec<String>>()
        .join("_")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(content: &str) -> Vec<Kind> {
        scan(content, &[], &[])
            .into_iter()
            .map(|draft| draft.kind)
            .collect()
    }

    fn confusables(content: &str) -> usize {
        kinds(content)
            .iter()
            .filter(|kind| **kind == Kind::Confusable)
            .count()
    }

    #[test]
    fn ordinary_latin_text_yields_nothing() {
        assert!(kinds("const paypal = require('paypal');").is_empty());
        assert!(kinds("café naïve Ærø Łódź İstanbul Đà Nẵng").is_empty());
        assert!(kinds("").is_empty());
    }

    /// **The false-positive guard, and the reason the tool is usable.**
    /// Every one of these is wholly one script, and a check that fired
    /// here would fire thousands of times on any localised repository.
    #[test]
    fn text_that_is_consistently_one_non_latin_script_yields_nothing() {
        for content in [
            "Привет мир",         // Russian
            "Καλημέρα κόσμε",     // Greek
            "你好世界",           // Chinese
            "こんにちは世界です", // Japanese: Han and Hiragana
            "안녕하세요 세계",    // Korean
            "مرحبا بالعالم",      // Arabic
            "שלום עולם",          // Hebrew
            "नमस्ते दुनिया",         // Devanagari
            "สวัสดีชาวโลก",         // Thai
        ] {
            assert!(kinds(content).is_empty(), "{content}");
        }
    }

    /// Japanese mixes Han, Hiragana and Katakana in one word all the
    /// time. UTS #39 augments the script set for exactly this, and
    /// without it every Japanese sentence is a mixed-script finding.
    #[test]
    fn japanese_and_korean_are_single_script_by_augmentation() {
        assert!(kinds("日本語テキストです").is_empty());
        assert!(kinds("한국어와漢字").is_empty());
    }

    #[test]
    fn a_cyrillic_letter_in_a_latin_word_is_found_twice() {
        // `pаypal` — the second character is U+0430, not U+0061.
        let found = scan("const p\u{430}ypal = 1;", &[], &[]);
        assert_eq!(
            found.iter().map(|d| d.kind).collect::<Vec<_>>(),
            [Kind::MixedScript, Kind::Confusable]
        );
        assert_eq!(found[0].scripts, ["Cyrillic", "Latin"]);
        assert_eq!(found[1].codepoints, ["U+0430"]);
        assert_eq!(found[1].resembles, ["U+0061"]);
        assert_eq!(found[1].scripts, ["Cyrillic"]);
        // Byte offset of the Cyrillic letter, not of the word.
        assert_eq!(found[1].offset, 7);
    }

    #[test]
    fn a_greek_letter_in_a_latin_word_is_found() {
        // `lοgin` — the second character is U+03BF.
        let found = scan("l\u{3bf}gin", &[], &[]);
        assert_eq!(found[0].kind, Kind::MixedScript);
        assert_eq!(found[1].resembles, ["U+006F"]);
    }

    #[test]
    fn a_full_width_letter_is_confusable_without_any_mixing() {
        // `ＦＩＬＥ` is Latin script throughout, so nothing is mixed.
        let found = scan("\u{FF26}\u{FF29}\u{FF2C}\u{FF25}", &[], &[]);
        assert_eq!(found.len(), 4);
        assert!(found.iter().all(|d| d.kind == Kind::Confusable));
        assert_eq!(found[0].codepoints, ["U+FF26"]);
        assert_eq!(found[0].resembles, ["U+0046"]);
    }

    #[test]
    fn a_mathematical_letter_is_confusable() {
        let found = scan("\u{1d41a}dmin", &[], &[]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, Kind::Confusable);
        assert_eq!(found[0].resembles, ["U+0061"]);
    }

    /// Ordinals and exponents fold to ASCII and are not impersonating
    /// it. This fires on every Spanish list and every unit of area.
    #[test]
    fn ordinals_and_exponents_are_not_confusable() {
        assert_eq!(confusables("1\u{00AA} 2\u{00BA} 5 m\u{00B2} H\u{2099}"), 0);
    }

    /// A ligature or a Roman numeral folds to more than one letter, and
    /// a character that reads as two characters is not wearing one
    /// character's face.
    #[test]
    fn a_multi_character_fold_is_not_confusable() {
        assert_eq!(ascii_lookalike('\u{FB01}'), None, "the fi ligature");
        assert_eq!(ascii_lookalike('\u{216B}'), None, "roman numeral twelve");
        assert_eq!(ascii_lookalike('\u{FF26}'), Some('F'));
        assert_eq!(ascii_lookalike('a'), None, "already ascii");
        assert_eq!(ascii_lookalike('\u{430}'), None, "not a compatibility form");
    }

    #[test]
    fn words_split_on_punctuation_and_keep_identifiers_whole() {
        assert_eq!(
            words("let snake_case_2 = f(x);", &[]),
            [(0, "let"), (4, "snake_case_2"), (19, "f"), (21, "x")]
        );
    }

    /// A combining mark belongs to the word it modifies. Splitting it
    /// off would leave a bare mark whose script set is Inherited and a
    /// base letter judged without it.
    #[test]
    fn a_combining_mark_stays_in_its_word() {
        assert_eq!(words("cafe\u{301} x", &[]), [(0, "cafe\u{301}"), (7, "x")]);
    }

    /// **The measured case.** CJK has no spaces, so a product name
    /// inside a translated string is one word with the text around it.
    /// Undeclared that is a finding and the file is refused anyway;
    /// declared it is a translation. Getting this wrong put 102 findings
    /// on two real locale trees the moment a caller took the way out the
    /// refusal offers them.
    #[test]
    fn latin_mixed_with_a_declared_script_is_a_translation() {
        let content = "CSVストリーミングの切り替え";
        assert_eq!(
            scan(content, &[], &[])
                .into_iter()
                .map(|draft| draft.kind)
                .collect::<Vec<_>>(),
            [Kind::MixedScript, Kind::Confusable]
        );
        assert!(
            scan(
                content,
                &[Script::Han, Script::Hiragana, Script::Katakana],
                &[]
            )
            .is_empty()
        );
    }

    /// And declaring one script is not declaring every script: the
    /// homoglyph in a Latin word is still found in the same file that
    /// legitimately contains Han.
    #[test]
    fn declaring_one_script_does_not_excuse_another() {
        let found = scan("設定 p\u{430}ypal", &[Script::Han], &[]);
        assert_eq!(
            found.iter().map(|draft| draft.kind).collect::<Vec<_>>(),
            [Kind::MixedScript, Kind::Confusable]
        );
        assert_eq!(found[1].codepoints, ["U+0430"]);
    }

    /// A compatibility form is not a script question, so declaring a
    /// script does not hide one.
    #[test]
    fn a_declared_script_does_not_hide_a_compatibility_form() {
        assert_eq!(confusables("\u{FF26}\u{FF29}\u{FF2C}\u{FF25}"), 4);
        assert_eq!(
            scan("\u{FF26}\u{FF29}\u{FF2C}\u{FF25}", &[Script::Han], &[]).len(),
            4
        );
    }

    /// Pinned because it looks like a bug and is not — **in a document
    /// whose format nobody told this crate**. A file holding
    /// `"Hello\nПривет"` contains the byte run `nПривет`, which is a
    /// Latin letter followed by Cyrillic with nothing between. The
    /// splitter has no language model on purpose: a backslash is an
    /// ordinary character in a `.txt`, where `C:\Привет` is a path, and
    /// guessing which of a file's backslashes are escapes is the
    /// guessing the rest of this crate refuses. See SPEC.md.
    #[test]
    fn an_escape_sequence_abutting_another_script_is_reported_as_written() {
        let found = kinds(r"Hello\nПривет");
        assert_eq!(found[0], Kind::MixedScript);
        assert!(found[1..].iter().all(|kind| *kind == Kind::Confusable));
        // A space between them, and there is nothing to report.
        assert!(kinds(r"Hello\n Привет").is_empty());
    }

    /// **And the case where it stops being guessing.** Inside a JSON
    /// string the grammar says `\n` is a line feed, so the `n` is not a
    /// letter and the two scripts never touch. The reader supplies the
    /// escape ranges; the splitter does not infer them.
    #[test]
    fn an_escape_sequence_in_a_json_string_is_not_word_material() {
        let content = r#"{"greeting":"Hello\nПривет"}"#;
        let escapes = super::super::locate::escape_spans(content, "json");
        assert_eq!(escapes.len(), 1, "the reader found no escape");
        assert!(
            scan(content, &[], &escapes).is_empty(),
            "the escape still glued two scripts together"
        );

        // The same bytes with no format declared are still reported, so
        // this is the reader's knowledge and not a weakened check.
        assert!(!scan(content, &[], &[]).is_empty());
    }

    /// A `\uXXXX` escape is six characters, and covering only the first
    /// two would leave `0041` as word material — gluing the text either
    /// side of it into one word rather than breaking it.
    #[test]
    fn a_unicode_escape_breaks_a_word_across_all_six_characters() {
        let content = r#"{"a":"Hello\u0041Привет"}"#;
        let escapes = super::super::locate::escape_spans(content, "json");
        assert!(scan(content, &[], &escapes).is_empty(), "{escapes:?}");
    }

    /// The splitter still sees everything either side of an escape. A
    /// homoglyph in the word *after* one is not excused by it.
    #[test]
    fn an_escape_does_not_hide_the_words_around_it() {
        let content = r#"{"a":"one\ntwo p\u{430}ypal"}"#.replace("\\u{430}", "\u{430}");
        let escapes = super::super::locate::escape_spans(&content, "json");
        assert_eq!(
            scan(&content, &[], &escapes)
                .into_iter()
                .map(|draft| draft.kind)
                .collect::<Vec<_>>(),
            [Kind::MixedScript, Kind::Confusable]
        );
    }

    #[test]
    fn a_latin_file_is_judged_rather_than_refused() {
        assert!(context("const total = 1;", &[]).is_none());
        assert!(context("café naïve", &[]).is_none());
        assert!(context("", &[]).is_none());
    }

    /// One quoted foreign word does not disable the check for the file —
    /// that would be the easiest evasion there is.
    #[test]
    fn a_single_foreign_word_does_not_refuse_the_file() {
        let mostly_english = format!("{} Привет", "the quick brown fox ".repeat(20));
        assert!(context(&mostly_english, &[]).is_none());
    }

    #[test]
    fn a_file_written_in_another_script_is_refused_by_name() {
        let refusal = context("ключ: значение\nдругой: текст", &[]).expect("a refusal");
        assert_eq!(refusal.reason, Reason::IntentionalScriptContext);
        assert!(refusal.detail.contains("Cyrillic"), "{}", refusal.detail);
        // The refusal has to say what still ran, or a reader takes it as
        // "this file was not scanned".
        assert!(refusal.detail.contains("Every other check did run"));
    }

    #[test]
    fn declaring_the_script_lifts_the_refusal() {
        let content = "ключ: значение\nдругой: текст";
        assert!(context(content, &[Script::Cyrillic]).is_none());
        assert!(
            context(content, &[Script::Han]).is_some(),
            "declaring a different script is not a declaration for this one"
        );
    }

    /// A Japanese document with a handful of Cyrillic letters in it, and
    /// Japanese declared.
    fn japanese_with_a_trace_of_cyrillic() -> String {
        format!("{} Привет", "これは日本語のテキストです。".repeat(8))
    }

    /// **The regression.** The share was counted over every non-Latin
    /// letter and then attributed to the undeclared scripts alone, so a
    /// file that is overwhelmingly Japanese and holds six Cyrillic
    /// letters was refused with "98% of this file's letters are
    /// Cyrillic". Three things were wrong and this pins the arithmetic:
    /// the number is the undeclared share, which is under the threshold.
    #[test]
    fn the_share_is_measured_over_the_undeclared_scripts_only() {
        let content = japanese_with_a_trace_of_cyrillic();
        assert!(
            context(&content, &[Script::Han, Script::Hiragana, Script::Katakana]).is_none(),
            "a file that is 6% undeclared was refused as though it were 98%"
        );

        // Undeclared, the same file is a Japanese document and is
        // refused — and the percentage names all three scripts, not one.
        let refusal = context(&content, &[]).expect("a refusal");
        let share: usize = refusal
            .detail
            .split('%')
            .next()
            .and_then(|number| number.parse().ok())
            .expect("a leading percentage");
        assert!(share > 90, "{}", refusal.detail);
        assert!(refusal.detail.contains("Cyrillic"), "{}", refusal.detail);
        assert!(refusal.detail.contains("Hiragana"), "{}", refusal.detail);
    }

    /// **The second half of the regression.** Declaring a script must
    /// mean the checks *run*: a homoglyph in a Latin word inside a
    /// Japanese file is exactly where one would hide, and the old
    /// refusal suppressed the check for the whole file the moment a
    /// caller declared the script their repository is written in.
    #[test]
    fn a_declared_file_is_judged_and_its_homoglyph_is_still_found() {
        let content = format!(
            "{} const p\u{430}ypal = 1;",
            "これは日本語のテキストです。".repeat(8)
        );
        let expected = [Script::Han, Script::Hiragana, Script::Katakana];
        assert!(context(&content, &expected).is_none(), "still refused");
        assert_eq!(
            scan(&content, &expected, &[])
                .into_iter()
                .map(|draft| draft.kind)
                .collect::<Vec<_>>(),
            [Kind::MixedScript, Kind::Confusable]
        );
    }

    /// A caller who declared a script and is refused anyway must not be
    /// told they declared nothing: it is false, and it sends them to
    /// check a flag they already passed.
    #[test]
    fn the_refusal_says_whether_a_script_was_declared() {
        let content = "ключ: значение\nдругой: текст";
        let undeclared = context(content, &[]).expect("a refusal");
        assert!(
            undeclared
                .detail
                .contains("no expected script was declared"),
            "{}",
            undeclared.detail
        );

        let declared = context(content, &[Script::Han]).expect("a refusal");
        assert!(
            !declared.detail.contains("no expected script was declared"),
            "a declared script was reported as no declaration: {}",
            declared.detail
        );
        assert!(
            declared.detail.contains("none of those"),
            "{}",
            declared.detail
        );
        assert!(declared.detail.is_ascii(), "{}", declared.detail);
    }

    /// Refusals speak the caller's vocabulary, and one of the two
    /// callers has no command line.
    #[test]
    fn the_refusal_names_no_flag() {
        let refusal = context("你好世界你好世界", &[]).expect("a refusal");
        assert!(!refusal.detail.contains("--"), "{}", refusal.detail);
    }

    #[test]
    fn a_script_is_named_by_full_name_or_tag_in_any_case() {
        for tag in ["Han", "han", "HAN", "Hani", "hani"] {
            assert_eq!(parse_script(tag).expect(tag), Script::Han);
        }
        assert_eq!(
            parse_script("cyrillic").expect("Cyrillic"),
            Script::Cyrillic
        );
        assert_eq!(
            parse_script("caucasian-albanian").expect("a script"),
            Script::Caucasian_Albanian
        );
    }

    #[test]
    fn an_unknown_script_is_refused_by_name() {
        let error = parse_script("Klingon").expect_err("a refusal");
        assert!(error.contains("Klingon"), "{error}");
    }

    /// These parse and then mean nothing, so accepting them would be a
    /// silent default: the caller believes they declared something.
    #[test]
    fn the_placeholder_script_values_are_refused() {
        for tag in ["Common", "Inherited", "Unknown"] {
            assert!(parse_script(tag).is_err(), "{tag}");
        }
    }

    /// The report-safety rule: nothing here quotes the text it found.
    #[test]
    fn no_draft_carries_the_text_it_reports() {
        for content in ["p\u{430}ypal", "\u{FF26}\u{FF29}", "\u{1d41a}dmin"] {
            let rendered = format!("{:?}", scan(content, &[], &[]));
            for character in content.chars().filter(|c| !c.is_ascii()) {
                assert!(!rendered.contains(character), "{rendered}");
            }
        }
    }
}
