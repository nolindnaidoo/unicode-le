# Changelog

This repository. The package that ships to crates.io keeps its own entry
in [`crate/CHANGELOG.md`](crate/CHANGELOG.md) — that one is what a
consumer reads, this one is what a reader of the repository needs.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-08-14

What changed in the crate is in
[`crate/CHANGELOG.md`](crate/CHANGELOG.md). This section is the
repository around it.

### Added

- **A terminal demo** at [`assets/demo.gif`](assets/demo.gif), driving
  the real binary over the files in [`assets/demo/`](assets/demo/).
  [`assets/demo.tape`](assets/demo.tape) is the `vhs` script that
  produced it, so `cd assets && vhs demo.tape` reproduces the recording
  rather than leaving an artifact nobody can regenerate. Both sit above
  `crate/`, where `cargo package` cannot reach them.

### Changed

- **New icon artwork.** All sixteen tools were redrawn in one style, so
  the family reads as one set wherever the cards sit side by side. The
  framing is unchanged — the drawing fills 65.8% of an 800×800 canvas
  and every smaller size is derived from that one file rather than drawn
  again.

### Fixed

- **The README's images resolve away from GitHub.** They were repository
  paths, which crates.io and every other renderer resolves against its
  own origin, so the demo and the icon were broken everywhere this file
  is read that is not this repository. They are absolute URLs now.

## [0.1.0] - 2026-08-12

First release. A CLI and MCP server that scans a tree for the Unicode
characters that hide meaning, and never quotes one back at you.

**Not yet published to crates.io**, and there is no VS Code extension
beside it. When one lands, `crate/fixtures/` becomes the contract between
the two frontends the way it is in the sibling repos.

### Added

- **The crate** — seven finding kinds, three refusals, both surfaces, and
  a false-positive guard that was measured rather than assumed. See
  [`crate/CHANGELOG.md`](crate/CHANGELOG.md) for what it does, and
  [`crate/SPEC.md`](crate/SPEC.md) for the behavioural contract.
- **Repository documentation** — [README.md](README.md) as the landing
  page, [AGENTS.md](AGENTS.md) as the engineering standard above
  `crate/AGENTS.md`, and this changelog. The known limitations are
  written down in AGENTS.md rather than discovered.
- **Five hardening test tiers**, each because something real got through
  a green suite, and each naming the bug it would have caught:
  - `hazards` — a real filesystem: a byte-order mark, a non-UTF-8 file, a
    UTF-16 catalogue, a FIFO, a permission-denied file, a path over 260
    characters, a symlink loop, an empty file, and a minified bundle that
    is one line. Built at runtime, because Windows cannot hold half of it
    in git; every case a platform cannot express skips **by name**.
  - `platform` — separators, case folding, the Windows device names, CRLF
    against a lone CR, stdin closing early, and the suite compared line
    for line with `TZ` set and unset.
  - `fuzz` — hostile text, time-boxed and seeded, aimed at this crate's
    own ways of being wrong: two indexing schemes over one string,
    unbounded grapheme clusters, words of pure combining marks, and bidi
    controls in the one position that is a special case.
  - `budget` — a wall-clock ceiling on a seeded corpus, plus linearity in
    both directions.
  - `coverage-matrix` — every `kind`, `severity` and `reason` reachable
    from a real fixture, with a marker line CI greps for, because
    `cargo test <filter>` exits 0 when the filter matches nothing.

- **Key paths.** A finding now carries `key` — the document's own name
  for where it sits, so a bidi control in a five-thousand-line locale
  catalogue comes back at `metrics.headline.eyebrow` rather than at a
  line number somebody has to go and look up. JSON, YAML, TOML, INI,
  `.env` and CSV resolve one, from the file's own extension; the MCP
  document tool takes `format` or `filename`, since a document handed to
  it has neither.

  **The format decides how a finding is addressed and never whether it
  exists** — the inversion the layer rests on, and the opposite of what
  a format-aware extractor does. Every scanner runs over the same raw
  text whatever the format, so a document nothing can parse is still
  scanned and still reports everything in it, without a key. The readers
  are line scanners rather than parsers, so the offsets stay the raw
  document's and a truncated document still yields what it did read.
  `coverage-matrix` now asserts every reader is reachable from a real
  fixture and comes back carrying a key path — which earns its place,
  because a lost key path costs no finding and a reader that quietly
  stopped naming things would pass every other test in the suite.

