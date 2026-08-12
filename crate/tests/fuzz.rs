//! A standing net over the detection layer, fed text nobody would type.
//!
//! Time-boxed, not run to convergence: the point is that a panic, a
//! hang, or a slice off a character boundary has somewhere to be caught,
//! not that the input space is proved. Sixty seconds in CI, a second
//! locally.
//!
//! **The generator is aimed at this crate's specific ways of being
//! wrong**, which are not the usual parser hazards:
//!
//! - Every offset in a report is a byte offset into a document that is
//!   full of multi-byte characters by construction, and every column is
//!   a UTF-16 count over the same. Two indexing schemes over one string
//!   is where an off-by-one becomes a slice off a boundary, and the
//!   scanners here index by byte while the report counts code units.
//! - A grapheme cluster can be arbitrarily long, so "the sequence at the
//!   divergence" in the normalization check has no bound but the line.
//! - Word segmentation walks `char_indices` and slices `content[a..b]`.
//!   A word made entirely of combining marks, or one that runs to the
//!   end of the document, is where those bounds meet.
//! - Bidi controls in every position — first byte, last byte, inside a
//!   combining sequence, straddling a line break — because the *first*
//!   byte is a special case in `characters.rs` and special cases are
//!   where a scanner forgets a document can also be one character long.
//!
//! What is asserted on every case is what the tool promises: the answer
//! is well-formed JSON, every finding sits at a real character boundary
//! with a line and column that exist, and **nothing in the answer is
//! anything but ASCII**. That last one is the whole product: a report
//! that quoted what it found would carry the attack onward.
//!
//! The checked-in corpus is the seed set.

use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

const BINARY: &str = env!("CARGO_BIN_EXE_unicode-le");

/// Seconds of fuzzing. CI passes 60; a bare `cargo test` runs one, so
/// the net is present on every push without owning the run.
fn budget() -> Duration {
    let seconds = std::env::var("UNICODE_LE_FUZZ_SECONDS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(1);
    Duration::from_secs(seconds)
}

/// Printed on every run, failing or not: a fuzz failure nobody can
/// reproduce is a fuzz failure nobody fixes.
fn seed() -> u64 {
    std::env::var("UNICODE_LE_FUZZ_SEED")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(0x0c0d_e901_11ce_2026)
}

/// One case may not take longer than this. A scanner that went quadratic
/// on a pathological document would otherwise be a CI job that never
/// ends.
const CASE_LIMIT: Duration = Duration::from_secs(20);

/// The corpus, as seeds. The documents the crate already agrees about
/// are what everything else grows from.
const SEEDS: [(&str, &str); 8] = [
    (
        "trojan-source.c",
        include_str!("../fixtures/documents/trojan-source.c"),
    ),
    (
        "invisible.ts",
        include_str!("../fixtures/documents/invisible.ts"),
    ),
    (
        "homoglyph.js",
        include_str!("../fixtures/documents/homoglyph.js"),
    ),
    (
        "fullwidth.ts",
        include_str!("../fixtures/documents/fullwidth.ts"),
    ),
    (
        "mixed-script.py",
        include_str!("../fixtures/documents/mixed-script.py"),
    ),
    ("nfd.txt", include_str!("../fixtures/documents/nfd.txt")),
    ("ja.json", include_str!("../fixtures/documents/ja.json")),
    ("ru.json", include_str!("../fixtures/documents/ru.json")),
];

/// xorshift64*, four lines and identical on every platform. A fuzz run
/// needs to be reproducible, not statistically excellent.
struct Seeded(u64);

impl Seeded {
    fn next(&mut self) -> u64 {
        let mut state = self.0;
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        self.0 = state;
        state.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, limit: usize) -> usize {
        let limit = u64::try_from(limit).unwrap_or(1).max(1);
        usize::try_from(self.next() % limit).unwrap_or(0)
    }

    fn pick<'a, T>(&mut self, from: &'a [T]) -> &'a T {
        &from[self.below(from.len())]
    }
}

/// A server held open across the whole run: spawning a process per case
/// would measure `fork`, not the scanner.
struct Server {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<Option<String>>,
}

impl Server {
    fn start() -> Self {
        let mut child = Command::new(BINARY)
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the server starts");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");

