# unicode-le

Find the Unicode characters in a codebase that hide meaning: the Trojan
Source bidirectional controls, invisibles, homoglyphs, words that mix
scripts, text that is not in NFC, spaces that are not the space, and
codepoints with no assigned meaning.

```bash
npx unicode-le src/
npx unicode-le --fail-on bidi . > unicode.json
git diff | npx unicode-le --stdin
```

The command it installs is `unicode-le`, the same command line as the Rust
crate [`unicode-le`](https://crates.io/crates/unicode-le), and it behaves
identically: same flags, same walk, same findings, same output, same exit
codes. Install whichever toolchain you already have.

## What it reports

Codepoints, never the characters themselves: a report that quoted them
would carry the attack to whoever read it. Every non-ASCII character in its
output, a path or a key included, is written as a `\uXXXX` escape. It never
rewrites a file.

A file plainly written in another script is not judged for confusables,
and says so. Name the scripts a tree is expected to contain with
`--script`, such as `--script Cyrillic,Han`, to have those files judged too.

Directories are walked the way ripgrep walks them: `.gitignore`, `.ignore`
and hidden files are respected unless you pass `--no-ignore` or `--hidden`.
It reads UTF-8. A file with a UTF-16 or UTF-32 byte-order mark, a binary
file and one that is not valid UTF-8 are refused rather than guessed at,
and each refusal is reported.

## Output

stdout carries one JSON document whenever it is piped or redirected, and
nothing when it is a terminal. stderr lists every finding.

```text
$ unicode-le src
src/auth.js:1:21  [high] bidi-control U+202E  right-to-left override: a bidirectional control reorders how the rest of the line renders, so the text a reviewer reads is not the text that runs
src/auth.js:2:6  [high] confusable U+0430 (resembles U+0061)  a Cyrillic character in a word that is not Cyrillic, and it reduces to the codepoint under `resembles`: the two are indistinguishable on screen
5 findings in 1 file
```

## Exit codes

- **0**: clean.
- **1**: findings, or with `--fail-on bidi` a bidirectional control alone.
- **2**: the question was malformed, a directory could not be walked, or
  `--strict` and a file was refused.

## For agents

`unicode-le mcp` serves the Model Context Protocol on stdio with two tools:
`detect_unicode_risks` for a document's text and `unicode_le_scan` for
files and directories. Hosts that want the document tool alone can install
[`unicode-le-mcp`](https://www.npmjs.com/package/unicode-le-mcp).

`unicode-le --help` lists every option.

MIT licensed. Part of [letools](https://letools.dev/tools/unicode-le).
