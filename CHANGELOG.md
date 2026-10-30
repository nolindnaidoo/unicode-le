# Changelog

All notable changes to Unicode-LE will be documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file covers the **VS Code extension**, the npm MCP server and the
npm command line. The Rust CLI in `crate/` keeps its own
[CHANGELOG](crate/CHANGELOG.md). From 1.2.0 every surface carries one
version and releases from one tag. The entries below 1.0.0 describe this
repository while it held the CLI alone.

## [Unreleased]

### Fixed

- The extension's MCP server, the one `unicode-le-mcp` and the editor run, died
  on an unhandled EPIPE with a Node stack trace when the client closed the
  pipe, which a host logs as the server crashing. It exits 2 quietly at the
  first reply it cannot write, as the crate's server and the npm command
  line do.
- The npm command line's MCP server dropped a frame that was not JSON, not
  UTF-8, not a JSON-RPC 2.0 request or led by a byte order mark without a
  word, where the extension's server answers a parse error; it answered a
  request with a null id, which the extension's server reads as a
  notification; it refused an unknown method, an unknown tool and a call
  naming no tool in its own words; and it held a frame until its newline
  with no ceiling, so a client that streamed without one grew the process
  without bound. It answers every malformed frame in the extension server's
  words now, refuses a frame over 16 MiB and serves the next line, reads a
  last line with no newline as the partial frame it is, joins the chunks of
  a line once at its newline rather than on every chunk, and waits for the
  pipe to drain before reading the next frame.
- The extension's MCP server answered `initialize` with whatever protocol
  version the client asked for and a `listChanged: false` capability, where
  the crate's server answers its own version and `tools: {}`; it replied to
  an `initialize` with no id; it answered `notifications/initialized` sent
  with an id with nothing, where the crate's server refuses an unknown
  method; and a frame led by a byte order mark was read as a request here
  and refused as not JSON there. The three servers answer one handshake and
  one malformed frame alike now. The server also stops reading while the
  pipe to a slow client is full, so what it holds is bounded by the
  requests in flight.
- A number setting cleared to an empty string or null read as 0 and was
  clamped to its floor, so the size guard became 1,000 bytes. It falls back
  to the default.
- A folder or workspace scan counted a file deleted or made unreadable after
  it was listed as "not UTF-8 text". It has its own line in the report:
  "N file(s) could not be read."
- The MCP server held every byte of a frame until its newline, with no
  ceiling, so a client that streamed without one grew the process without
  bound and got no answer. A frame over 16 MiB is refused with a parse error
  and the next line is served.
- A failure inside a command, such as the editor refusing to open a document
  or a dialog going away, escaped as an unhandled rejection the editor
  reported in its own words, home directory included. Every command reports
  it as one redacted, localized error.
- The crate release workflow read a crates.io answer it could not parse, or
  a failed request, as "unpublished" and would have let the publish run. It
  refuses to publish without a version list.

## [1.2.3] - 2026-10-18

### Changed

- **Activates once the window has started, instead of on 32 language
  IDs.** vsce publishes every `onLanguage:` activation event as a marketplace
  tag, and Open VSX scores tags, so 32 language names diluted the
  keywords people search for. Unicode-LE, its status bar item and its MCP server
  are now ready in every window shortly after startup, rather than when the
  first file in one of those languages opens.

- **The name carries the next words people search for.** Unicode-LE ranked
  12th for "bidi" and 16th for "trojan source" on 2026-10-18, because
  neither was in the name. The name is now "Unicode-LE: Invisible Character,
  Bidi & Trojan Source Detector".

## [1.2.2] - 2026-10-18

### Changed

- **The listing carries the words people search for.** Open VSX ranks a
  match in the name five times a match in the description, and "Unicode-LE"
  holds none of the characters it finds, so a search for "invisible
  characters" put it 14th and "bidi" 19th on 2026-10-18. The name is
  "Unicode-LE: Invisible Character Detector", the description opens with
  "Invisible character detector", where a card has room for about forty
  characters, and the keywords are cut from 29 to 11 phrases someone would
  search for.

## [1.2.1] - 2026-10-17

### Fixed

- **`mcp` no longer ends the session on a frame that is not UTF-8**, in
  the crate or the npm package. Such a frame is dropped like one that is not
  JSON, and the requests after it are served. The npm server reads frames
  with `src/cli/frames.ts`, the crate server's framing in one file shared
  across the family, and `check:cli-pipes` now feeds both servers eighteen
  malformed frames and requests and compares the answers.

## [1.2.0] - 2026-10-17

### Changed

- **One version for every surface.** The extension, the MCP server on
  npm, the new npm command line and the Rust crate all release as 1.2.0
  from the tag `v1.2.0`. The crate jumps from 0.2.3 to join them, and CI
  now fails a commit where any of them disagree.

### Added