        // Read on a thread so a case that never answers is a timeout
        // naming its body rather than a blocked test.
        let (sender, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if sender.send(line.ok()).is_err() {
                    return;
                }
            }
            let _ = sender.send(None);
        });

        Self {
            child,
            stdin,
            lines,
        }
    }

    fn call(&mut self, case: &str, arguments: &serde_json::Value) -> String {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "detect_unicode_risks", "arguments": arguments },
        });
        writeln!(self.stdin, "{request}")
            .unwrap_or_else(|error| panic!("{case}: the server stopped reading ({error})"));
        self.stdin
            .flush()
            .unwrap_or_else(|error| panic!("{case}: could not flush ({error})"));

        match self.lines.recv_timeout(CASE_LIMIT) {
            Ok(Some(line)) => line,
            Ok(None) => panic!("{case}: the server died — a panic, or a slice off a boundary"),
            Err(RecvTimeoutError::Timeout) => {
                panic!("{case}: no answer in {CASE_LIMIT:?} — the scanner is not terminating")
            }
            Err(RecvTimeoutError::Disconnected) => panic!("{case}: the server closed stdout"),
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Every bidirectional control, in the order the standard lists them.
const BIDI: [char; 10] = [
    '\u{061C}', '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2067}',
    '\u{2068}', '\u{2069}',
];

/// Characters that occupy a position and render as nothing.
const INVISIBLE: [char; 7] = [
    '\u{00AD}', '\u{180E}', '\u{200B}', '\u{200C}', '\u{200D}', '\u{2060}', '\u{FEFF}',
];

/// Combining marks, for building grapheme clusters nobody could read.
const COMBINING: [char; 6] = [
    '\u{0300}', '\u{0301}', '\u{0308}', '\u{0327}', '\u{05BF}', '\u{094D}',
];

/// One letter from each of the scripts the mixed-script rules turn on,
/// plus the compatibility forms the confusable check folds.
const LETTERS: [char; 14] = [
    'a',
    'Z',
    '\u{430}',
    '\u{3bf}',
    '\u{4e2d}',
    '\u{3042}',
    '\u{30a2}',
    '\u{d55c}',
    '\u{5d0}',
    '\u{627}',
    '\u{915}',
    '\u{e01}',
    '\u{FF26}',
    '\u{1d41a}',
];

/// Codepoints with no assigned meaning, and the private-use areas.
const UNASSIGNED: [char; 5] = [
    '\u{0378}',
    '\u{E000}',
    '\u{F8FF}',
    '\u{F0000}',
    '\u{10FFFD}',
];

/// Spaces that are not the space.
const SPACES: [char; 6] = [
    '\u{00A0}', '\u{1680}', '\u{2000}', '\u{2009}', '\u{202F}', '\u{3000}',
];

/// One fragment of hostile text. Every arm is a shape that reaches a
/// different scanner, and the mix of them is what makes a document
/// nobody would write by hand.
fn fragment(seeded: &mut Seeded) -> String {
    match seeded.below(14) {
        // A deeply nested run of directional embeddings, never popped.
        // The Trojan Source class, taken past what any renderer expects.
        0 => (0..=seeded.below(200))
            .map(|_| *seeded.pick(&BIDI))
            .collect(),
        // One base letter under an enormous grapheme cluster.
        1 => {
            let mut out = String::from(*seeded.pick(&LETTERS));
            for _ in 0..=seeded.below(400) {
                out.push(*seeded.pick(&COMBINING));
            }
            out
        }
        // A cluster with no base at all: combining marks are `Inherited`
        // and belong to the word before them, and there is none.
        2 => (0..=seeded.below(60))
            .map(|_| *seeded.pick(&COMBINING))
            .collect(),
        // Pathological alternation: a different script every character,
        // which is a word no single script accounts for and a UTS #39
        // resolution over the whole run.
        3 => (0..=seeded.below(300))
            .map(|_| *seeded.pick(&LETTERS))
            .collect(),
        // The same, glued into one identifier so word segmentation
        // cannot split it.
        4 => {
            let mut out = String::new();
            for _ in 0..=seeded.below(80) {
                out.push(*seeded.pick(&LETTERS));
                out.push('_');
            }
            out
        }
        5 => (0..=seeded.below(120))
            .map(|_| *seeded.pick(&INVISIBLE))
            .collect(),
        6 => (0..=seeded.below(40))
            .map(|_| *seeded.pick(&UNASSIGNED))
            .collect(),
        7 => (0..=seeded.below(60))
            .map(|_| *seeded.pick(&SPACES))
            .collect(),
        // Escape sequences abutting another script: the one known false
        // positive, written down in SPEC.md, generated on purpose so it
        // stays a finding rather than a crash.
        8 => format!(
            "\"Hello\\n{}\"",
            (0..=seeded.below(30))
                .map(|_| *seeded.pick(&LETTERS))
                .collect::<String>()
        ),
        // A decomposed line under a bidi control: the normalization
        // check and the character check reading the same bytes.
        9 => format!(
            "e{}{}e{}",
            *seeded.pick(&COMBINING),
            *seeded.pick(&BIDI),
            *seeded.pick(&COMBINING)
        ),
        // Ordinary source, so a document is not uniformly hostile.
        10 => format!(
            "const value{} = {};",
            seeded.below(1000),
            seeded.below(1000)
        ),
        // A byte-order mark somewhere other than the front, which is
        // where it stops being an encoding and becomes a finding.
        11 => format!("\u{feff}{}", seeded.below(1000)),
        // Astral characters: four bytes, one scalar, two UTF-16 code
        // units. Every column after one is where a count diverges.
        12 => (0..=seeded.below(50))
            .map(|_| char::from_u32(0x1_0000 + seeded.below(0x1000) as u32).unwrap_or('\u{1d41a}'))
            .collect(),
        _ => "\u{4e2d}\u{6587}".repeat(1 + seeded.below(100)),
    }
}

fn document(seeded: &mut Seeded) -> String {
    let mut out = String::new();
    // A leading byte-order mark on some documents: the one position
    // where U+FEFF is the encoding rather than a finding.
    if seeded.below(8) == 0 {
        out.push('\u{feff}');
    }
    let newline = if seeded.below(4) == 0 { "\r\n" } else { "\n" };
    for _ in 0..=seeded.below(12) {
        out.push_str(&fragment(seeded));
        if seeded.below(3) != 0 {
            out.push_str(newline);
        }
    }
    // Half the time, no trailing newline: the last line has no
    // terminator and every per-line check has to cope.
    if seeded.below(2) == 0 {
        while out.ends_with('\n') || out.ends_with('\r') {
            out.pop();
        }
    }
    out
}

/// The scripts a caller might declare, including none and including one
/// that has nothing to do with the document.
fn scripts(seeded: &mut Seeded) -> Vec<&'static str> {
    match seeded.below(6) {
        0 => vec!["Han", "Hiragana", "Katakana"],
        1 => vec!["Cyrillic"],
        2 => vec!["Han"],
        3 => vec!["Cyrillic", "Greek", "Han", "Hangul", "Hebrew", "Arabic"],
        4 => vec!["Devanagari"],
        _ => Vec::new(),
    }
}

