# AGENTS.md — Unicode-LE

Technical source of truth for this repository. [README.md](README.md) is
the user-facing doc; this file is for anyone, human or agent, changing
the code.

**This repo hosts one product**: the Rust CLI and MCP server in
[`crate/`](crate/). Its own [`crate/AGENTS.md`](crate/AGENTS.md) is the
source of truth for everything *inside* the crate — layout, the settled
decisions, the corpus contract, the definition of done — and
[`crate/SPEC.md`](crate/SPEC.md) defines the product behaviour. **On any
conflict, `crate/AGENTS.md` wins for crate code and this file wins for
the repository around it.**

Read `crate/AGENTS.md` before writing Rust. This file covers what sits
above it: the standard the code is held to, the invariants that survive
across both, the toolchain, CI, and how a change ships.

## What this is

A CLI and MCP server that scans a tree for the Unicode characters that
hide meaning: Trojan Source bidirectional controls (CVE-2021-42574),
invisibles, homoglyphs, words that mix scripts, text that is not in NFC,
spaces that are not the space, and codepoints with no assigned meaning.

It sits on both sides of the LE thesis: a **security screen** and a
**data-prep cleanliness pass**. It reads files and writes none. It makes
no network request, ever, on any surface.

**Status: published on crates.io.** There is no VS Code extension beside
it. When one lands, `crate/fixtures/` becomes
the contract between the two frontends the way it is in the sibling
repos, and the `parity` and `differential` CI jobs those repos run
arrive with it — they are deliberately absent rather than present and
vacuous.

## Architecture

```
crate/
├── src/
│   ├── detect/     pure: the character tables, the script rules, the
│   │               normalization check, encoding, positions, codepoint
│   │               rendering, and the per-format key-path readers
│   │               (json, yaml, toml, ini, dotenv, csv behind locate.rs).
│   │               No filesystem. 75% line coverage floor per module.
│   ├── escape.rs   the one place a path or a key becomes inert
│   ├── walk.rs     ignore-aware tree walking
│   ├── scan.rs     one file end to end — the only path either surface calls
│   ├── cli.rs      the terminal surface
│   └── mcp/        the agent surface
├── fixtures/       the corpus: one document per finding kind, plus the
│                   documents that must produce nothing
├── tests/          contracts, scenarios, and the four hardening tiers
└── SPEC.md         the behavioural contract
```

The seam that matters: **`scan.rs` is the only path either surface
calls.** `cli.rs` and `mcp/` are projections of one implementation, and
`tests/contracts.rs` drives both over one tree and asserts they return
identical reports. A surface that grows its own copy of a rule is a bug.

Everything below `detect/` is pure, which is why the decision layer —
including the two rules the tool rests on, report safety and the
script-context refusal — tests from a string with no disk and no flake.

## Code style

These are not preferences to weigh against convenience. They are the
shape the code is expected to take, and a review rejects work that
ignores them. The reason each one exists is stated, because a rule
without a reason gets cargo-culted into places it does not belong.

### Control flow

**Guard clauses first, then the work.** Every function opens with its
preconditions, each returning immediately. The body that follows is the
happy path at a single indent level, and it reads top to bottom.

**No statement-position `else`.** An `else` is a guard clause that has
not been extracted yet. Two branches become an early return
(`let Some(x) = ... else { return }`); many branches become a `match`
that returns from every arm, or a lookup table. This is the rule that
does the most work in practice — it is what keeps nesting flat and stops
a function growing a second responsibility inside its own `else`.

**Value-position `if/else` is fine.** `let x = if cond { a } else { b }`
is Rust's ternary and reads as one expression.

**`match` is preferred** over any chain of condition tests on the same
value; use match guards rather than `if/else` inside arms.

**Maximum nesting is two levels inside a function.** A third level means
the inner block wants to be its own named function.

Prefer combinators where they read cleanly: `bool::then_some`,
`Option::map/filter/is_some_and`, `?`.

### Errors

**`Result<T, String>` for fallible functions.** No `anyhow`, no
`thiserror`, no `clap`. Arguments are hand-parsed in `cli.rs`.

**Every error path says something true.** A message names what failed,
why, and what state the caller is now in. "Scan failed" is not a message;
"not valid UTF-8 at byte 3: the encoding is unknown and is not guessed"
is.

