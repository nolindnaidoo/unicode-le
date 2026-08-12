# unicode-le (CLI) — engineering standards

This is the source of truth for how code in `crate/` is written, tested,
and reviewed. It applies to every contributor, human or AI-assisted.
[SPEC.md](SPEC.md) defines the product behavior — findings, refusals,
exit codes, both surfaces; this file is how the code gets there. AGENTS.md
wins on any conflict.

## What this project is

The command-line and MCP frontend of Unicode-LE: scan a tree for the
Unicode characters that hide meaning — Trojan Source bidirectional
controls, invisibles, homoglyphs, words that mix scripts, text that is
not in NFC, spaces that are not the space, and codepoints with no
assigned meaning. It sits on both sides of the family thesis: a security
screen and a data-prep cleanliness pass.

**Status: v0.1.0 core.** Both surfaces, all seven findings, all three
refusals and the test tiers below are built and green. There is no
VS Code extension yet; when one lands, `fixtures/` becomes the contract
between the two frontends the way it is in the sibling repos.

## Layout

```
crate/src/
├── detect/         pure: the character tables, the script rules, the
│                   normalization check, encoding, positions, the
│                   codepoint rendering, and the key-path readers —
│                   format.rs decides which, locate.rs is the seam, and
│                   json/yaml/toml/ini/dotenv/csv are the readers.
│                   No filesystem, pub(crate).
├── escape.rs       the one place a path or a key becomes inert
├── walk.rs         ignore-aware tree walking
├── scan.rs         one file end to end — the only path either surface calls
├── cli.rs          the terminal surface
└── mcp/            the agent surface
```

- **`detect/` touches no filesystem.** It takes bytes or text and
  returns findings and refusals, so the entire decision layer — including
  the two rules the tool rests on, report safety and the script-context
  refusal — tests from a string with no disk and no flake. It carries the
  **90% line coverage floor per module**. A `std::fs` call appearing
  there is a bug.
- **`scan.rs` is the only path either surface calls.** `cli.rs` and
  `mcp/` are projections of one implementation; a surface that grows its
  own copy of a rule is a bug, and `tests/contracts.rs` asserts the two
  return identical reports for the same tree.
- **`walk.rs` selects, it does not decide.** Its one rule — a file named
  explicitly is read whatever the ignore rules say — is why intent beats
  configuration.
- Keep modules flat. No layers, registries, managers, or services. No
  trait with a single implementation.

## Decisions already made (do not relitigate)

- **A finding never carries the character it found.** `codepoints` and
  `resembles` are `U+XXXX`; `detail` is prose written in this crate. There
  is no field holding source text and there will not be one. A report
  that pasted a raw U+202E would reorder the terminal, the diff and the
  ticket of whoever read it, and the tool would be the delivery
  mechanism for the thing it detects. `detect::hazards` asserts a
  serialized report is pure ASCII, and `tests/contracts.rs` asserts it
  again at the process boundary, on both streams.

  **Practical consequence: no em dashes, no curly quotes, no arrows in
  any `detail` or refusal string.** Doc comments may have them; anything
  that reaches the output may not. This has already broken once.
- **The confusable check fires on mixing, never on a script.** Text
  wholly in one non-Latin script produces nothing. Getting this wrong
  makes the tool unusable on any internationalised repository, which is
  the kind that most needs it — and once it is switched off, so is the
  Trojan Source screen. Three rules hold it, all in `detect/scripts.rs`
  and all documented in SPEC.md: word-level judging, the 10% file-level
  refusal, and a declared script being an *expected* one.
- **The 10% threshold was measured.** A `zh-CN` UI catalogue is 16%
  Han; a foreign word quoted in an English document is 0.1%. Do not
  change the number without measuring again on real locale files and
  saying what you measured.
- **`--script` changes what counts, not just whether the file is
  judged.** CJK has no spaces, so a Latin product name inside a
  translated string is one word with it. Without this rule, declaring
  the scripts on two real locale trees produced 102 findings. The
  narrowing that keeps it honest: mixing with an *undeclared* script is
  still a finding in the same file, and a compatibility form is not a
  script question and is reported either way.
- **Declaring turns the checks on, never off.** The refusal share is
  measured over the **undeclared** scripts alone, over every letter in
  the file. Measuring all non-Latin letters and then naming only the
  undeclared scripts refused a file the caller had already accounted for
  — so declaring the script a repository is written in disabled the
  homoglyph and mixed-script checks for every file in it, which is
  exactly where a homoglyph would be hiding. The refusal message is
  conditional for the same reason: telling a caller no script was
  declared when one was is false.
