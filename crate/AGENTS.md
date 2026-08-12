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
│                   codepoint rendering. No filesystem, pub(crate).
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
  did not run.
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

A change is not done because it compiles; it is done when it is tested,
linted, documented where behavior changed (README / CHANGELOG / SPEC /
this file), and honest — claims in docs must match the code.
