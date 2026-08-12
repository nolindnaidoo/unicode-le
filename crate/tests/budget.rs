//! A wall-clock ceiling, and two shape checks on how the clock moves.
//!
//! A sibling crate was fifty times slower than the rest of the family
//! for a whole release and nothing noticed, because nothing measured it.
//! The ceiling here is deliberately loose — a shared runner is not a
//! benchmark rig — and exists to catch an order of magnitude, not a
//! percent.
//!
//! Two linearity assertions, because a tree grows in two directions and
//! only one of them is the obvious one:
//!
//! - **four times the documents** must not cost six times the clock;
//! - **four times the content in one document** must not either. That is
//!   the one that catches a quadratic, and this crate had one.
//!
//! `ips-le` found the shape first: a UTF-16 column index that re-counts
//! code units from the line start on every lookup. It is invisible on a
//! config file and fatal on the shape this tool is pointed at most — a
//! minified bundle, which is **one line** holding the whole document,
//! with one column lookup per finding on it. Measured here before the
//! fix, on a debug build: 5,000 findings on such a line took 1.4s,
//! 10,000 took 5.5s, 20,000 took 21.8s, and 40,000 never finished. Two
//! times the work, four times the clock, which is the shape of a square.
//! `detect/position.rs` now carries checkpoints, and
//! `four_times_the_content_in_one_document_is_not_six_times_the_clock`
//! is what keeps them.
//!
//! The corpus is **non-ASCII by construction**. An ASCII document needs
//! no code-unit counting at all and takes a path where the bug cannot
//! exist, so an ASCII corpus would have measured nothing.
//!
//! Gated behind `UNICODE_LE_BUDGET` and run by CI on one platform with
//! `--test-threads=1`; a timing assertion measured against other tests
//! on the same cores is noise. A skipped run says so by name.

use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};

const BINARY: &str = env!("CARGO_BIN_EXE_unicode-le");

/// The corpus: 300 documents, generated from a fixed seed rather than
/// checked in — 300 near-identical files in git would be 300 a reviewer
/// has to ignore — and from a **fixed** seed, so two runs measure the
/// same corpus.
const SEED: u64 = 0x0c0d_e901_11ce_2026;
const DOCUMENTS: usize = 300;

/// Fragments per document at the base size. Each one is a line holding
/// roughly one finding.
const FRAGMENTS: usize = 40;

/// **10× the local measurement**, recorded with the machine it came
/// from: 0.265 s for the 300-document corpus on an Apple M-series
/// laptop, debug build, 2026-08. Ten times that leaves a shared runner
/// room to be several times slower and still be right; it does not leave
/// room for an order of magnitude, which is the thing worth catching.
const BUDGET: Duration = Duration::from_millis(2_650);

/// Four times the work, at most six times the clock.
const LINEARITY: f64 = 6.0;

fn enabled(name: &str) -> bool {
    if std::env::var_os("UNICODE_LE_BUDGET").is_some() {
        return true;
    }
    eprintln!("SKIPPED {name}: set UNICODE_LE_BUDGET to run it");
    false
}

/// xorshift64*, four lines and identical on every platform.
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
}

struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl Server {
    fn start() -> Self {
        let mut child = Command::new(BINARY)
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the server starts");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn detect(&mut self, content: &str) {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "detect_unicode_risks",
                // Capped low on purpose: the measurement is the scan,
                // not the cost of serializing a report nobody reads.
                "arguments": { "content": content, "maxResults": 1 },
            },
        });
        writeln!(self.stdin, "{request}").expect("the server reads");
        self.stdin.flush().expect("flushed");
        let mut line = String::new();
        let read = self
            .stdout
            .read_line(&mut line)
            .expect("the server answers");
        assert!(read > 0, "the server closed stdout mid-run");
        assert!(
            line.contains("\"detect_unicode_risks\""),
            "the measured call failed: {line}"
        );
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One fragment of roughly the shape a real source file holds: accented
/// Latin so the code-unit count has work to do, an occasional finding,
/// and identifiers for the word splitter to walk.
fn fragment(seeded: &mut Seeded, index: usize) -> String {
    match index % 5 {
        0 => format!(
            "const caf\u{e9}Value{} = {};",
            seeded.below(1000),
            seeded.below(1000)
        ),
        1 => format!(
            "// r\u{e9}sum\u{e9} note {}\u{00A0}here",
            seeded.below(1000)
        ),
        2 => format!(
            "const label{} = \"na\u{ef}ve\u{200B}text\";",
            seeded.below(1000)
        ),
        3 => format!("const \u{4e2d}\u{6587}{} = 1;", seeded.below(1000)),
        _ => format!(
            "export function handler{}() {{ return 1; }}",
            seeded.below(1000)
        ),
    }
}