**Never swallow.** No `let _ =` over a fallible call that mattered, no
`|| true`, no `continue-on-error` in CI. If a failure is genuinely
ignorable, a comment says why.

**Refuse rather than guess.** Ambiguous input returns a named refusal
reason, never a plausible answer. Each of the three reasons exists
because the honest answer was "I did not judge this", and each says what
*did* run. A test that passes by normalizing something that should have
been refused is the bug this whole family exists to prevent.

**Never report success you did not achieve.** A file that vanishes from
the report reads to whoever ran it as a file that was clean, and that is
the one thing this tool must never say.

### Data

**Immutable by default.** Build a new value and return it; never mutate a
parameter. A `&mut` in a signature needs a reason a reader can see.

**No trait with a single implementation**, no layers, registries,
managers or services. Keep modules flat.

**Tables over branches.** The character tables in `detect/characters.rs`,
the `KINDS` table in `detect/mod.rs`, the `FLAGS` list in `cli.rs`: one
table, consulted by the parser, the usage text and the tests, so a kind
cannot exist that no filter can name and a filter cannot name a kind that
does not exist.

**Define it once.** A duplicated rule is a rule that drifts and then gets
fixed in one copy.

### Comments

Comments explain **why**, never what. A comment restating the code is
noise that goes stale. A comment recording the reason a non-obvious
choice was made — the constraint, the bug it prevents, the measurement
behind a threshold — is the most valuable line in the file, and it is
what keeps the next person from "simplifying" it back into a defect.

## Invariants (things that were once broken — keep them true)

- **A finding never carries the character it found.** `codepoints` and
  `resembles` are `U+XXXX`; `detail` is prose written in this crate.
  There is no field holding source text and there will not be one. A
  report that pasted a raw U+202E would reorder the terminal, the diff
  and the ticket of whoever read it, and the tool would be the delivery
  mechanism for the thing it detects. Asserted at three levels:
  `detect::hazards` over the whole corpus, `tests/contracts.rs` at the
  process boundary on both streams, and `tests/fuzz.rs` on every
  generated document.
- **Nothing this tool prints is ever non-ASCII, including the two fields
  it does not author.** `file` is the caller's path and `key` is text out
  of the document, so neither can be rewritten — a path has to open and a
  key has to match. Both are **escaped** instead: every non-ASCII
  codepoint is emitted as `\uXXXX`, which is JSON's own escape, so the
  wire bytes are inert *and* a parser decodes them back byte for byte.
  The exemption those two fields used to have is exactly what made a
  hostile file name exploitable. `escape` is the one place that decides
  it, applied to the **serialized document** — escaping the field first
  does not survive `serde_json`, which escapes the backslash and ships
  `\\u202E`.

  **Practical consequence: no em dashes, no curly quotes, no arrows in
  any `detail` or refusal string.** Doc comments may have them; anything
  reaching the output may not. This has already broken once.
- **The confusable check fires on mixing, never on a script.** Text
  wholly in one non-Latin script produces nothing. Getting this wrong
  makes the tool unusable on the internationalised repositories that most
  need it, and once it is switched off, so is the Trojan Source screen.
- **Declaring a script turns the checks on, never off — and never on
  harder.** The `intentional_script_context` share is measured over the
  **undeclared** scripts alone. Measuring every non-Latin letter and
  naming only the undeclared ones refused a file the caller had already
  accounted for — which meant declaring the script a repository is
  written in disabled the homoglyph check for every file in it, exactly
  where one would hide. Pinned by
  `the_share_is_measured_over_the_undeclared_scripts_only` and
  `a_declared_file_is_judged_and_its_homoglyph_is_still_found`.

  **`scripts::judge` got the same direction wrong twice**, so the
  property is now stated on its own:
  `declaring_a_script_never_adds_a_finding`. The second time, a word that
  mixed an undeclared script with Latin returned early and never reached
  the compatibility check, so declaring a script *added* four findings on
  `設ＦＩＬＥ`. **A compatibility form is not a script question** — a
  full-width `Ｆ` reads as `F` in any file — and the guard against a
  third occurrence is structural rather than a comment: `compatibility`
  does not take the declared scripts as a parameter, so nothing can gate
  it by them. Only `mixing` may see them.
