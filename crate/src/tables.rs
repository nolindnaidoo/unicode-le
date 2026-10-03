//! The Unicode data the extension answers from, rendered from this crate.
//!
//! Every finding here rests on Unicode properties this crate takes from
//! its dependencies and its toolchain: the script of a character and its
//! script extensions (`unicode-script`), the UTS #39 confusable prototypes
//! (`unicode-security`), canonical decomposition and composition
//! (`unicode-normalization`), and what counts as a letter, a digit or
//! whitespace (`char`, from the pinned Rust toolchain). The extension
//! runs in whatever JavaScript engine the editor or Node happens to ship,
//! and those carry their own Unicode version — so the two frontends would
//! disagree on exactly the newest characters, which is where an attacker
//! would look.
//!
//! So the extension does not ask its engine. It reads
//! `fixtures/unicode-tables.json`, which is this crate's answers written
//! down, and this test fails the moment the file and the crate disagree:
//! a dependency or toolchain bump that moves one character has to move
//! the file in the same change. Rewrite it with
//! `UNICODE_LE_WRITE_TABLES=1 cargo test --locked tables` and read the
//! diff.
//!
//! Ranges are written as deltas so the file stays small enough to bundle:
//! a range table is `[delta to start, value, delta to start, value, ...]`
//! covering every codepoint, and a set is `[delta to start, length, ...]`.

use std::fmt::Write as _;

use unicode_normalization::char::{canonical_combining_class, compose, decompose_canonical};
use unicode_script::{Script, ScriptExtension, UnicodeScript};
use unicode_security::is_potential_mixed_script_confusable_char;

use crate::detect::scripts::{ascii_lookalike, resembles};

const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/unicode-tables.json");

fn codepoints() -> impl Iterator<Item = char> {
    (0..=0x0010_FFFF_u32).filter_map(char::from_u32)
}

fn is_hangul(c: char) -> bool {
    matches!(c as u32, 0x1100..=0x11FF | 0xAC00..=0xD7A3)
}

/// `[delta, value, ...]` over every codepoint, one entry per run.
fn runs(mut value: impl FnMut(char) -> u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut previous_start = 0;
    let mut current: Option<u32> = None;
    for c in codepoints() {
        let v = value(c);
        if current != Some(v) {
            out.push(c as u32 - previous_start);
            out.push(v);
            previous_start = c as u32;
            current = Some(v);
        }
    }
    out
}

/// `[delta to start, length, ...]` for the codepoints where `test` holds.
fn set(test: impl Fn(char) -> bool) -> Vec<u32> {
    let mut out = Vec::new();
    let mut previous_start = 0;
    let mut start: Option<u32> = None;
    let mut last = 0;
    for c in codepoints() {
        let code = c as u32;
        let inside = test(c);
        match (inside, start) {
            (true, None) => {
                start = Some(code);
                last = code;
            }
            // A surrogate gap is not a break: no string can hold one.
            (true, Some(_)) if code == last + 1 || (last == 0xD7FF && code == 0xE000) => {
                last = code;
            }
            (true, Some(begin)) => {
                out.extend([begin - previous_start, last - begin + 1]);
                previous_start = begin;
                start = Some(code);
                last = code;
            }
            (false, Some(begin)) => {
                out.extend([begin - previous_start, last - begin + 1]);
                previous_start = begin;
                start = None;
            }
            (false, None) => {}
        }
    }
    if let Some(begin) = start {
        out.extend([begin - previous_start, last - begin + 1]);
    }
    out
}

/// `[delta to key, count, values...]` for a sparse map, keys ascending.
fn map(entries: impl Iterator<Item = (char, Vec<char>)>) -> Vec<u32> {
    let mut out = Vec::new();
    let mut previous = 0;
    for (key, values) in entries {
        out.push(key as u32 - previous);
        previous = key as u32;
        out.push(u32::try_from(values.len()).expect("a short mapping"));
        out.extend(values.into_iter().map(|c| c as u32));
    }
    out
}

fn numbers(values: &[u32]) -> String {
    let mut out = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        write!(out, "{value}").expect("writing to a string");
    }
    out.push(']');
    out
}

/// Every script any codepoint carries, as a value or in an extension,
/// in the order the names sort — so the file does not depend on the order
/// of an enum in a dependency.
fn all_scripts() -> Vec<Script> {
    let mut scripts: Vec<Script> = Vec::new();
    for c in codepoints() {
        for script in std::iter::once(c.script()).chain(c.script_extension().iter()) {
            if !scripts.contains(&script) {
                scripts.push(script);
            }
        }
    }
    if !scripts.contains(&Script::Unknown) {
        scripts.push(Script::Unknown);
    }
    scripts.sort_unstable_by_key(|script| script.full_name());
    scripts
}

fn index_in(scripts: &[Script], script: Script) -> u32 {
    u32::try_from(scripts.iter().position(|s| *s == script).expect("listed")).expect("few")
}