### Fixed

- **A hostile file name no longer reaches the reader raw.** The
  report-safety rule covered everything the scan *found* and stopped at
  the one string the crate does not author: `file` is the caller's own
  path, echoed back so it can be opened and grepped for, and that
  exemption is what made it exploitable. A repository holding a file
  named with a right-to-left override produced a report that reordered
  the terminal, the diff and the pull request of whoever read it.

  Every non-ASCII codepoint in a path — and in a `key`, which is text
  out of the document and carries the same hazard — is now emitted as
  `\uXXXX`, on both streams and both surfaces. Because that is JSON's
  own escape, both properties hold at once: the raw bytes carry nothing
  that can render, and a parser decodes the field back to the identical
  string, so the path still opens the file it names. The escape is
  applied to the serialized document, never to the field — escaping
  first does not survive `serde_json`, which escapes the backslash and
  ships `\\u202E`.
- **The escape-sequence false positive is resolved for JSON.** Inside a
  JSON string, `\n` is a line feed and not the letter `n`, so
  `"Hello\nПривет"` is no longer reported as a word that mixes scripts.
  The JSON reader hands the word splitter the byte ranges of the escapes
  it found; the splitter still infers nothing. In every other format it
  is still reported, because there the bytes really are a Latin letter
  followed by Cyrillic and a backslash is an ordinary character where
  `C:\Привет` is a path.
- **Declaring a script no longer disables the checks it was meant to
  enable.** The `intentional_script_context` share was counted over
  *every* non-Latin letter and then attributed to the undeclared scripts
  alone. A file that was overwhelmingly Japanese and held six Cyrillic
  letters was refused with "98% of this file's letters are Cyrillic" —
  the wrong number (the Cyrillic share is 5.8%), the wrong clause ("no
  expected script was declared" when one was), and the wrong outcome:
  refused *despite* the declaration. The consequence was the serious
  part. Declaring the script a repository is written in switched the
  homoglyph and mixed-script checks off for every file in it, which is
  exactly where a homoglyph would be hiding. The share is now measured
  over the undeclared scripts only, the clause is conditional, and a
  declared file is judged — a Cyrillic `а` in a Latin word inside a
  declared-Japanese file is found.
- **The script share cannot overflow.** It multiplied two `usize` counts,
  which wraps on a 32-bit target at about 43 million letters — a 43 MB
  file of text, well inside what a repository holds. `overflow-checks =
  true` made that a panic rather than a wrong number, which is the right
  failure and still a failure: this tool answers or refuses by name, and
  aborting does neither. Widened to `u128`, and the decision and the
  percentage the refusal prints are one computation now, so the message
  cannot read below the threshold it refused at.
- **A compatibility form inside a mixed word is reported.** `judge`
  answered two questions as one either/or: a word that mixed an
  undeclared script with Latin returned its `mixed-script` finding and
  never reached the compatibility check below it. So `設ＦＩＬＥ`
  answered nothing under `--kind confusable` and four findings under
  `--kind confusable --script Han` — **declaring a script added
  findings**, which is the opposite of what declaring is for and the
  mirror of the bug above, in the same function.

  A compatibility form is not a script question: a full-width `Ｆ` reads
  as `F` and does not compare equal to it whatever the file around it is
  written in. The two questions are answered independently now, and the
  half that is not about scripts **does not take the declared scripts as
  a parameter** — so a third change cannot gate it by accident without
  adding an argument that has no business being there.
  `declaring_a_script_never_adds_a_finding` states the property both
  bugs broke, in opposite directions.
- **The position index no longer re-counts UTF-16 code units from the
  line start on every lookup.** Invisible on a config file and quadratic
  on the shape this tool is pointed at most: a minified bundle is one
  line holding the whole document, with one column lookup per finding on
  it. Measured on a debug build before the fix — 5,000 findings 1.4s,
  10,000 5.5s, 20,000 21.8s, 40,000 never finished. After: 20,000 in
  0.33s and 80,000 in 1.37s. `ips-le` found the shape first.
- **Report paths are separated by `/` on every platform.** A sibling
  shipped `\` on Windows for a whole release, which made every path in a
  Windows report differ from the same path in a Linux one for no reason a
  reader could see. `scan::report_path` is the one place that decides it.

[0.1.0]: https://crates.io/crates/unicode-le/0.1.0
[0.1.1]: https://crates.io/crates/unicode-le/0.1.1