- **Columns are UTF-16 code units and lookups are checkpointed.**
  UTF-16 is what an editor's ruler shows. Counting them from the line
  start on every lookup is quadratic on a minified bundle — one line
  holding the whole document, one lookup per finding — which measured
  21.8s for 20,000 findings before `detect/position.rs` grew checkpoints
  and 0.33s after. `tests/budget.rs` is what keeps it.
- **Paths in the report are separated by `/` on every platform.** A
  report is diffed against one produced on another machine. `\` on
  Windows made every path differ for no reason a reader could see, in a
  sibling, for a whole release. `scan::report_path` is the one place that
  decides this; asserted by `tests/platform.rs`. Separator, not encoding:
  the escaping above is a different rule and the two do not collide.
- **A format decides how a finding is addressed, never whether it
  exists.** This is the inversion the whole `detect/locate.rs` layer
  rests on, and it is the opposite of a format-aware extractor. Every
  scanner runs over the same raw text whatever the format is, so a
  document nothing can parse is still scanned and still reports
  everything in it — it loses its key paths and not one finding. The
  readers are line scanners rather than parsers, which is also what keeps
  the offsets the raw document's. `a_format_never_changes_which_findings_exist`
  runs one document through every reader and asserts the findings are
  identical each time.

  **The corollary is why `coverage-matrix` checks the readers.** A lost
  key path costs no finding, so a reader that quietly stopped naming
  anything would pass every other test in the suite.
- **A leading UTF-8 BOM is not stripped**, unlike the sibling crates. The
  report carries byte offsets into the file on disk; deleting three bytes
  would move all of them. It is excluded from the findings instead, which
  is a rule about meaning rather than an edit to the input.
- **No encoding is guessed.** Byte-order mark first, binary sniff second,
  UTF-8 last — in that order, because a UTF-16 file carries NUL bytes and
  sniffing first names the wrong reason. A mis-decode here does not
  merely miss findings, it *invents* them.
- **Nothing is rewritten.** No `--fix`, no normalization, no stripping.
  Contract tests assert no flag and no tool schema offers it, and that a
  scanned file is byte-identical afterwards.
- **stdout is protocol, stderr is human. There is no `--json` flag.** One
  JSON document for the run, `schema: 1`, no timestamp — two runs over an
  unchanged tree produce identical bytes.
- **Refusals speak the caller's vocabulary.** An MCP caller has no
  command line; no message aimed at one names a flag, and a test asserts
  no MCP output contains `--`.
- **No network, on any surface, ever.**

## Testing

The default suite runs everywhere on every push. Six further tiers run
in CI, and **each exists because something real got through a green
suite**; each names the bug it would have caught.

| Tier | What it reaches that unit tests cannot | Where |
|---|---|---|
| `contracts` | the exit codes and the stream contract, driven against the built binary | `crate/tests/contracts.rs` |
| `hazards` | a real filesystem: a byte-order mark, a non-UTF-8 file, a FIFO, a permission-denied file, a 260-character path, a symlink loop, an empty file, a one-line bundle | `crate/tests/hazards.rs` |
| `platform` | a second operating system: separators, case folding, reserved device names, CRLF against a lone CR, stdin closing early, `TZ` set and unset | `crate/tests/platform.rs` |
| `fuzz` | text nobody would type, time-boxed and seeded | `crate/tests/fuzz.rs` |
| `budget` | a clock: a wall-clock ceiling plus linearity in both directions | `crate/tests/budget.rs` |
| `scenarios` | a document far larger than an editor opens | `crate/tests/scenarios.rs` |
| `coverage-matrix` | whether every format reader, `kind`, `severity` and `reason` is reachable from a real fixture | `crate/src/detect/corpus.rs` |

Rules that hold across all of them:

- **A skipped case is never reported as a pass.** `hazards` and
  `platform` name every case the platform cannot express on stderr;
  `budget` and `scenarios` say plainly that they did not run. **A gated
  tier that no job sets is a tier that has never run** — a sibling
  shipped a scenarios suite asserting a shape the code had stopped
  producing — so every gate above has the job that turns it on.
- **The tree is built at runtime**, not checked in: Windows cannot hold a
  FIFO, a permission-denied file, or half of these names in git.
- **A marker line is not decoration.** `cargo test <filter>` exits 0 when
  the filter matches nothing, so a renamed test would leave a CI job
  green and checking nothing. `coverage-matrix` prints markers and the
  job greps for them.
- **Every bug fix ships with a regression test that fails before the
  fix**, and the failure is *observed*, not assumed. The quadratic above
  was proved by reverting the fix and watching
  `four_times_the_content_in_one_document_is_not_six_times_the_clock`
  report 13.02× against its 6× limit.
- **`detect/` carries a 75% line coverage floor per module.** A floor is a backstop against an untested module, not a target: it sits well below where the code actually is, and is not raised to track it.

## Toolchain

- **Rust**, `edition = "2024"`, MSRV **1.88** — declared in
  `crate/Cargo.toml` and verified by the `msrv` CI job, which is the only
  thing that stops the declared floor becoming a fiction.
  `crate/rust-toolchain.toml` gives contributors the toolchain CI uses.
- **Clippy pedantic, deny warnings.** `cargo clippy --all-targets --
  -D warnings`.
- **No inline `#[allow(...)]` anywhere.** The `policy` CI job greps
  `crate/src` for one. A lint you mean to relax goes in
  `[lints.clippy]` in `crate/Cargo.toml` with a comment saying why, so
  every relaxation is visible in one place.
