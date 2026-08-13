# Contributor and agent instructions

**Read [AGENTS.md](AGENTS.md) before writing any code.** It carries the
engineering standard this repository is held to — control flow, error handling,
module shape — plus the architecture, the invariants and why each one exists.
[CLAUDE.md](CLAUDE.md) is the short version: gates and traps.

This repo is crate-only: everything real lives in `crate/`, and
[`crate/AGENTS.md`](crate/AGENTS.md) wins over the root one for anything inside
it. [`crate/SPEC.md`](crate/SPEC.md) defines the product behaviour.

This file exists only to route you there. It is deliberately thin: the standard
lives in one place so it cannot drift between tools.

## Non-negotiables

- Guard clauses first. **No statement-position `else`** — two branches are an
  early return, many are a `match` or a lookup table. Value-position `if/else`
  is fine.
- Nesting stops at two levels inside a function.
- **`Result<T, String>` for fallible functions.** No `anyhow`, no `thiserror`,
  no `clap`.
- `unsafe` is forbidden crate-wide, with no platform exemption. A test is not
  an exemption either.
- **No inline `#[allow(...)]` anywhere.** CI greps for it. A lint you mean to
  relax goes in `[lints.clippy]` in `crate/Cargo.toml` with a comment saying
  why.
- Flat modules. No layers, registries, managers or services, and no trait with
  a single implementation.
- **Refuse rather than guess.** Ambiguous input returns a named refusal reason,
  never a plausible answer. A test that passes by normalizing something that
  should have been refused is the bug this whole family exists to prevent.
- **Nothing in the output is ever non-ASCII**, and no finding ever carries the
  character it found. A report that pasted a bidi control would reorder the
  terminal of whoever read it.
- **stdout is protocol, stderr is human.** There is no `--json` flag, and exit
  codes are part of the API.
- Nothing is rewritten. No `--fix`, no normalization, no stripping.
- No network, on any surface, ever.
- Never report success you did not achieve.
- Comments explain **why**, never what.
- Commits are conventional (`fix:`, `feat:`, `docs:`…), imperative, under 72
  characters, and enforced by the `commit-msg` hook in `.githooks/`.

## Before you commit

```bash
cd crate && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --locked
```

Coverage floors are a backstop against an untested module, not a target: they
sit well below where the code actually is and are never raised to track it.
Every claim in a README or in help text must be provable against the code — run
the binary and read the output, do not describe what you expect it to print.

**Provable is about behaviour and numbers, not availability.** An install line
for a publish you are about to make is *staged*, not forbidden — write it, and
let the release commit be what makes it true.