/// What every answer must satisfy, whatever went in.
fn check(case: &str, document: &str, raw: &str) {
    let response: serde_json::Value = serde_json::from_str(raw)
        .unwrap_or_else(|error| panic!("{case}: the answer is not JSON ({error}) — {raw}"));
    assert!(
        response.get("error").is_none(),
        "{case}: the tool call failed — {raw}"
    );
    let envelope = &response["result"]["structuredContent"];
    assert_eq!(
        envelope["meta"]["tool"], "detect_unicode_risks",
        "{case}: not the tool's envelope — {raw}"
    );

    // **The product.** A report that carried what it found would be the
    // delivery mechanism for the thing it detects, so the whole answer
    // is ASCII or the run is a failure.
    assert!(
        raw.is_ascii(),
        "{case}: the answer is not ASCII, so something in the document reached it"
    );

    let data = &envelope["data"];
    let findings = data["findings"].as_array().expect("a finding list");
    // Counted once. Doing it per finding makes this checker quadratic on
    // exactly the documents it exists to prove the crate is not.
    let last_line = document.lines().count() + 1;
    let mut previous = 0usize;
    for finding in findings {
        let offset = finding["offset"].as_u64().expect("an offset") as usize;
        assert!(
            offset <= document.len(),
            "{case}: a finding at byte {offset} in a {}-byte document",
            document.len()
        );
        assert!(
            document.is_char_boundary(offset),
            "{case}: a finding at byte {offset}, which is inside a character"
        );
        // Document order, so two runs over an unchanged file produce
        // byte-identical reports.
        assert!(
            offset >= previous,
            "{case}: findings came back out of order at byte {offset}"
        );
        previous = offset;

        let line = finding["line"].as_u64().expect("a line");
        let column = finding["column"].as_u64().expect("a column");
        assert!(
            line >= 1 && column >= 1,
            "{case}: {line}:{column} is not 1-based"
        );
        assert!(
            line as usize <= last_line,
            "{case}: line {line} is past the end of the document"
        );

        // Every codepoint is `U+XXXX` and nothing else. This is the
        // representation, not a rendering of it.
        for codepoint in finding["codepoints"].as_array().expect("codepoints") {
            let text = codepoint.as_str().expect("a codepoint string");
            assert!(
                text.starts_with("U+") && text.len() >= 6,
                "{case}: {text:?} is not a codepoint"
            );
        }
    }

    assert!(
        data["refusals"].is_array(),
        "{case}: refusals is not a list — {raw}"
    );
    assert!(
        envelope["meta"]["truncated"].is_boolean(),
        "{case}: truncated is not a boolean — {raw}"
    );
}