- **`unsafe` is forbidden crate-wide** (`[lints.rust]`), with no platform
  exemption. A test is not an exemption either: `tests/hazards.rs` shells
  out to `mkfifo` rather than calling libc.
- **Overflow checks stay on in release.** Every output is a claim
  somebody scripts against — a line number, a column, a byte offset, a
  count — and a wrapped number is silently wrong data in a report whose
  whole value is honesty, which is worse than a crash.
- **Four dependencies beyond serde**: three unicode-rs crates carrying
  data this crate must not copy, and `ignore` for the walk. Every one is
  justified by a comment in `crate/Cargo.toml`. Justify any addition; a
  crate that *rewrites* text has no place here whatever it detects on the
  way.
- **Coverage** is `cargo llvm-cov`, enforced per module rather than on
  the total: a total hides one module sliding while the others carry it.

## Security & automation

- **CodeQL** runs on push, PR and weekly, configured in
  `.github/codeql-config.yml`. Fixtures are excluded on purpose: they
  contain inputs that are *supposed* to look dangerous, and scanning them
  produces findings that can only ever be dismissed.
- **`cargo audit`** runs as its own CI job.
- **Dependabot** opens grouped weekly PRs.
- **Auto-merge is workflow-driven, not GitHub-native**: `main` has no
  required status checks, so native auto-merge would land a PR before CI
  started. `dependabot-auto-merge.yml` waits for the run to conclude.
- **Actions are pinned to commit SHAs.** A tag is mutable. The trailing
  `# vX.Y.Z` comment is what Dependabot reads and rewrites.
- **`ci-crate.yml` is workflow-dispatchable.** Push events are not
  guaranteed: during a GitHub Actions incident they are throttled and
  silently dropped, and a dropped event never replays, leaving a commit
  with no way to be verified.

## Agent and editor instructions

Every major coding assistant looks for its own instruction file, so each
one is present and each is a thin pointer to this document:

| File | Tool |
|---|---|
| `AGENTS.md` | the standard itself — OpenAI Codex and others read this directly |
| `CLAUDE.md` | Claude Code |
| `GEMINI.md` | Gemini CLI |
| `.cursorrules`, `.cursor/rules/project.mdc` | Cursor (legacy and current formats) |
| `.windsurfrules` | Windsurf |
| `.clinerules` | Cline |
| `.github/copilot-instructions.md` | GitHub Copilot |

**Keep them thin.** They restate the non-negotiables and route the reader
here; they must never grow a second copy of the standard, because a copy
drifts and then two tools disagree about the same repository. Change the
standard here, and only the pointer's short list if a non-negotiable
itself changed.

None of them ship: `crate/Cargo.toml` excludes `AGENTS.md` and
`CLAUDE.md` from the package, and the rest are above the crate directory
where `cargo package` cannot reach.

## Git identity

Every commit uses the GitHub noreply address:

```
13629544+nolindnaidoo@users.noreply.github.com
```

A real address in commit metadata is public forever — GitHub's API serves
it for any public repo, and scrapers harvest it. Never set a real address
in `user.email`, globally or repo-locally, and never commit with one. A
repo-local `user.email` silently overrides the global one, so check
`git config user.email` in a fresh clone before the first commit.

