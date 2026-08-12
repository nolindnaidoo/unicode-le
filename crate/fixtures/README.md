# The corpus

Every case here runs as a unit test. The files live inside `crate/`
because `cargo package` cannot reach above its own directory, and
because `cargo test` on the unpacked crate has to be able to reproduce
the claims the README makes.

| File | What it pins |
|---|---|
| `documents/` | The source documents, one per finding kind plus the ones that must produce nothing. |
| `detection.json` → `documents` | Every finding, with its kind, severity, line, UTF-16 column, byte offset, codepoints, scripts and what it resembles — and, for the translations, the refusal instead. |
| `detection.json` → `encodings` | The three files that are not UTF-8 text, and the reason each is refused. |
| `mcp-detect-unicode.json` | The `detect_unicode_risks` MCP tool: the whole `{ ok, data, diagnostics, meta }` envelope, per case. |

## Deliberate contents

- **The documents that must produce *nothing* are the important ones.**
  `zh-cn.json`, `ru.json` and `ja.json` are ordinary translations. A
  confusable check that fires on them fires thousands of times on any
  localised repository, gets switched off, and takes the Trojan Source
  screen with it. Two tests guard them: one asserts no confusable or
  mixed-script finding at all, and one declares the scripts and asserts
  the checks *ran* and still found nothing — without the second, the
  first would pass because a refusal had suppressed them.
- **`ja.json` mixes Han, Hiragana and Katakana inside single words**,
  which is what Japanese does. UTS #39 augments the script set for
  exactly this; without that augmentation every line of it is a
  mixed-script finding.
- **`trojan-source.c` is the example from the paper** (Boucher &
  Anderson, CVE-2021-42574). It renders as an `if (isAdmin)` guard
  around the privileged branch and compiles as an unconditional comment.
  It is here verbatim because a synthesised bidi case would not prove
  the thing worth proving.
- **`clean.ts` holds accented Latin** — `café`, `naïve` — and must come
  back empty. A detector that fires on every European language is as
  broken as one that never fires.
- **`nfd.txt` carries a decomposed line and a composed one**, so the
  check has to discriminate rather than flag the file.
- **`bom.ts` has a byte-order mark in two places.** The leading one is
  the encoding and is not a finding; the interior one is a zero-width
  no-break space inside a string and is.
- **`private-use.txt` carries U+E000 and U+0378** — one private-use, one
  unassigned. They share a kind and must not share a reason.
- **One document per key-path reader**, and each holds a real finding in
  a real structure: `messages.json` (nested three deep, plus an escape
  sequence that must *not* read as a mixed word), `config.yaml`,
  `config.toml`, `settings.ini`, `secrets.env` and `rows.csv`. The
  coverage matrix asserts every offered format is reachable from one of
  them and comes back carrying a key path. That matters more than it
  looks: a lost key path costs no finding by design, so a reader that
  quietly stopped naming anything would pass every other test here.
- **`utf16le.txt`, `binary.dat` and `latin1.txt` are stored as bytes**,
  because that is the whole point of them: a fixture stored as a string
  would already have been decoded, which is the thing being refused.
  `utf16le.txt` also pins that the byte-order mark is read *before* the
  binary sniff — it carries NUL bytes too, and sniffing first would name
  the wrong reason.

## Regenerating

Do not hand-edit `detection.json` or `mcp-detect-unicode.json` to make a
failing test pass. If the change is intended, regenerate them from the
built binary and **read the diff**: an expectation file that is silently
rewritten to match a regression is worse than no corpus at all.
