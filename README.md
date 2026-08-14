<p align="center">
  <img src="https://raw.githubusercontent.com/nolindnaidoo/unicode-le/main/assets/icon.png" alt="Unicode-LE logo" width="96" height="96"/>
</p>
<h1 align="center">Unicode-LE: The Characters That Are Not What They Look Like</h1>
<p align="center">
  <b>Scan a tree for the Unicode that hides meaning — and never see it quoted back at you</b><br/>
  <i>Trojan Source bidi controls, invisibles, homoglyphs, mixed scripts, non-NFC text, spaces that are not the space</i>
</p>

<p align="center">
  <a href="https://crates.io/crates/unicode-le">
    <img src="https://img.shields.io/crates/v/unicode-le?style=for-the-badge&label=Rust%20CLI&color=blue&logo=rust" alt="unicode-le on crates.io" />
  </a>
  <a href="https://crates.io/crates/unicode-le">
    <img src="https://img.shields.io/crates/d/unicode-le?style=for-the-badge&label=Downloads&color=blue" alt="crates.io downloads" />
  </a>
  <a href="https://github.com/nolindnaidoo/unicode-le/actions/workflows/ci-crate.yml">
    <img src="https://img.shields.io/github/actions/workflow/status/nolindnaidoo/unicode-le/ci-crate.yml?branch=main&style=for-the-badge&label=CI&color=blue&logo=githubactions&logoColor=white" alt="CI" />
  </a>
  <a href="https://github.com/nolindnaidoo/unicode-le/blob/main/crate/Cargo.toml">
    <img src="https://img.shields.io/badge/rustc-1.88+-blue?style=for-the-badge&logo=rust" alt="MSRV: Rust 1.88+" />
  </a>
  <a href="https://github.com/nolindnaidoo/unicode-le/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/license-MIT-blue?style=for-the-badge" alt="MIT licensed" />
  </a>
  <a href="https://letools.dev/tools/unicode-le">
    <img src="https://img.shields.io/badge/LE%20Tools-letools.dev-blue?style=for-the-badge" alt="LE Tools" />
  </a>
</p>

---

<p align="center">
  <img src="https://raw.githubusercontent.com/nolindnaidoo/unicode-le/main/assets/demo.gif" alt="Unicode-LE demo — the real binary, recorded by assets/demo.tape" style="max-width: 100%; height: auto;" />
</p>