## Commits
- **Commits are conventional and CI enforces it.** The `commits` job in
  `.github/workflows/ci-crate.yml` validates every pushed commit's subject
  against the same pattern and the same 72-character cap as
  `.githooks/commit-msg`. The hook is opt-in per clone (`git config
  core.hooksPath .githooks`), so `--no-verify` and a fresh checkout defer
  the check to CI rather than escaping it. Scopes may be comma-separated.

Subjects use a conventional prefix — `feat:`, `fix:`, `docs:`, `test:`,
`ci:`, `build:`, `chore:`, `refactor:`, `perf:`, `revert:` — an optional
`(scope)`, and an imperative summary under 72 characters with no trailing
period. The body says why the change was needed and what it prevents; a
subject alone is rarely enough to reconstruct a decision six months
later.

The `commit-msg` hook in `.githooks/` rejects the message before the
commit exists. It is POSIX shell rather than a call out to Node, because
the sibling extension repos share a `scripts/commit-lint.js` and this
repo has no JavaScript in it at all. Point git at it once per clone:

```bash
git config core.hooksPath .githooks
```

Merge and revert commits are exempt — git writes those subjects, not a
person.

**The hook is not the only gate.** The `commits` job in `ci-crate.yml` runs
the same check over the pushed range, so `--no-verify` delays the failure
rather than avoiding it — the same arrangement the extension repos have.

## Release

1. Bump `version` in `crate/Cargo.toml`, and write both CHANGELOG
   entries: the root [CHANGELOG.md](CHANGELOG.md) for what a reader of
   the repository needs, and [`crate/CHANGELOG.md`](crate/CHANGELOG.md)
   for what ships inside the package. The entry must describe what
   actually changed, including bug fixes.
2. Update SPEC.md in the same commit if behaviour moved. A claim in a doc
   that the code does not back is the defect this family is least willing
   to ship.
3. CI green on all three operating systems — including the five
   hardening jobs, not only `test`.
4. Tag the commit being released, so the tag is the artifact rather than
   an approximation of it.
5. Dispatch the **Release (crate)** workflow with `publish` unchecked.
   The `preflight` job runs the gates, asserts the packaged crate carries
   `fixtures/` (a fixture that does not travel makes the published tests
   unrunnable by anyone else), asserts the version is not already on
   crates.io, asserts `crate/CHANGELOG.md` documents it, and finishes
   with `cargo publish --dry-run`.
6. Dispatch it again with `publish` checked. **Dispatch-only, never on a
   tag push**: a crates.io version can never be reused, so the
   irreversible step is one someone chooses on purpose. The token lives
   on the `crates-io` GitHub environment and is verified to be non-empty
   before anything irreversible happens.

## Known limitations (documented, not bugs)

- **A path named explicitly that is not a regular file or a directory
  vanishes from the report.** `walk::collect` selects regular files, so a
  FIFO, a socket or a device named on the command line produces a run
  with zero files and exit 0 — which reads as a clean answer about a file
  that was never looked at, and sits against `walk.rs`'s own rule that a
  file named explicitly is always read. It does not hang, which is the
  hazard that matters, and `a_fifo_does_not_block_the_run` pins that
  floor. **Unresolved: whether such a path should be a named refusal or a
  malformed question.**
- **`unicode-security` is on Unicode 16.0 data while `unicode-script` is
  on 17.0.** A confusable pair added in Unicode 17 is not detected, and
  the `unassigned-or-private-use` check is a release ahead of the
  confusable check. Neither is a correctness bug; both are the cost of
  standing on maintained data instead of copying it, and both improve
  when upstream does. Written down in `crate/SPEC.md` rather than glossed.
- **One false positive is left on purpose, outside JSON.** A backslash
  escape immediately followed by non-Latin text makes a mixed word:
  `"Hello\nПривет"` contains the byte run `nПривет`. Inside a JSON
  string the grammar settles it — `\n` is a line feed, so the JSON
  reader hands the word splitter the escape ranges and the word breaks
  there. In a `.rs`, a `.py` or a `.txt` it is still reported, because
  the bytes really are a Latin letter followed by Cyrillic and a
  backslash is an ordinary character where `C:\Привет` is a path.
  Resolving it needs a grammar that says so, and this crate has one for
  JSON and for nothing else it reads. `--kind` narrows it away.
