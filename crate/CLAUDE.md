# Instructions for AI coding assistants

Read [AGENTS.md](AGENTS.md) first — it is the engineering-standards
document for this crate and the source of truth for layout, control-flow
style, the settled decisions, the corpus contract, testing requirements
and the definition of done. [SPEC.md](SPEC.md) defines the product
behavior. AGENTS.md wins on any conflict. The repository root has its own
`AGENTS.md` covering what sits above the crate.

- Before declaring any change complete, run exactly what CI runs:
  `cargo fmt --all --check`,
  `cargo clippy --all-targets -- -D warnings`,
  `cargo test --locked`. All three must pass.
- Never add an inline `#[allow(...)]`. Fix the lint, or add a commented
  relaxation to `[lints.clippy]` in `Cargo.toml`. The `policy` job greps
  `src/` for one.
- New logic goes in `detect/` when it is pure — it must then be unit
  tested, and it carries a **75% line coverage floor per module**. No
  filesystem there.
- **Nothing this tool prints is ever non-ASCII**, including `file` and
  `key`, which it does not author and therefore *escapes* rather than
  sanitises — `escape::json` on the **serialized document**, never on
  the field, because `serde_json` escapes a backslash you added and the
  value stops round-tripping. Practical consequence: **no em dashes, no
  curly quotes, no arrows in any `detail` or refusal string.** Doc
  comments may have them; anything reaching a stream may not. This has
  already broken once.
- **A finding never carries the character it found.** `codepoints` and
  `resembles` are `U+XXXX`; a report that pasted a raw U+202E would
  reorder the terminal, the diff and the ticket of whoever read it.
- **Declaring a script turns the checks ON, never off.** The
  `intentional_script_context` share is measured over the **undeclared**
  scripts alone. Getting this backwards disabled the homoglyph check for
  every file in an internationalised repository — precisely where one
  would hide. `scripts::judge` got the direction wrong twice, so
  `declaring_a_script_never_adds_a_finding` states the property on its
  own, and `compatibility` does not take the declared scripts as a
  parameter so nothing can gate it by them.
- **A format decides how a finding is addressed, never whether it
  exists.** If a change to `detect/locate.rs` or a reader alters a
  finding, the change is wrong.
- **Nothing is rewritten.** No `--fix`, no normalization, no stripping —
  and a crate that rewrites text has no place in the dependency list
  whatever it detects on the way.
- **Refuse rather than guess.** Each of the three refusal reasons exists
  because the honest answer was "I did not judge this", and each says
  what *did* run.
- **`fixtures/documents/`'s three translations — `zh-cn.json`, `ru.json`,
  `ja.json` — are the most important files in the repository.** Two tests
  guard them and both are needed: without
  `a_declared_translation_is_judged_and_is_still_clean`, the other passes
  for the wrong reason, because a refusal suppresses the checks and "no
  findings" would mean "not judged". Changing a document or an
  expectation is a behavior change and needs a CHANGELOG entry.
- **Columns are UTF-16 code units and the lookup is checkpointed.**
  Counting from the line start per lookup is quadratic on a minified
  bundle. Prove a change with `tests/budget.rs`; do not reason about it.
- The gated suites are opt-in and CI sets them: `UNICODE_LE_BUDGET=1`,
  `UNICODE_LE_FUZZ_SECONDS`, `UNICODE_LE_FUZZ_SEED`,
  `UNICODE_LE_SCENARIOS=1`. A skipped case says so by name; a skip is
  never reported as a pass. `cargo test <filter>` exits 0 when the filter
  matches nothing, which is why `coverage-matrix` prints markers and the
  job greps for them.
- Write a regression test for every bug you fix, and **observe it fail
  before the fix**. The quadratic above was proved by reverting the fix
  and watching the budget test report 13.02x against its 6x limit.