#[test]
fn the_detector_survives_the_corpus_and_what_grows_out_of_it() {
    let seed = seed();
    let budget = budget();
    eprintln!("fuzz: seed {seed:#x}, budget {budget:?}");

    let mut server = Server::start();
    let mut seeded = Seeded(seed);
    let mut cases = 0usize;

    // The checked-in corpus first: the documents the crate already
    // agrees about are the seeds everything else grows from.
    for (name, content) in SEEDS {
        for declared in [
            vec![],
            vec!["Han", "Hiragana", "Katakana"],
            vec!["Cyrillic"],
        ] {
            let case = format!("seed corpus {name} {declared:?}");
            let raw = server.call(
                &case,
                &serde_json::json!({ "content": content, "scripts": declared }),
            );
            check(&case, content, &raw);
            cases += 1;
        }
    }

    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        let content = document(&mut seeded);
        let declared = scripts(&mut seeded);
        let case = format!("seed {seed:#x} case {cases}");
        let raw = server.call(
            &case,
            &serde_json::json!({ "content": content, "scripts": declared }),
        );
        check(&case, &content, &raw);
        cases += 1;
    }

    eprintln!("fuzz: {cases} documents, seed {seed:#x}");
    assert!(cases > SEEDS.len(), "the fuzzer ran no generated documents");
}

/// The pathological shapes by name, outside the random path, so a
/// failure says what broke rather than which seed found it.
#[test]
fn pathological_documents_terminate() {
    let mut server = Server::start();
    let cases: [(&str, String); 8] = [
        (
            "every bidi control, five thousand times",
            BIDI.iter().collect::<String>().repeat(5_000),
        ),
        (
            "one base letter under fifty thousand combining marks",
            format!("e{}", "\u{0301}".repeat(50_000)),
        ),
        (
            "combining marks with no base at all",
            "\u{0301}".repeat(50_000),
        ),
        (
            "a different script every character, in one word",
            LETTERS.iter().collect::<String>().repeat(2_000),
        ),
        ("one word the length of the document", "a".repeat(500_000)),
        (
            "a hundred thousand invisibles on one line",
            "\u{200B}".repeat(100_000),
        ),
        (
            "astral characters only, so every column is a surrogate pair",
            "\u{1d41a}".repeat(50_000),
        ),
        (
            "a bidi control as the entire document",
            "\u{202E}".to_string(),
        ),
    ];

    for (name, content) in cases {
        let started = Instant::now();
        let raw = server.call(name, &serde_json::json!({ "content": content }));
        check(name, &content, &raw);
        eprintln!("fuzz: {name} answered in {:?}", started.elapsed());
    }
}

/// A document made entirely of one-character lines, each holding a
/// finding. The per-line checks build a line index over the whole file
/// and the per-finding lookups walk it, so this is where the two meet.
#[test]
fn a_finding_on_every_line_of_a_long_document_terminates() {
    let mut server = Server::start();
    let mut content = String::new();
    for index in 0..50_000 {
        let _ = writeln!(content, "{}\u{202E}", index % 10);
    }
    let started = Instant::now();
    let raw = server.call(
        "a finding on every line",
        &serde_json::json!({
            "content": content,
            "maxResults": 5000,
        }),
    );
    check("a finding on every line", &content, &raw);
    eprintln!(
        "fuzz: a finding on every line answered in {:?}",
        started.elapsed()
    );
}
