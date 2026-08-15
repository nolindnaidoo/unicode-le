# unicode-le — Rust specification

A CLI and MCP server that scans a tree for the Unicode characters that
hide meaning: the Trojan Source bidirectional controls, invisibles,
homoglyphs, words that mix scripts, text that is not in NFC, spaces that
are not the space, and codepoints with no assigned meaning.

It sits on both sides of the LE thesis. As a **security screen** it
answers CVE-2021-42574. As a **data-prep cleanliness pass** it answers
why one row of a CSV never matches another that looks identical.

## The one question

**Which characters in this tree are not what they look like?**

Asked over a whole tree, answered without rewriting a byte, with an exit
code a CI step can fail on.

---

## Discovery: what already exists

Checked before a line was written, because UAX #39 is a **data**
standard and hand-copying its tables into this crate would be a
maintenance liability rather than a feature.

| Crate | Version | Last release | Licence | Verdict |
|---|---|---|---|---|
| [`unicode-security`](https://crates.io/crates/unicode-security) | 0.1.2 | 2024-09-12 | MIT/Apache-2.0 | **Depend.** It *is* UAX #39. |
| [`unicode-script`](https://crates.io/crates/unicode-script) | 0.5.8 | 2025-12-03 | MIT/Apache-2.0 | **Depend.** |
| [`unicode-normalization`](https://crates.io/crates/unicode-normalization) | 0.1.25 | 2025-10-30 | MIT/Apache-2.0 | **Depend.** |
| [`unicode-bidi`](https://crates.io/crates/unicode-bidi) | 0.3.18 | 2024-12-16 | MIT/Apache-2.0 | **No.** |
| [`unicode-segmentation`](https://crates.io/crates/unicode-segmentation) | 1.13.3 | 2026-06-01 | MIT/Apache-2.0 | **No.** |
| [`decancer`](https://crates.io/crates/decancer) | 3.3.3 | 2025-07-16 | MIT | **No.** |

**`unicode-security` covers most of the detection surface, and this
crate depends on it rather than duplicating it.** It ships the
confusable prototypes (`skeleton`), the augmented script sets and
single-script resolution (`MixedScript`), the potential-mixed-script
confusable predicate, the restriction levels and the identifier profile
— six thousand-odd data mappings that move with every Unicode release.
Reimplementing any of that here would buy nothing and owe a debt
forever. It is by unicode-rs, permissively licensed, and carries 12.5M
downloads.

Its last release is September 2024 and its tables are Unicode 16.0,
while `unicode-script` is on 17.0. **Written down rather than glossed:**
a confusable pair added in Unicode 17 is not detected, and the
`unassigned-or-private-use` check — which asks `unicode-script`, not
`unicode-security` — is a release ahead of the confusable check. Neither
is a correctness bug; both are the cost of standing on maintained data
instead of copying it, and both improve when upstream does.

The three rejected crates:

- **`unicode-bidi`** implements the *bidirectional algorithm* — how to
  lay text out. This tool needs to know that a control character is
  present, which is a ten-entry table and a `char` comparison. Pulling
  in a layout engine to answer a membership test would be a dependency
  with no work to do.
- **`unicode-segmentation`** implements UAX #29 word segmentation, which
  splits `snake_case` into three words and would judge each fragment
  separately — precisely the mixing this has to see. A word here is
  "letters, digits, underscores and combining marks", which is an
  identifier in every language this is pointed at.
- **`decancer`** *removes* confusables: it takes text and hands back
  ASCII. That is the opposite operation. This tool never rewrites
  anything, and a library whose purpose is rewriting has no use here.

**What is left, and what this crate is:** the LE layer. Walk a tree,
decode or refuse, locate a risk at a file, line, column and byte offset,
classify it, keep the report safe to read, and refuse to judge what it
cannot judge honestly. None of that is in any of the crates above.

---

## The report-safety rule

**A finding never carries the character it found.** `codepoints` and
`resembles` are `U+XXXX` strings; `detail` is English prose written by
this crate. There is no field holding source text and no context line,
and `detect::hazards` asserts over the whole corpus that a serialized
report is pure ASCII.

**Two fields are not written by this crate**, and they are the ones that
made the rule incomplete: `file` is the path the caller supplied, and
`key` is text out of the document being scanned. Both must be carried
exactly — a path has to open, and a key path has to match the reader's
own document — so neither can be rewritten.

They are **escaped rather than rewritten**: every non-ASCII codepoint in
anything this tool prints is emitted as `\uXXXX`. Because that is JSON's
own escape, both halves hold at once.

- **Inert on the wire.** The raw bytes of stdout, of an MCP frame, of a
  diff or of a pull request contain no character that can reorder
  anything. A repository holding a file named `invoice`, U+202E,
  `fdp.ts` used to produce a report that reordered the terminal of
  whoever read it.
- **Unchanged for a machine.** A parser decodes the escape back to the
  identical string, so the path in the report still opens the file it
  names and the key still matches the document.

The escape is applied to the *serialized document*, not to the field.
Escaping first does not survive serialization: `serde_json` escapes the
backslash it finds, and the value ships as `\\u202E` and parses back as
six literal characters. Applying it afterwards is sound because JSON's
own syntax is ASCII — a non-ASCII codepoint can only occur inside a
string literal, which is exactly where `\uXXXX` is defined. Astral
codepoints become a surrogate pair, because the escape is sixteen bits.

On stderr the same spelling is used, so a path or a key in the human
summary can be grepped for with the one in the report.

This is specific to this tool and it is not fussiness. Every sibling can
quote what it found, because a regex or a file path is inert on the
page. Here it is not: a report that pasted a raw U+202E reorders the
terminal, the diff, the pull request and the chat window of whoever
reads it. The tool would become the delivery mechanism for the thing it
detects, and it would work best against the person doing the review. The
same applies to a zero-width joiner pasted into a commit message and a
homoglyph pasted into a ticket title.

`U+202E` is inert, greppable, and searchable in any Unicode reference.

---

## Findings

Seven kinds. Every finding carries `kind`, `severity`, `line`, `column`,
`offset`, `codepoints`, `scripts`, and `detail`; confusables also carry
`resembles`, and a finding in a document whose format is readable also
carries `key`.

| kind | severity | what |
|---|---|---|
| `bidi-control` | high | U+202A–U+202E, U+2066–U+2069, U+061C. The Trojan Source class, CVE-2021-42574: these reorder how the rest of the line renders, so the text a reviewer reads is not the text that runs. |
| `confusable` | high | A homoglyph. Either a character from another script inside a word that is not in that script (Cyrillic `а` in `pаypal`), or a compatibility form of an ASCII character (full-width `Ｆ`, mathematical `𝐚`, the Kelvin sign). `resembles` carries what it reduces to. |
| `mixed-script` | high | One word that no single script accounts for, under UTS #39's resolved script set. |
| `invisible` | medium | U+200B–U+200D, U+2060, U+00AD, U+180E, and U+FEFF anywhere but the first byte. |
| `unassigned-or-private-use` | medium | A codepoint with no assigned script: private-use, unassigned, a noncharacter. |
| `non-nfc` | low | A line that is not in Normalization Form C. The form is reported; **the file is not rewritten**. |
| `unusual-whitespace` | low | U+00A0, U+3000, U+1680, U+2000–U+200A, U+202F, U+205F: renders like a space, is not one. |

**Severity does not rank `bidi-control` above the other two highs.**
Three levels cannot express "this one is the CVE", so the mechanism that
does is `--fail-on bidi`, which fails a run on the Trojan Source class
and nothing else.

### Notes on particular findings

**A leading U+FEFF is the encoding, not an invisible.** The same
character anywhere else is a zero-width no-break space sitting inside a
string, and that is a finding. The byte is **not stripped**: the report
carries byte offsets into the file on disk, and deleting three bytes
from the front would make every offset in the report wrong by three.

**`non-nfc` is one finding per line**, positioned at the first character
where the line and its NFC form part company. Per file would give a
position nobody can act on; per character would report every letter of a
decomposed document. The form reported is `NFD` or `neither NFC nor
NFD` — NFKD is a subset of NFD, so naming it would be a distinction
without a difference, and a *mixture* is a different problem from a
systematically decomposed file.

**`unassigned-or-private-use` asks the script table.** UAX #24 assigns
`Script::Unknown` to exactly the unassigned, private-use, noncharacter
and surrogate codepoints, so this crate carries no range table that
would need updating every September. The private-use areas themselves
are fixed by the standard and are written down, only to tell the two
reasons apart in the message.

**The whitespace and normalization checks are not gated by script.** A
no-break space is the same character in a Chinese file as in an English
one. `--kind` is how a caller who wants only the security screen
narrows it.

---

## Key paths

A line and a column point at a place in a file. `metrics.headline.eyebrow`
points at a place in the **document**, and for the file this tool is most
often aimed at — a five-thousand-line locale catalogue — that is the
difference between a finding somebody can act on and a line number they
have to go and look up.

Where the format is readable, a finding carries `key`. Where it is not,
the field is **absent** — not empty, because a consumer branching on
presence would take one for the other.

| format | resolved from | key path looks like |
|---|---|---|
| `json` | `.json`, `.jsonc` | `metrics.headline.eyebrow`, `rows.[2].id` |
| `yaml` | `.yaml`, `.yml` | `service.display.label`, `items.[0].name` |
| `toml` | `.toml` | `server.limits.note` |
| `ini` | `.ini`, `.cfg`, `.conf`, `.properties` | `database.host` |
| `env` | `.env` | `API_HOST` |
| `csv` | `.csv` | the column's header name, or `[3]` |
| `tsv` | `.tsv` | the same, split on tabs |
| `text` | everything else | absent |

**The format decides how a finding is addressed and never whether it
exists.** This is the inversion the whole layer rests on, and it is the
opposite of what a format-aware *extractor* does: in `numbers-le` the
format decides what counts, because `"42"` is a string in JSON and a
number in `.env`. Nothing of that kind happens here — a right-to-left
override is the same override however the file around it is punctuated.
So a document whose format cannot be parsed, or whose extension nothing
recognises, is still scanned and still reports every finding in it,
without a key. A test runs one document through every reader and asserts
the findings are identical each time.

The readers are **line scanners, not parsers**. That keeps the offsets
the raw document's, which is what the whole report is indexed by, and it
means a truncated or invalid document still yields the key paths it did
manage to read. Each reader's limits are written down in its own module:
YAML flow style, anchors and multi-line scalars are not modelled; a TOML
array spread over several lines has its key on the first; the index of a
TOML array-of-tables is deliberately absent rather than guessed.

Array elements are `[0]`, `[1]`, joined by the same rule as every other
segment: `orders.[2].id`.

An empty path is the document's root, which names nothing, and is
reported as no key at all.

### The escape-sequence false positive, resolved for JSON

A backslash escape immediately followed by non-Latin text used to make a
mixed word: `"Hello\nПривет"` contains the byte run `nПривет`, which is
Latin `n` against Cyrillic with nothing between.

**Inside a JSON string this is no longer reported**, because there the
grammar settles it: `\n` is a line feed, so the `n` is not a letter and
the two scripts never touch. The JSON reader hands the word splitter the
byte ranges of the escape sequences it found, and no word is built
across one. A `\uXXXX` escape is covered across all six characters, so
the hex digits cannot glue two words together either.

**In every other format it is still reported, and that is deliberate.**
The bytes really are a Latin letter followed by Cyrillic; a backslash is
an ordinary character in a `.txt` file, where `C:\Привет` is a path.
Only a reader that knows the escape rule may resolve it, and nothing
here guesses one. `--kind` narrows it away for a caller who only wants
the security screen.

---

## The false-positive guard

**This is the design, not a filter.** A confusable check that fires on
ordinary Russian, Greek or Chinese text produces thousands of findings
on exactly the internationalised repositories that most need the check.
It gets switched off within a day, and the Trojan Source screen goes off
with it. Three rules:

**1. A word is judged, never a file.** The UTS #39 resolved script set
is computed per word. `Привет` is wholly Cyrillic and is Russian. The
Cyrillic `а` in `pаypal` sits in a word that is otherwise Latin, and only
that one is a finding. UTS #39 also *augments* the set — Han with
Hiragana and Katakana is Japanese, Han with Hangul is Korean — so
ordinary Japanese prose is one script rather than three.

**2. A file plainly written in another script is refused, not guessed
at.** When at least 10% of a file's letters belong to an undeclared
non-Latin script, the confusable and mixed-script checks do not run and
the file carries an `intentional_script_context` refusal saying so.
Every other check still runs on it.

*The ten percent was measured, not picked.* The thinnest real case is a
`zh-CN` UI catalogue, where the keys are ASCII and the Han values are
shorter than the English they replace: 16% of its letters are Han. A
single foreign word quoted in an English document is a tenth of one
percent. Ten sits comfortably between them.

*The share is over the **undeclared** scripts alone*, and the denominator
is every letter in the file. A file that is 98% Japanese and holds six
Cyrillic letters is 5.8% undeclared once Han, Hiragana and Katakana are
named, so it is judged rather than refused — the caller has accounted for
what the file is written in, and a Latin baseline is back. Counting every
non-Latin letter and then naming only the undeclared scripts got both the
number and the outcome wrong, and the outcome was the serious half:
declaring the script a repository is written in switched the homoglyph
check off for every file in it. The refusal survives for what it was for,
a file with **no useful Latin baseline at all**.

The refusal's wording follows the same split. With nothing declared it
says no expected script was declared and what to name; with something
declared it says none of the scripts it found is among them, because
telling a caller they declared nothing when they did is false and sends
them to check a flag they already passed.

**3. A declared script is an expected script.** `--script Han` does more
than lift the refusal: it changes what counts. CJK is written without
spaces, so `CSVストリーミング` is one word under any segmentation, and a
caller who has said the tree contains Han should not then be told that
every string mentioning a product name mixes scripts. Mixing Latin with
a *declared* script is a translation; mixing it with an **undeclared**
one is still a finding in the same file, which is what makes declaring
worth doing rather than equivalent to switching the check off. A
compatibility form is not a script question and is reported either way.

*This rule was found by running the tool, not by reasoning about it.*
Without it, declaring the scripts on two real locale trees produced 102
findings — every one of them a product name inside a translated string.

**Dogfooded**, read-only, against `numbers-le/src/i18n` (12 locale
files) and a 25-locale mobile app catalogue:

| | files | findings | refusals |
|---|---|---|---|
| no script declared | 37 | **0** | 12 (`intentional_script_context`) |
| scripts declared | 37 | **0** | 0 |

Both halves matter. The first says the guard holds; the second says it
holds because the checks *ran and found nothing*, not because a refusal
suppressed them.

### The one false positive left, written down

A backslash escape immediately followed by non-Latin text makes a mixed
word: in a Rust, JavaScript or Python source file, `"Hello\nПривет"`
contains the byte run `nПривет`, which is Latin `n` plus Cyrillic and is
reported as `mixed-script`.

**It is resolved for JSON and left alone everywhere else** — see "The
escape-sequence false positive, resolved for JSON" above. The bytes
really are a Latin letter followed by Cyrillic with nothing between
them; a backslash is an ordinary character in a `.txt` file, where
`C:\Привет` is a path. Resolving it needs a grammar that says so, and
this crate has one for JSON and for no other format it reads. Inferring
which of a `.rs` file's backslashes are escapes would be exactly the
guessing the rest of this specification refuses. `--kind` narrows it
away for a caller who only wants the security screen.

---

## Refusals

A refusal is a first-class result, not an error: the run carries on, the
reader is told what was not covered, and nothing is reported clean that
was never looked at.

| reason | when | what still ran |
|---|---|---|
| `binary_or_undecodable` | a NUL byte in the first 8 KB, bytes that are not valid UTF-8, or a file that could not be read | nothing |
| `encoding_unknown` | a UTF-16 or UTF-32 byte-order mark | nothing |
| `intentional_script_context` | ≥10% of the file's letters belong to a non-Latin script that was **not** declared | everything except the confusable and mixed-script checks |

**No encoding is ever guessed.** This matters more here than in any
sibling: read a UTF-16 file as UTF-8 or Latin-1 and every second byte
looks like a NUL or a stray high byte, so a tool that guessed would
report a document full of invisible and unassigned characters that are
not in the file. A confident, detailed, fabricated answer is the worst
failure available to a security screen. The order is byte-order mark
first, binary sniff second, UTF-8 last — the mark before the sniff,
because a UTF-16 file carries NUL bytes too and sniffing first would
name the wrong reason.

An I/O error (a permission problem, a file that vanished) is reported
under `binary_or_undecodable` with the operating system's message. The
reason name is broad enough to hold it and the detail says exactly what
happened; a fourth reason for one case was not worth the API.

`summary.unexamined` counts the files where **nothing** was read, which
is a different number from `summary.refusals`. A file refused for its
script context was still screened for bidi controls and invisibles.

---

## Output contract

**stdout is protocol, stderr is human. There is no `--json` flag.** One
JSON document for the whole run, `schema: 1`, no timestamp — two runs
over an unchanged tree produce identical bytes, which is most of what a
report in CI is for.

One document rather than JSON Lines, unlike `regex-le`: the interesting
numbers here are totals — how many files were refused, whether any
finding is a bidi control — and a consumer reading lines has to
accumulate those itself, slightly differently each time.

```json
{
  "schema": 1,
  "files": [
    {
      "file": "src/auth.ts",
      "findings": [
        {
          "kind": "confusable",
          "severity": "high",
          "line": 2,
          "column": 8,
          "offset": 70,
          "key": "auth.provider",
          "codepoints": ["U+0430"],
          "scripts": ["Cyrillic"],
          "resembles": ["U+0061"],
          "detail": "a Cyrillic character in a word that is not Cyrillic, and it reduces to the codepoint under `resembles`: the two are indistinguishable on screen"
        }
      ],
      "refusals": [],
      "summary": { "findings": 1, "bidi": 0, "refusals": 0 }
    }
  ],
  "summary": { "files": 1, "findings": 1, "bidi": 0, "refusals": 0, "unexamined": 0 }
}
```

**Columns are UTF-16 code units, 1-based.** An editor reports UTF-16
columns, and the whole job of this tool is pointing at one character in a
file the reader is about to open. Byte counting and scalar counting both
diverge from that, in opposite directions, on exactly the characters this
finds: an invisible U+2060 is three bytes and one code unit; a
mathematical U+1D41A is four bytes, one scalar and **two** code units.
The byte offset is carried alongside for callers that address the file
rather than the editor.

**Paths are separated by `/` on every platform**, including Windows. A
report is diffed against one produced on another machine and read by
someone who does not have the tree, so a separator that moves with the
operating system makes every line differ for no reason a reader can see.
The one exception is a refusal naming a path the caller supplied, which
comes back exactly as typed: a message that rewrites its own path cannot
be grepped for by the person who typed it.

### Exit codes are the API

- **0** — clean.
- **1** — at least one finding that `--fail-on` counts.
- **2** — the question was malformed: an unknown flag, an unknown kind
  or script, a path that does not exist. Also `--strict` with any
  refusal.

`--fail-on` defaults to `any`; `bidi` counts only the Trojan Source
class, for a pipeline that wants the security screen without the
cleanliness pass.

`--strict` turns any refusal into exit 2. It is off by default because a
check that exits 2 on every repository holding a PNG is a check nobody
puts in CI; it exists because otherwise there is no way to insist that a
scan covered what it was pointed at. On an internationalised tree that
means naming `--script`, which is the point.

---

## The CLI surface

```
usage: unicode-le [options] <file|dir>...
       unicode-le [options] --stdin
       unicode-le mcp
       unicode-le --version | --help

Options:
  --kind <kind>     report only these kinds (repeatable, comma-separated):
                    bidi, invisible, confusable, mixed-script, non-nfc,
                    whitespace, unassigned
  --script <tag>    a non-Latin script this tree is expected to contain,
                    by Unicode name or ISO 15924 tag (repeatable,
                    comma-separated), e.g. Han or Cyrillic
  --fail-on <what>  what exits 1: any finding, or bidi for the Trojan
                    Source class alone (default any)
  --strict          exit 2 if any file was refused
  --stdin           read one document from stdin
  --hidden          walk hidden files and directories too
  --no-ignore       walk files that .gitignore excludes
```

`--script` takes a full UAX #24 name or an ISO 15924 tag, case
insensitively: `Han`, `han`, `Hani`, `caucasian-albanian`. `Common`,
`Inherited` and `Unknown` parse and would then do nothing, so they are
refused rather than silently accepted.

### What is walked

Every text file. There is no format filter — a bidi control is a bidi
control in a `.md`, a `.json` and a file with no extension, and the
Trojan Source paper's own examples are ordinary source files. A
directory is walked the way ripgrep walks one: `.gitignore` honoured,
hidden files skipped, `--no-ignore` and `--hidden` to reach the rest. A
file named explicitly is always read.

---

## The MCP surface

- **`detect_unicode_risks`** — a document in, findings and refusals out.
  Touches no filesystem: an agent already has file-read tools, and
  duplicating them here would add a path-traversal surface for no
  capability. This is the tool that matters most for this crate: a model
  that pastes a document into its own reasoning has already been handed
  the bidi controls in it, and what comes back from here is `U+XXXX` and
  English, so the *answer* cannot carry the attack onward into a commit
  message or a review comment. It takes `format` or `filename` — a
  document arriving here has no name of its own, so one of those is the
  only way to ask for key paths. Neither opens anything, and a name
  nothing recognises costs the key paths and no findings.
- **`unicode_le_scan`** — files or directories in, the same report the
  CLI writes.

Every tool returns `{ ok, data, diagnostics, meta }`, where `ok` means
the check ran and never that the answer was yes. A refusal comes back
twice — structured in `data.refusals` and as a `warning` diagnostic — so
a model reading only the text cannot take an empty finding list for a
clean tree.

**Refusals speak the caller's vocabulary.** An MCP caller has no command
line, so no message on that surface names a flag; a test asserts no MCP
output contains `--`.

---

## Non-goals

- **It never rewrites or normalizes a file.** Not with a flag, not with
  a tool, not ever. Normalizing is a content change with consequences
  this tool cannot see: a fixture may be NFD on purpose, and a test that
  pins a decomposed sequence fails the moment something helpfully
  composes it. The form is reported; what to do about it is a decision
  with context this tool does not have.
- **It does not strip anything.** Deleting a zero-width joiner from a
  string is a change to data, and a scanner that edits is a scanner
  nobody can run on a repository they do not own.
- **It never quotes what it found.** See the report-safety rule.
- **It does not guess an encoding.**
- **It does not prove text safe.** A codepoint that is not in its tables
  and a confusable pair added after Unicode 16 will not be flagged.
  Silence is not a clearance.
- **No network, ever.**

## Not in v1

- **A baseline file** for accepting known findings.
- **Per-path script expectations** — `--script` applies to the whole
  run. A `.unicode-le` file naming the expected script per directory is
  the obvious next step and is deliberately not guessed at here.
- **Restriction-level reporting.** `unicode-security` computes the UTS
  #39 restriction level and this crate does not surface it; it is an
  identifier-policy question, and no finding needs it yet.
- **Grapheme-cluster columns.** Columns are UTF-16 code units, which is
  what an editor shows.
