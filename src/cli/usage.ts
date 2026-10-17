/**
 * The command line's help text and the flags it names.
 *
 * Word for word the Rust CLI's, because both install as `unicode-le` and a
 * person reading `--help` cannot tell which one they have.
 * `scripts/check-cli-parity.ts` compares the two outputs, and `cli.test.ts`
 * holds FLAGS equal to the flags this text names.
 */

export const USAGE = `usage: unicode-le [options] <file|dir>...
       unicode-le [options] --stdin
       unicode-le mcp
       unicode-le --version | --help

Finds the Unicode characters in a tree that hide meaning: the Trojan
Source bidirectional controls, invisibles, homoglyphs, words that mix
scripts, text that is not in NFC, spaces that are not the space, and
codepoints with no assigned meaning.

It reports codepoints, never the characters themselves: a report that
quoted them would carry the attack to whoever read it. It never rewrites
a file: what form your text is in is reported, not corrected.

A file plainly written in another script is not judged for confusables,
and says so. Without being told which script to expect there is no way
to tell a forged name from a translated one, and guessing would bury the
real findings under every word of every translation.

One JSON document on stdout when it is piped or redirected, human
summary on stderr. At a terminal stdout stays silent.

Options:
  --kind <kind>     report only these kinds (repeatable, comma-separated):
                    bidi, invisible, confusable, mixed-script, non-nfc,
                    whitespace, unassigned
  --script <tag>    a non-Latin script this tree is expected to contain,
                    by Unicode name or ISO 15924 tag (repeatable,
                    comma-separated), e.g. Han or Cyrillic
  --fail-on <what>  what exits 1: any finding, or bidi for the Trojan
                    Source class alone (default any). Refused alongside
                    a --kind that excludes what it gates on, because the
                    filter would decide the gate
  --strict          exit 2 if any file was refused, rather than reporting
                    it and carrying on
  --stdin           read one document from stdin
  --hidden          walk hidden files and directories too
  --no-ignore       walk files that .gitignore excludes

Exit codes: 0 clean, 1 findings, 2 the question was malformed.`;

export const FLAGS: readonly string[] = Object.freeze([
	'--kind',
	'--script',
	'--fail-on',
	'--strict',
	'--stdin',
	'--hidden',
	'--no-ignore',
]);
