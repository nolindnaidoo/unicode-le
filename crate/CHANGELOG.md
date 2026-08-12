# Changelog

The Rust CLI and MCP server.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-08-12

First release. The core: seven finding kinds, three refusals, both
surfaces, and a false-positive guard that was measured rather than
assumed.

### Added

- **Seven findings.** `bidi-control` (the Trojan Source class,
  CVE-2021-42574), `confusable`, `mixed-script`, `invisible`,
  `unassigned-or-private-use`, `non-nfc` and `unusual-whitespace`. Each
  carries a file, a 1-based line and UTF-16 column, a byte offset, the
  codepoints as `U+XXXX`, the scripts involved and a severity;
  confusables also carry what they reduce to.
- **The CLI**: one JSON report on stdout with `schema: 1` and no
  timestamp, a human summary on stderr, and exit codes — 0 clean, 1
  findings, 2 the question was malformed. `--kind`, `--script`,
  `--fail-on`, `--strict`, `--stdin`, `--hidden`, `--no-ignore`.
- **The MCP server** (`unicode-le mcp`) with `detect_unicode_risks`,
  which touches no filesystem, and `unicode_le_scan`, which returns the
  same report the CLI writes. A contract test drives both surfaces over
  one tree and asserts they answer identically.
- **Three refusals**, each a first-class result rather than an error:
  `binary_or_undecodable`, `encoding_unknown` and
  `intentional_script_context`.

### The shape of it

**Nothing it finds is ever quoted back.** A finding carries `U+XXXX` and
prose this crate wrote, never source text. A report that pasted a raw
U+202E would reorder the terminal, the diff and the pull request of
whoever read it — the tool would become the delivery mechanism for the
thing it detects. A test asserts a serialized report is pure ASCII, over
a document holding one of every hazard.

**It refuses to judge what it cannot judge honestly.** A file whose
letters are at least 10% an undeclared non-Latin script gets the
confusable and mixed-script checks withheld and a refusal saying so;
every other check still runs on it. The share is measured over the
**undeclared** scripts alone, so naming what a tree is written in turns
the checks on rather than off: a file that is 98% Japanese with Han,
Hiragana and Katakana declared is judged, and a Cyrillic `а` in a Latin
word inside it is still found. No encoding is guessed: a UTF-16
byte-order mark is named and refused, because decoding it as UTF-8 would
invent invisible and unassigned characters that are not in the file.

**It never rewrites anything.** No `--fix`, no normalization, no
stripping. The normalization form is reported; what to do about it needs
context this tool does not have.

**It stands on `unicode-security`, not on a copy of its tables.** UAX
#39 is a data standard; see SPEC.md, "Discovery: what already exists",
for what was checked and why `unicode-bidi`, `unicode-segmentation` and
`decancer` were not taken.

### Held to

Four hardening tiers beyond the unit and contract suites, each with its
own CI job and each naming the bug it exists to catch: `hazards` (a real
filesystem, built at runtime, every unexpressible case skipped **by
name**), `platform` (a second operating system, and the suite compared
line for line with `TZ` set and unset), `fuzz` (hostile text, time-boxed
and seeded) and `budget` (a wall-clock ceiling plus linearity in both
directions). A fifth, `coverage-matrix`, asserts every finding kind,
every severity and every refusal reason is reachable from a real fixture.

Three defects were found and fixed before this release rather than after
it, and each has a regression test whose failure was observed against the
unfixed code:

- The `intentional_script_context` share was counted over every non-Latin
  letter and attributed to the undeclared scripts alone, so declaring a
  script refused the file it was meant to unlock.
- The position index re-counted UTF-16 code units from the line start on
  every lookup, which is quadratic on a minified bundle: 20,000 findings
  on one line took 21.8s and now take 0.33s.
- Report paths carried the platform's separator rather than `/`.

### Verified

Run read-only against two real locale trees — `numbers-le/src/i18n` and
a 25-locale mobile app catalogue, 37 files across 25 languages including
Cyrillic, Greek, Han, Hiragana, Katakana and Hangul — with no script
declared and with every script declared. **Zero findings both ways**,
and the second run proves the checks ran rather than being suppressed.

Declaring a script initially produced 102 findings on those same trees,
every one of them a Latin product name inside a translated string: CJK
is written without spaces, so `CSVストリーミング` is one word. The rule
that a declared script is an *expected* script came from that run, and
`latin_mixed_with_a_declared_script_is_a_translation` pins it.

[0.1.0]: https://github.com/nolindnaidoo/unicode-le/releases/tag/crate-v0.1.0