- An npm command line, `unicode-le`, built in `cli/` from `src/cli/`. It
  is the Rust CLI's surface over this extension's detection engine: the
  same flags, walk, findings, refusals, escaping, exit codes and JSON
  report, and `unicode-le mcp` serves the same two tools with the same
  bytes. Nothing it writes carries a character it found.
  `scripts/check-cli-parity.ts` runs both binaries over one tree on three
  operating systems and fails on any byte of difference, and
  `scripts/check-cli-pipes.ts` does the same with stdout or stderr closed
  early. It is published as `unicode-le` on npm, beside `unicode-le-mcp`.

### Fixed

- The rating prompt's Rate button opened the Marketplace review page for
  every install, including one from Open VSX in VSCodium, Cursor or
  Windsurf, because it told the registries apart by a publisher name the
  Open VSX listing does not carry. It reads the editor's name instead, and
  an Open VSX install rates on Open VSX.

## [1.1.0] - 2026-10-07

### Added

- A rating prompt. On the 3rd successful use, one notification asks whether
  you would rate the extension, with *Rate*, *Later* and *Don't Ask Again*.
  *Rate* opens the registry your copy was installed from: the VS Code
  Marketplace or Open VSX. *Later*, or dismissing it, asks once more on the
  20th use; that second ask is the last. It never appears on activation or
  after a failed run, and it is never shown if you have set
  `notificationsLevel` to `important` or `silent` yourself. The answer
  follows you through Settings Sync. Translated into all 12 locales.
- Scan one folder. `Unicode-LE: Scan Folder for Unicode Risks` screens every
  file under a folder, from the command palette or from a folder in the
  Explorer. The report names each file relative to that folder.
- A scan's findings can go to the Problems panel. With
  `unicode-le.workspace.scanProblemsEnabled` on, each finding is a warning on
  its line, and each scan replaces the last one's. It is off by default. A
  message gives the codepoint, never the character.
- `unicode-le.workspace.scanMaxResults` caps how many findings a scan lists
  before it stops reading. The default is 10,000.
- `unicode-le.workspace.scanAlwaysInclude` reads a path whatever the switches
  below say.
- A scan's report ends with a line for each thing it left unread.
- Positions are now a setting. `unicode-le.showPositions` decides whether the
  output gives the line and column of each finding, and
  `unicode-le.clipboardIncludesPositions` decides the same for the copy on the
  clipboard. Both are on by default, so the output is what it was.

### Changed

- A folder or workspace scan skips more by default, and says so. Three
  switches are on: `unicode-le.workspace.scanUseDefaultExcludes` skips
  dependency folders, build output, caches and lockfiles,
  `unicode-le.workspace.scanRespectGitignore` skips whatever the project's
  `.gitignore` files skip, and `unicode-le.workspace.scanSkipBinaryFiles`
  skips images, fonts, archives and other files that are not text. A scan
  used to read everything outside six folders and name every image as not
  judged. A character hidden in a skipped file is not reported, so turn the
  switches off to screen everything.
- `unicode-le.workspace.scanExcludes` is empty by default. The six patterns
  it held are covered by the built-in list. It now adds to that list.
- No command is bound to a key by default any more. The one default this
  extension shipped sat on a key the editor, the system or another LE
  extension already used. Every command can still be given a key under
  Keyboard Shortcuts.

## [1.0.1] - 2026-10-04

### Fixed

- The Open VSX links and the Open VSX downloads badge in the README and the
  npm README pointed at a namespace the listing has left, so they led nowhere.
  The listing is under `nolindnaidoo` now, and so are they.

### Removed

- The Zed extension in `zed/`, with the CI job that built it and the workflow
  that synced it. It was never listed in Zed's registry.

## [1.0.0] - 2026-10-03

### Added

- **The VS Code extension.** `Unicode-LE: Detect Unicode Risks` screens the
  active document and `Unicode-LE: Scan Workspace for Unicode Risks` screens
  every file on disk, each opening a Markdown report that names every finding
  by kind, severity, line, UTF-16 column, codepoint and — in JSON, YAML, TOML,
  INI, `.properties`, `.env`, CSV and TSV — the key path it sits under.
- **The report never carries a character it found.** Codepoints are written
  `U+XXXX`; a path or key path from the document is escaped to `\uXXXX`.
- **Settings** for the kinds to report and the scripts the workspace is written
  in, so a translated file is judged rather than refused, plus the workspace
  scan's patterns, excludes and file cap.
- **The MCP server in the VSIX and on npm** as `unicode-le-mcp`: the same
  `detect_unicode_risks` tool the Rust CLI serves, answering identically. Every
  non-ASCII character in a reply leaves as `\uXXXX`.
- **The engine is a port of the crate's**, held to it by the shared corpus, a
  differential that feeds both servers thousands of generated documents with
  codepoints from every plane, and a check that both servers define the tool
  identically. Every Unicode property comes from
  `crate/fixtures/unicode-tables.json`, which the crate renders, so the editor's
  JavaScript engine never decides a finding.
- **The workspace scan refuses rather than guesses**: a UTF-16 or UTF-32 file, a
  binary file and invalid UTF-8 are each refused by name.
- Localized into twelve languages: the manifest and every runtime string.
- A Zed extension that runs the MCP server as a context server.

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