/// The distinct script extensions, each a list of script indices or one
/// of the two property values that stand for every script, and the runs
/// that assign one to every codepoint.
fn extensions(scripts: &[Script]) -> (Vec<String>, Vec<u32>) {
    let mut distinct: Vec<ScriptExtension> = Vec::new();
    let assigned = runs(|c| {
        let ext = c.script_extension();
        let at = distinct.iter().position(|e| *e == ext).unwrap_or_else(|| {
            distinct.push(ext);
            distinct.len() - 1
        });
        u32::try_from(at).expect("few")
    });
    let rendered = distinct
        .iter()
        .map(|ext| {
            if ext.is_common() {
                "\"Common\"".to_string()
            } else if ext.is_inherited() {
                "\"Inherited\"".to_string()
            } else {
                numbers(
                    &ext.iter()
                        .map(|script| index_in(scripts, script))
                        .collect::<Vec<u32>>(),
                )
            }
        })
        .collect();
    (rendered, assigned)
}

fn full_decomposition(c: char) -> Vec<char> {
    let mut full = Vec::new();
    decompose_canonical(c, |d| full.push(d));
    full
}

/// Every composition the canonical algorithm can perform, as `[first,
/// second, composed, ...]`. The first half is a character that decomposes
/// or starts a decomposition; the second is one that appears after the
/// first in a decomposition. Hangul is algorithmic and left out.
fn compositions() -> Vec<u32> {
    let mut firsts: Vec<char> = Vec::new();
    let mut seconds: Vec<char> = Vec::new();
    for c in codepoints().filter(|c| !is_hangul(*c)) {
        let full = full_decomposition(c);
        if full == [c] {
            continue;
        }
        firsts.push(c);
        firsts.push(full[0]);
        seconds.extend(full[1..].iter().copied());
    }
    firsts.sort_unstable();
    firsts.dedup();
    seconds.sort_unstable();
    seconds.dedup();
    let mut out = Vec::new();
    for first in &firsts {
        for second in &seconds {
            if let Some(composed) = compose(*first, *second) {
                out.extend([*first as u32, *second as u32, composed as u32]);
            }
        }
    }
    out
}

fn version(
    (major, minor, patch): (
        impl std::fmt::Display,
        impl std::fmt::Display,
        impl std::fmt::Display,
    ),
) -> String {
    format!("\"{major}.{minor}.{patch}\"")
}

fn render() -> String {
    let scripts = all_scripts();
    let (extension_list, extension_runs) = extensions(&scripts);
    let decompositions = codepoints()
        .filter(|c| !is_hangul(*c))
        .map(|c| (c, full_decomposition(c)))
        .filter(|(c, full)| full != &[*c]);
    let homoglyphs = codepoints().filter_map(|c| {
        if c.is_ascii() || !is_potential_mixed_script_confusable_char(c) {
            return None;
        }
        resembles(c).map(|prototype| (c, prototype))
    });
    let lookalikes = codepoints().filter_map(|c| ascii_lookalike(c).map(|plain| (c, vec![plain])));
    let names: Vec<String> = scripts
        .iter()
        .map(|s| format!("[\"{}\",\"{}\"]", s.full_name(), s.short_name()))
        .collect();

    let fields = [
        ("schema", "1".to_string()),
        (
            "unicode",
            format!(
                "{{\"script\":{},\"security\":{},\"normalization\":{},\"char\":{}}}",
                version(unicode_script::UNICODE_VERSION),
                version(unicode_security::UNICODE_VERSION),
                version(unicode_normalization::UNICODE_VERSION),
                version(char::UNICODE_VERSION),
            ),
        ),
        ("scripts", format!("[{}]", names.join(","))),
        ("script", numbers(&runs(|c| index_in(&scripts, c.script())))),
        ("extensions", format!("[{}]", extension_list.join(","))),
        ("extension", numbers(&extension_runs)),
        ("alphabetic", numbers(&set(char::is_alphabetic))),
        ("numeric", numbers(&set(char::is_numeric))),
        ("whitespace", numbers(&set(char::is_whitespace))),
        (
            "combining",
            numbers(&runs(|c| u32::from(canonical_combining_class(c)))),
        ),
        ("decomposition", numbers(&map(decompositions))),
        ("composition", numbers(&compositions())),
        ("homoglyph", numbers(&map(homoglyphs))),
        ("lookalike", numbers(&map(lookalikes))),
    ];
    let body: Vec<String> = fields
        .iter()
        .map(|(name, value)| format!("\"{name}\":{value}"))
        .collect();
    format!("{{\n{}\n}}\n", body.join(",\n"))
}

/// **The extension answers from this file, so it must be this crate's
/// answers.** A dependency or toolchain bump that moves a single character
/// fails here, and the fix is to regenerate and read the diff — never to
/// edit the file by hand.
#[test]
fn the_shared_unicode_tables_are_this_crates() {
    let rendered = render();
    if std::env::var_os("UNICODE_LE_WRITE_TABLES").is_some() {
        std::fs::write(PATH, &rendered).expect("the tables are writable");
        println!("wrote {PATH}");
        return;
    }
    let committed = std::fs::read_to_string(PATH).unwrap_or_default();
    assert!(
        committed == rendered,
        "fixtures/unicode-tables.json no longer matches what this crate computes. Regenerate it \
         with `UNICODE_LE_WRITE_TABLES=1 cargo test --locked tables` and read the diff: the \
         extension answers from that file."
    );
}