- **Paths in the report use `/` on every platform.** `scan::report_path`
  is the one place that decides it, and `tests/platform.rs` asserts it. A
  refusal naming a caller-supplied path is the exception and echoes it
  verbatim, because a message that rewrites its path cannot be grepped
  for.
- **`file` and `key` are the two fields this crate does not author, and
  they are escaped rather than rewritten.** Every non-ASCII codepoint in
  anything printed becomes `\uXXXX`; `escape` is the one place that
  decides it, applied to the *serialized document* because escaping a
  field first does not survive `serde_json`. Both must round-trip — a
  path has to open, a key has to match the document — and a test asserts
  the decoded path opens the file it names. The exemption these two used
  to have is what made a hostile file name exploitable.
- **A format decides how a finding is addressed, never whether it
  exists.** `detect/locate.rs` and the six readers behind it contribute
  key paths and nothing else; every scanner runs over the same raw text
  whatever the format. A document nothing can parse is still scanned and
  loses only its key paths. They are line scanners rather than parsers,
  which is what keeps the offsets the raw document's — and each states
  its own limits in its own module doc.
- **The JSON reader is the only one allowed to resolve an escape.**
  `\n` inside a JSON string is a line feed, so the word splitter breaks
  there and `"Hello\nПривет"` is not a mixed word. Everywhere else the
  bytes really are a Latin letter against Cyrillic and it stays a
  finding. Do not extend this to a format whose grammar this crate does
  not read.
- **Column lookups are checkpointed, not counted from the line start.**
  `detect/position.rs` carries a checkpoint every kilobyte, empty for an
  ASCII document. Without them a minified bundle — one line, one lookup
  per finding — is quadratic: 20,000 findings measured 21.8s before and
  0.33s after.
- **Nothing is rewritten.** No `--fix`, no normalization, no stripping.
  Contract tests assert no flag and no tool schema offers it, and that a
  scanned file is byte-identical afterwards.
- **No encoding is guessed.** Byte-order mark first, binary sniff
  second, UTF-8 last — in that order, because a UTF-16 file carries NUL
  bytes and sniffing first names the wrong reason. A mis-decode here does
  not merely miss findings, it *invents* them.
- **A leading UTF-8 BOM is not stripped**, unlike the sibling crates.
  The report carries byte offsets into the file on disk; deleting three
  bytes would move all of them. It is excluded from the findings
  instead, which is a rule about meaning rather than an edit.
- **Columns are UTF-16 code units.** That is what an editor shows, and
  pointing at the wrong character defeats the whole finding. Byte
  offsets are carried alongside.
- **Refusals are results, not errors.** The run carries on and the
  reader is told what was not covered. `--strict` is the only way one
  reaches the exit code.
- **stdout is protocol, stderr is human. There is no `--json` flag.**
  One JSON document for the run — not JSON Lines, unlike `regex-le` —
  because the interesting numbers here are totals.
- **Refusals speak the caller's vocabulary.** An MCP caller has no
  command line; no message aimed at one names a flag. A test asserts no
  MCP output contains `--`.

## Control-flow style

Flat over nested, guards over branches — the same rules as pixelcoords,
pixelactions, scrape-le and regex-le:

- **No statement-position `else`.** Guard clauses and early `return`
  (`if !ok { return ... }` / `let Some(x) = ... else { return }`), then
  fall through to the happy path.
- **Value-position `if/else` is fine** — `let x = if cond { a } else
  { b }` is Rust's ternary.
- **`match` is fine and preferred** over any chain of condition tests on
  the same value; use match guards instead of `if/else` inside arms.
- Prefer combinators where they read cleanly: `bool::then_some`,
  `Option::map/filter/is_some_and`, `?`.
- No nesting deeper than two levels inside a function; extract a named
  helper instead.

## Hard rules

- **No inline `#[allow(...)]`.** Either fix the lint or add a visible,
  commented relaxation to `[lints.clippy]` in `Cargo.toml`.
- **Clippy pedantic, deny warnings.** `cargo clippy --all-targets --
  -D warnings` must pass.
- **No `anyhow`, no `thiserror`, no `clap`.** Errors are
  `Result<T, String>` and arguments are hand-parsed in `cli.rs`.
