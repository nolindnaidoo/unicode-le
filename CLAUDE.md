# CLAUDE.md

[AGENTS.md](AGENTS.md) is the technical source of truth for this repo: the
engineering standard the code is held to — control flow, error handling,
immutability, structure — plus the architecture, the invariants and why each
one exists. Read it before writing code.

**This repo is crate-only.** Everything real lives in `crate/`, and
[`crate/AGENTS.md`](crate/AGENTS.md) wins over the root one for anything inside
it: layout, the settled decisions, the corpus contract, the definition of done.
[`crate/SPEC.md`](crate/SPEC.md) defines the product behaviour. There is no
VS Code extension here yet — when one lands, `crate/fixtures/` becomes the
contract between two frontends and the `parity` and `differential` CI jobs
arrive with it.

## Where to look

| Question | File |
|---|---|
| How should this code be written? | [AGENTS.md](AGENTS.md), then [`crate/AGENTS.md`](crate/AGENTS.md) |
| What is the tool supposed to do? | [`crate/SPEC.md`](crate/SPEC.md) |
| What does the user see? | [README.md](README.md) |
| What changed? | [CHANGELOG.md](CHANGELOG.md) and [`crate/CHANGELOG.md`](crate/CHANGELOG.md) |
| What is known-broken? | [AGENTS.md](AGENTS.md#known-limitations-documented-not-bugs) |

## Gates

```bash
cd crate
cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --locked
```

All three, before every push. Before a release, also run the hardening tiers
the way CI does — they are not in the default suite by accident, but a change
to `detect/` or `scan.rs` should see them at least once:

```bash
cargo test --locked --test hazards --test platform -- --nocapture
UNICODE_LE_FUZZ_SECONDS=60 cargo test --locked --test fuzz -- --test-threads=1 --nocapture
UNICODE_LE_BUDGET=1 cargo test --locked --test budget -- --test-threads=1 --nocapture
UNICODE_LE_SCENARIOS=1 cargo test --locked --test scenarios
```

## Things that will bite you

- **Nothing in the output may be non-ASCII.** No em dashes, no curly quotes,
  no arrows in any `detail` or refusal string — a report that carried a
  character it found would be the delivery mechanism for the thing it detects.
  Doc comments may have them; anything reaching stdout or stderr may not. This
  has already broken once, and three test layers assert it.
- **`file` and `key` are not written by this crate**, so they are escaped
  rather than sanitised: `escape::json` on the serialized document, never on
  the field, because `serde_json` escapes a backslash you added and the value
  stops round-tripping. Both *must* round-trip — a path has to open and a key
  has to match the document. `escape::text` is the stderr half.
- **A format never changes which findings exist**, only how they are
  addressed. If a change to `detect/locate.rs` or a reader alters a finding,
  the change is wrong. `a_format_never_changes_which_findings_exist` is the
  guard, and `coverage-matrix` is the other half: a lost key path costs no
  finding, so a reader that stopped naming things passes everything else.
- **Only the JSON reader may resolve an escape sequence.** `\n` inside a JSON
  string is a line feed and breaks a word; a backslash in a `.txt` is an
  ordinary character and `C:\Привет` is a path. Do not extend this to a
  grammar the crate does not read.
- **Declaring a script turns the checks ON.** The refusal share is measured
  over *undeclared* scripts only. Getting this backwards disabled the
  homoglyph check for every file in an internationalised repository, which is
  precisely where one would hide. See the invariant in AGENTS.md.
- **Columns are UTF-16 code units, and the lookup is checkpointed.** Counting
  from the line start per lookup is quadratic on a minified bundle.
  `tests/budget.rs` is what keeps it; prove a change with it, do not reason
  about it.
- **Never add an inline `#[allow(...)]`.** CI greps `crate/src` for one. Fix
  the lint, or add a commented relaxation to `[lints.clippy]` in
  `crate/Cargo.toml`.
- **A skipped test is never a pass.** Every gated tier prints `SKIPPED <name>`
  with the reason. If you add a case a platform cannot express, skip it by
  name.
- **`cargo test <filter>` exits 0 when the filter matches nothing.** That is
  why `coverage-matrix` prints marker lines and CI greps for them. Renaming
  one of those tests without updating the workflow leaves the job green and
  checking nothing.
- **Changing a fixture or an expectation is a behaviour change** and needs a
  CHANGELOG entry. The three translation fixtures — `zh-cn.json`, `ru.json`,
  `ja.json` — are the most important files in the repository.
- **Scaffolding is shared with the other crate-only repos, not with the
  extension repos.** `.editorconfig`, `.gitattributes`,
  `.githooks/commit-msg`, `dependabot.yml`, `codeql-config.yml`,
  `codeql.yml` and `dependabot-auto-merge.yml` are byte-identical across the
  six, and `letools-site/scripts/check-fleet.ts` holds them there — run
  `bun run check:fleet ../` from a checkout of the site. **`ci-crate.yml` and
  `release-crate.yml` are this repo's own**, deliberately: the crates stand on
  their own. The agent instruction files are one document *within* a repo and
  never across them. The extension-shaped files (`ci.yml`, `biome.json`,
  `release.yml`, `zed-sync.yml`) do not exist here — copying one from a
  two-frontend sibling re-imposes a shape this repo does not have.
- **CI narrows itself on a docs-only push.** `ci-crate.yml` fires on `*.md` and
  the agent instruction files — it has to, because the `policy` job greps them,
  and the filter used to admit only `crate/**` so that gate could run only when
  the files it guards had *not* been touched. On a docs-only push `policy` and
  `commits` run and every Rust job skips. Anything unrecognised, and an
  unreadable diff, counts as code and runs everything.
- **Coverage floors are a backstop, not a target** — well below where the code
  actually is, and never raised to track it. 75% per
  module in `detect/`, enforced per module rather than on the total.
- **Every claim must be provable.** No number, format or behaviour goes in a
  README, a doc or help text unless the code backs it. Run the binary and read
  the output before writing down what it prints. That governs **behaviour and
  numbers**, not **availability**: an install line for a publish you are about
  to make is **staged, not forbidden**. Write it, and let the release commit be
  what makes it true.