> **Useful?** A star is how other developers find it —
> [★ GitHub](https://github.com/nolindnaidoo/unicode-le) ·
> [letools.dev/tools/unicode-le](https://letools.dev/tools/unicode-le)

A right-to-left override that makes a reviewer read an `if` guard that is
not there. A Cyrillic `а` in `pаypal`. A zero-width space between two
strings that a hash says are different and a person says are the same. A
no-break space where a `split(' ')` expects a space.

One command over a whole tree. Nothing is rewritten, and **nothing it
prints can render as anything** — findings carry `U+XXXX`, never the
character, and even a file name or a key that holds a bidi control comes
out as `\uXXXX`. A report that pasted one raw would reorder the
terminal, the diff and the pull request of whoever read it. Because
`\uXXXX` is JSON's own escape, a parser still decodes the path back to
the file it opens.

## Sixty seconds

```
$ unicode-le .
./README.md:1:9  [low] unusual-whitespace U+00A0  no-break space: renders like a
  space and is not one, so a trim, a split or a comparison against U+0020 does
  not see it
./src/auth.ts:1:7  [high] mixed-script U+0430  one word written in Cyrillic and
  Latin: no single script accounts for it, which is how a name that reads as
  familiar is forged
./src/auth.ts:1:8  [high] confusable U+0430  a Cyrillic character in a word that
  is not Cyrillic, and it reduces to the codepoint under `resembles`: the two
  are indistinguishable on screen
./src/render.ts:1:16  [high] bidi-control U+202E  right-to-left override: a
  bidirectional control reorders how the rest of the line renders, so the text
  a reviewer reads is not the text that runs
4 findings in 4 files
```

(Long lines wrapped here for the page; the tool writes one line per
finding. The file order is the walk's, which is sorted, so two runs over
an unchanged tree produce the same bytes.)

stdout is one JSON document; stderr is what you see above.

```bash

# in CI, for the CVE and nothing else:
unicode-le --fail-on bidi .
```

## Install

```bash
cargo install unicode-le
```

Or build it from source:

```bash
git clone https://github.com/nolindnaidoo/unicode-le
cd unicode-le/crate
cargo build --release
./target/release/unicode-le --help
```

## It runs on internationalised code without drowning you

This is the part that decides whether the tool survives contact with a
real repository. A naive confusable check flags every letter of every
Russian and Chinese string in your tree — thousands of findings on
exactly the codebases that most need the check, so it gets switched off,
and the Trojan Source screen goes off with it.

**A word is judged, never a file.** `Привет` is wholly Cyrillic and is
Russian. The Cyrillic `а` in `pаypal` sits in a word that is otherwise
Latin, and only that one is a finding. Japanese mixes Han, Hiragana and
Katakana in one word constantly, and UTS #39 knows that is Japanese.

**A file plainly written in another script is refused, not guessed at.**
Your `zh-CN.json` gets a refusal saying the confusable check did not run
on it and why — not four hundred findings. Every other check still did.

**Naming the script judges it properly.** `--script Han` says Han is
expected here, so a Latin product name inside a Chinese string is a
translation rather than a finding — while a Cyrillic letter in a Latin
word *in that same file* is still caught. Declaring is how you turn the
check on, never how you turn it off.

Reproduce it against the three translation fixtures the crate ships —
Chinese, Russian and Japanese:

```bash
unicode-le fixtures/documents/{zh-cn,ru,ja}.json
unicode-le --script Han,Hiragana,Katakana,Cyrillic fixtures/documents/{zh-cn,ru,ja}.json
```

| | findings | refusals |
|---|---|---|
| no script declared | **0** | 3 — `intentional_script_context`, naming the share it measured |
| `--script Han,Hiragana,Katakana,Cyrillic` | **0** | **0** — every check ran |

Both numbers matter. The second says the checks *ran* and found nothing.

## It says where in the document, not just where in the file

```
$ unicode-le locales/
locales/en.json:412:19  [high] bidi-control U+202E  at metrics.headline.eyebrow
  right-to-left override: a bidirectional control reorders how the rest of the
  line renders, so the text a reviewer reads is not the text that runs
```

A line number in a five-thousand-line catalogue is something you have to
go and look up. `metrics.headline.eyebrow` is the thing you were looking
for.

| format | from | key path |
|---|---|---|
| JSON | `.json`, `.jsonc` | `metrics.headline.eyebrow`, `rows.[2].id` |
| YAML | `.yaml`, `.yml` | `service.display.label` |
| TOML | `.toml` | `server.limits.note` |
| INI | `.ini`, `.cfg`, `.conf`, `.properties` | `database.host` |
| dotenv | `.env` | `API_HOST` |
| CSV | `.csv`, `.tsv` | the column's header name |
| anything else | — | no key, same findings |

**The format never decides whether a finding exists**, only how it is
addressed — the opposite of what a format-aware extractor does. A file
whose format cannot be parsed is still scanned and still reports
everything in it; a truncated document still yields the key paths it did
manage to read. A test runs one document through every reader and
asserts the findings come back identical each time.

One thing the format does decide: inside a JSON string, `\n` is an
escape and not the letter `n`, so `"Hello\nПривет"` is no longer
reported as a word that mixes scripts. In a `.txt` file it still is,
because there a backslash is an ordinary character and `C:\Привет` is a
path.

## It never rewrites your files

No `--fix`. No normalization. No stripping. The form your text is in is
**reported**; what to do about it is a decision with context this tool
does not have — your NFD fixture may be NFD on purpose, and a test that
pins a decomposed sequence breaks the moment something helpfully
composes it.

It also cannot prove text safe. A confusable pair added after Unicode
16.0 is not in the tables it stands on. Silence is not a clearance.

## What it finds

| kind | severity | what |
|---|---|---|
| `bidi-control` | high | The Trojan Source class, [CVE-2021-42574](https://trojansource.codes/). U+202A–U+202E, U+2066–U+2069, U+061C. |
| `confusable` | high | A homoglyph: Cyrillic `а` for `a`, Greek `ο` for `o`, full-width `Ｆ`, mathematical `𝐚`. Reports what it resembles. |
| `mixed-script` | high | One word that no single script accounts for, under UTS #39's resolved script set. |
| `invisible` | medium | Zero-width space, joiner and non-joiner, word joiner, soft hyphen, an interior BOM, the Mongolian vowel separator. |
| `unassigned-or-private-use` | medium | Private-use, unassigned, noncharacter. |
| `non-nfc` | low | A line that is not in Normalization Form C, with the form it is in. |
| `unusual-whitespace` | low | No-break space, ideographic space, and the rest of the spaces that are not the space. |

Severity does not rank `bidi-control` above the other two highs — three
levels cannot say "this one is the CVE". `--fail-on bidi` does.

Every finding carries a file, a 1-based line and **UTF-16 column** (what
your editor's ruler shows), a byte offset, the codepoints as `U+XXXX`,
the scripts involved and a severity. Confusables also carry `resembles`,
and a finding in a document whose format is readable carries `key`.

## It refuses rather than guessing

A refusal is a first-class result, not an error: the run carries on, and
nothing is reported clean that was never looked at.

| reason | when | what still ran |
|---|---|---|
| `binary_or_undecodable` | a NUL byte in the first 8 KB, bytes that are not UTF-8, or a file it could not read | nothing |
| `encoding_unknown` | a UTF-16 or UTF-32 byte-order mark | nothing |
| `intentional_script_context` | at least 10% of the letters belong to an **undeclared** non-Latin script | everything except the confusable and mixed-script checks |

**No encoding is ever guessed.** Read a UTF-16 file as UTF-8 and every
second byte looks like a NUL: a tool that guessed would report a file
full of invisible and unassigned characters that are not in it. A
confident, detailed, fabricated answer is the worst thing a security
screen can produce.

`summary.unexamined` counts the files where **nothing** was read, which
is a different number from `summary.refusals` — a file refused for its
script context was still screened for bidi controls and invisibles.

Refusals do not fail the run. `--strict` makes them exit 2, for a
pipeline that wants to insist the scan covered what it was pointed at.

## Exit codes are the API

- **0** — clean.
- **1** — at least one finding that `--fail-on` counts.
- **2** — the question was malformed: an unknown flag, an unknown kind or
  script, a path that does not exist. Also `--strict` with any refusal.

## Options

```
usage: unicode-le [options] <file|dir>...
       unicode-le [options] --stdin
       unicode-le mcp
       unicode-le --version | --help

  --kind <kind>     bidi, invisible, confusable, mixed-script, non-nfc,
                    whitespace, unassigned (repeatable, comma-separated)
  --script <tag>    a non-Latin script this tree is expected to contain,
                    by Unicode name or ISO 15924 tag: Han, Cyrillic, Hani
  --fail-on <what>  any (default) or bidi
  --strict          exit 2 if any file was refused
  --stdin           read one document from stdin
  --hidden          walk hidden files and directories too
  --no-ignore       walk files that .gitignore excludes
```

Every text file is walked — there is no format filter, because a bidi
control is a bidi control in a `.md`, a `.json` and a file with no
extension. A directory is walked the way ripgrep walks one; a file named
explicitly is always read.

## As an MCP server

```bash
unicode-le mcp
```

Two tools over stdio:

- **`detect_unicode_risks`** — a document in, findings out. No
  filesystem. Worth pointing at explicitly: a model that reads a file
  itself has already swallowed the bidi controls in it. What comes back
  from here is `U+XXXX` and English, so the answer cannot carry the
  attack into a commit message or a review comment.
- **`unicode_le_scan`** — files or directories in, the same report the
  CLI writes.

Both return `{ ok, data, diagnostics, meta }`, where `ok` means the check
ran — never that the answer was yes. Refusals speak the caller's
vocabulary: an MCP caller has no command line, so no message on that
surface names a flag, and a test asserts none contains `--`.

## Documentation

| What | Where |
|---|---|
| What the tool is allowed to say — scope, output contract, refusals, non-goals | [`crate/SPEC.md`](crate/SPEC.md) |
| How the code is written and held together — architecture, invariants, the gates | [`crate/AGENTS.md`](crate/AGENTS.md) |
| The crate's own front page | [`crate/README.md`](crate/README.md) |
| What changed | [CHANGELOG.md](CHANGELOG.md) · [`crate/CHANGELOG.md`](crate/CHANGELOG.md) |
| The tool's page, and the other fifteen | [letools.dev/tools/unicode-le](https://letools.dev/tools/unicode-le) |

## More from the LE family

Sixteen single-purpose tools for the work in front of every model. Each ships
a Rust CLI and an MCP server. One page: **[letools.dev](https://letools.dev)**

**Get it out**

- **[String-LE](https://letools.dev/tools/string-le)** — Extract every string in a codebase, with its position, so a person can read them
- **[Numbers-LE](https://letools.dev/tools/numbers-le)** — Extract every hardcoded number in a codebase, so a person can check them
- **[Units-LE](https://letools.dev/tools/units-le)** — Extract every quantity with its unit, normalized, and refuse the ambiguous ones by name
- **[Dates-LE](https://letools.dev/tools/dates-le)** — Extract every date and timestamp, and the exact instant each one resolves to
- **[IDs-LE](https://letools.dev/tools/ids-le)** — Extract every UUID, ULID, NanoID, ObjectId and Snowflake, and decode the time inside
- **[IPs-LE](https://letools.dev/tools/ips-le)** — Extract every IP address, CIDR block and MAC, normalized and classified by scope
- **[URLs-LE](https://letools.dev/tools/urls-le)** — Extract every URL in a codebase, with its protocol and exact position
- **[Paths-LE](https://letools.dev/tools/paths-le)** — Extract every file path in a codebase, and say whether it still points at anything
- **[Colors-LE](https://letools.dev/tools/colors-le)** — Extract every color in a codebase, and say which ones are not in your palette

**Check it**

- **[Regex-LE](https://letools.dev/tools/regex-le)** — Find every regex in a codebase, and report which can be driven into catastrophic backtracking
- **[Versions-LE](https://letools.dev/tools/versions-le)** — Find where one dependency is constrained differently across a repository's manifests
- **[i18n-LE](https://letools.dev/tools/i18n-le)** — Identify the i18n library a project uses, then audit its catalogs by that library's rules
- **[Scrape-LE](https://letools.dev/tools/scrape-le)** — Check whether a page is scrapeable before the scraper is written, and say when it cannot tell

**Guard it**

- **[Secrets-LE](https://letools.dev/tools/secrets-le)** — Find hardcoded credentials in a codebase, and never print one into the report
- **[EnvSync-LE](https://letools.dev/tools/envsync-le)** — Compare the dotenv files in a tree, and say which keys are missing from which
- **[Unicode-LE](https://letools.dev/tools/unicode-le)** — Find the Unicode that hides meaning — bidi controls, invisibles, homoglyphs, mixed scripts

Each stands on its own: no shared crate, no published core. Where two of them
agree, it is because the same answer was right twice.

**Contact** — [nolindnaidoo.com](https://nolindnaidoo.com) · [GitHub](https://github.com/nolindnaidoo) · [LinkedIn](https://www.linkedin.com/in/nolindnaidoo/)

## Also by nolindnaidoo

**Rust** — pixelcoords and pixelactions are one loop: pixelcoords answers
*where*, pixelactions *acts* there. Their own tools, their own voice — not
part of the LE family.

- **[pixelcoords](https://github.com/nolindnaidoo/pixelcoords)** — Freeze your screen, mark regions, get pixel-exact coordinates and crops
  [pixelcoords.dev](https://pixelcoords.dev) · [crates.io](https://crates.io/crates/pixelcoords) · [docs.rs](https://docs.rs/pixelcoords)
- **[pixelactions](https://github.com/nolindnaidoo/pixelactions)** — Consume human-verified coordinates, perform the interaction, confirm it landed
  [pixelactions.dev](https://pixelactions.dev) · [crates.io](https://crates.io/crates/pixelactions) · [docs.rs](https://docs.rs/pixelactions)

## License

MIT © [nolindnaidoo](https://github.com/nolindnaidoo) — see [LICENSE](LICENSE).