- **No async runtime.** This tool reads files and scans text.
- **`unsafe` is forbidden crate-wide** (`[lints.rust]`).
- **Dependencies are a cost, and the Unicode ones are the exception
  that proves it.** Four dependencies beyond serde: three unicode-rs
  crates carrying data this crate must not copy, and `ignore` for the
  walk. Every one is justified by a comment in `Cargo.toml`. Justify any
  addition; a crate that *rewrites* text (`decancer`) has no place here
  whatever it detects on the way.
- **No network, ever.**
- **Strict parsing, never silent defaults.** An unrecognised flag, a
  kind that does not resolve, a script tag that is not a writing system:
  all are errors with actionable messages. A typo'd `--strict` that
  quietly did nothing would produce a report the caller believed had
  insisted on coverage.
- **Refuse rather than guess.** Every one of the three refusal reasons
  exists because the honest answer was "I did not judge this", and each
  says what *did* run.

## The corpus contract

`fixtures/` lives inside this crate so the published package is
self-contained — `cargo package` cannot reach above its own directory —
and so `cargo test` on the unpacked tarball runs every case, which makes
the claims in the README checkable rather than trusted.

`fixtures/documents/` holds one document per finding kind, plus the
documents that must produce **nothing**: `clean.ts`, and the three
translations `zh-cn.json`, `ru.json` and `ja.json`. Those three are the
most important files in the repository. Two tests guard them, and both
are needed:

- `no_translation_produces_a_confusable_or_mixed_script_finding` — the
  guard, stated directly rather than inferred from an expectation list
  that could be quietly edited to match a regression.
- `a_declared_translation_is_judged_and_is_still_clean` — without it the
  first would pass for the wrong reason, because a refusal suppresses
  the checks and "no findings" would mean "not judged".

Changing a document or an expectation is a behavior change and needs a
CHANGELOG entry.

## Testing

The bar, enforced by review:

- **`detect/`: 90% line coverage floor per module.** Everything in it is
  pure; if something is hard to test there, the design is wrong.
- **Exit codes and the stream contract belong in `tests/contracts.rs`.**
  They are the API — callers branch on them — so they are pinned by
  tests that drive the built binary against a temporary tree. A new
  refusal adds its case there.
- **Anything needing a document larger than an editor opens is
  `tests/scenarios.rs`**, gated behind `UNICODE_LE_SCENARIOS`. A skipped
  scenario is never reported as a pass; each one says plainly that it
  did not run. **No CI job sets that variable today** — see the root
  AGENTS.md's known limitations.
- **Four hardening tiers, each because something real got through a green
  suite**, each with its own CI job and each naming the bug it would have
  caught: `tests/hazards.rs` (a real filesystem), `tests/platform.rs` (a
  second operating system), `tests/fuzz.rs` (text nobody would type,
  time-boxed and seeded), `tests/budget.rs` (a wall-clock ceiling and
  linearity in both directions). The `coverage_matrix_*` tests in
  `detect/corpus.rs` are the fifth: every format reader, `kind`,
  `severity` and `reason` reachable from a real fixture. The reader half
  earns its place — a lost key path costs no finding by design, so a
  reader that stopped naming anything would pass every other test here. They print marker lines because
  `cargo test <filter>` exits 0 when the filter matches nothing, and CI
  greps for them.
- **A regression test's failure is observed, not assumed.** Revert the
  fix, watch the test go red, restore it. The quadratic in
  `position.rs` was proved that way: 13.02x against a 6x limit.
- **Every table entry carries its test.** A character in `characters.rs`
  that no test classifies is a silently dead row, and
  `every_table_entry_classifies` fails on one.
- **Every bug fix ships with a regression test** that fails before the
  fix.
- **Run the binary against a real internationalised tree, not only the
  tests.** The 102-finding false positive that produced the
  declared-script rule was invisible to every test in the suite: the
  fixtures were correct, small, and did not contain a product name
  inside a Japanese string. Point it at a locale directory before
  claiming anything about false positives.
- Tests are deterministic: no clocks, no randomness, and **no filesystem
  in `detect/` tests**.

## Verification — the definition of done

All of it, before every push:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
UNICODE_LE_SCENARIOS=1 cargo test --locked
```

And when `detect/` or `scan.rs` changed, the two tiers the default suite
runs only at their smallest setting:

```bash
UNICODE_LE_FUZZ_SECONDS=60 cargo test --locked --test fuzz -- --test-threads=1 --nocapture
UNICODE_LE_BUDGET=1 cargo test --locked --test budget -- --test-threads=1 --nocapture
```

A change is not done because it compiles; it is done when it is tested,
linted, documented where behavior changed (README / CHANGELOG / SPEC /
this file), and honest — claims in docs must match the code.