/// A document of `fragments` lines.
fn document(seeded: &mut Seeded, fragments: usize) -> String {
    let mut out = String::new();
    for index in 0..fragments {
        let _ = writeln!(out, "{}", fragment(seeded, index));
    }
    out
}

/// The same content as `document`, on **one line**. This is the shape
/// the quadratic lived in: a minified bundle, where every column lookup
/// counts from the same line start.
fn one_line(seeded: &mut Seeded, fragments: usize) -> String {
    let mut out = String::new();
    for index in 0..fragments {
        let _ = write!(out, "{} ", fragment(seeded, index));
    }
    out
}

/// The fastest of three runs. The fastest, not the mean: a shared runner
/// pauses for reasons that have nothing to do with this code, and the
/// question is what the work costs, not what the neighbours cost.
fn fastest(corpus: &[String]) -> Duration {
    let mut best = Duration::MAX;
    for _ in 0..3 {
        let mut server = Server::start();
        let started = Instant::now();
        for content in corpus {
            server.detect(content);
        }
        best = best.min(started.elapsed());
    }
    best
}

fn corpus(documents: usize, fragments: usize, single_line: bool) -> Vec<String> {
    let mut seeded = Seeded(SEED);
    (0..documents)
        .map(|_| {
            if single_line {
                return one_line(&mut seeded, fragments);
            }
            document(&mut seeded, fragments)
        })
        .collect()
}

#[test]
fn three_hundred_documents_scan_inside_their_budget() {
    if !enabled("three_hundred_documents_scan_inside_their_budget") {
        return;
    }
    let elapsed = fastest(&corpus(DOCUMENTS, FRAGMENTS, false));
    eprintln!("budget: {DOCUMENTS} documents in {elapsed:?} (ceiling {BUDGET:?}, seed {SEED:#x})");
    assert!(
        elapsed <= BUDGET,
        "{DOCUMENTS} documents took {elapsed:?}, over the {BUDGET:?} ceiling (seed {SEED:#x})"
    );
}

#[test]
fn four_times_the_documents_is_not_six_times_the_clock() {
    if !enabled("four_times_the_documents_is_not_six_times_the_clock") {
        return;
    }
    let one = fastest(&corpus(DOCUMENTS, FRAGMENTS, false));
    let four = fastest(&corpus(DOCUMENTS * 4, FRAGMENTS, false));
    let ratio = four.as_secs_f64() / one.as_secs_f64().max(f64::EPSILON);
    eprintln!(
        "budget: {one:?} for {DOCUMENTS}, {four:?} for {}, ratio {ratio:.2}",
        DOCUMENTS * 4
    );
    assert!(
        ratio <= LINEARITY,
        "four times the corpus cost {ratio:.2}x the time (limit {LINEARITY}x), seed {SEED:#x}"
    );
}

/// **The one that catches a quadratic**, and the one this crate needed.
/// Four times the content in a single document, on a single non-ASCII
/// line, with a finding every few characters — so the column lookup runs
/// four times as often over a line four times as long. A per-lookup scan
/// from the line start shows up here at sixteen times the clock and
/// nowhere else.
#[test]
fn four_times_the_content_in_one_document_is_not_six_times_the_clock() {
    if !enabled("four_times_the_content_in_one_document_is_not_six_times_the_clock") {
        return;
    }
    let one = fastest(&corpus(20, 500, true));
    let four = fastest(&corpus(20, 2_000, true));
    let ratio = four.as_secs_f64() / one.as_secs_f64().max(f64::EPSILON);
    eprintln!("budget: {one:?} for 500 fragments on one line, {four:?} for 2000, ratio {ratio:.2}");
    assert!(
        ratio <= LINEARITY,
        "four times the content on one line cost {ratio:.2}x the time (limit {LINEARITY}x), \
         seed {SEED:#x} — the position index is counting from the line start again"
    );
}

/// The same axis over many lines rather than one, so a regression can be
/// told apart: this one stays linear even with the quadratic present,
/// and the single-line case above does not.
#[test]
fn four_times_the_lines_in_one_document_is_not_six_times_the_clock() {
    if !enabled("four_times_the_lines_in_one_document_is_not_six_times_the_clock") {
        return;
    }
    let one = fastest(&corpus(20, 500, false));
    let four = fastest(&corpus(20, 2_000, false));
    let ratio = four.as_secs_f64() / one.as_secs_f64().max(f64::EPSILON);
    eprintln!("budget: {one:?} for 500 lines, {four:?} for 2000, ratio {ratio:.2}");
    assert!(
        ratio <= LINEARITY,
        "four times the lines cost {ratio:.2}x the time (limit {LINEARITY}x), seed {SEED:#x}"
    );
}
