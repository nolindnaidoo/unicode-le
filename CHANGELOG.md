# Changelog

This repository. The package that ships to crates.io keeps its own entry
in [`crate/CHANGELOG.md`](crate/CHANGELOG.md) — that one is what a
consumer reads, this one is what a reader of the repository needs.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

### Fixed

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

[0.1.0]: https://github.com/nolindnaidoo/unicode-le/releases/tag/crate-v0.1.0
