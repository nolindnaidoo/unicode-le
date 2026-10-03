import {
	examine,
	parseKind,
	parseScript,
	resolveFormat,
	SUPPORTED_FORMATS,
} from '../detection';
import { hasLoneSurrogate } from '../detection/text';
import { KINDS, type Kind } from '../types';
import {
	capped,
	DEFAULT_MAX_RESULTS,
	type Diagnostic,
	envelope,
	MAX_MAX_RESULTS,
	readMaxResults,
	readString,
} from './envelope';
import type { ToolDefinition } from './transport';

/**
 * The tool this server exposes: `detect_unicode_risks`, which the crate's
 * server offers too. One name, one schema, two implementations — a caller must
 * get the same answer whichever it reaches, so the definition below is the
 * crate's, word for word, and `crate/fixtures/mcp-detect-unicode.json` pins the
 * answers on both sides.
 *
 * It touches no filesystem: an agent already has file-read tools. And it
 * matters more here than anywhere in the family, because a model that pasted a
 * document into its own reasoning has already been handed the bidi controls in
 * it. What comes back is `U+XXXX` and English, so the answer cannot carry the
 * attack on into a commit message or a review comment.
 */

const DESCRIPTION =
	'Find the Unicode characters in a document that hide meaning: bidirectional controls (the Trojan Source class, CVE-2021-42574), invisible characters, homoglyphs, words that mix scripts, text that is not in NFC, spaces that are not the space, and unassigned or private-use codepoints. Each finding carries a 1-based line and column, a byte offset, the codepoints as U+XXXX, the script and a severity. It never returns the offending characters themselves, and never rewrites anything. A document plainly written in a non-Latin script is not judged for homoglyphs unless that script is named in `scripts`; it says so in `refusals` rather than guessing.';

/** A list argument, refused by name when it is anything but strings. */
function readStrings(
	args: Record<string, unknown>,
	name: string,
): readonly string[] {
	if (!Object.hasOwn(args, name)) return [];
	const value = args[name];
	if (!Array.isArray(value) || value.some((item) => typeof item !== 'string')) {
		throw new Error(`${name} must be a list of strings`);
	}
	return value as string[];
}

/**
 * The declared format, if any. Refused when it is not a string, and accepted
 * quietly when it names nothing known: the format only decides whether a
 * finding can carry a key path, so an unknown one costs key paths and never a
 * scan.
 */
function readFormat(args: Record<string, unknown>): string | undefined {
	if (!Object.hasOwn(args, 'format')) return undefined;
	if (typeof args.format !== 'string')
		throw new Error('format must be a string');
	return args.format;
}

function detect(args: Record<string, unknown>): Promise<unknown> {
	// The crate's order: content, then the cap, then the options — so a call
	// with two mistakes is refused for the same one by either server.
	const content = readString(args, 'content');
	const maxResults = readMaxResults(args);
	const kinds: Kind[] = readStrings(args, 'kinds').map(parseKind);
	const expectedScripts = readStrings(args, 'scripts').map(parseScript);
	const format = readFormat(args);
	// No `&str` can hold an unpaired surrogate, so the crate cannot be asked
	// this question at all. Refusing it is the honest answer; scanning it would
	// report a character the document does not contain.
	if (hasLoneSurrogate(content)) {
		throw new Error(
			'content holds an unpaired surrogate, which is not text; nothing was scanned',
		);
	}

	const filename =
		typeof args.filename === 'string' ? args.filename : undefined;
	const examination = examine(content, resolveFormat(format, filename), {
		kinds,
		expectedScripts,
	});

	// The `truncated` flag matters more than the cap: a silently incomplete
	// answer is wrong in the most expensive way.
	const { items, truncated } = capped(examination.findings, maxResults);

	// A refusal is carried twice on purpose: structured, so a caller can branch
	// on it, and as a diagnostic, so a model reading the text cannot take an
	// empty finding list for a clean document.
	const diagnostics: Diagnostic[] = examination.refusals.map((refusal) => ({
		severity: 'warning',
		code: refusal.reason,
		message: refusal.detail,
	}));

	return Promise.resolve(
		envelope(
			'detect_unicode_risks',
			{ findings: items, refusals: examination.refusals },
			items.length,
			diagnostics,
			truncated,
		),
	);
}

export const TOOLS: readonly ToolDefinition[] = Object.freeze([
	Object.freeze({
		name: 'detect_unicode_risks',
		description: DESCRIPTION,
		inputSchema: {
			type: 'object',
			properties: {
				content: { type: 'string', description: 'The document text to scan.' },
				kinds: {
					type: 'array',
					items: { type: 'string', enum: KINDS.map(([, short]) => short) },
					description:
						'Report only these kinds. Omit for all of them, which is the only setting under which an empty result means clean.',
				},
				scripts: {
					type: 'array',
					items: { type: 'string' },
					description:
						'Non-Latin scripts this document is expected to be written in, by Unicode name or ISO 15924 tag, e.g. Han or Cyrillic. Naming them is what allows the homoglyph check to run on a translated document.',
				},
				format: {
					type: 'string',
					enum: SUPPORTED_FORMATS,
					description:
						"The document's format, so each finding can carry the key path that names where it sits. Omit it and findings carry a line and column only; an unrecognised name costs the key paths and never a finding.",
				},
				filename: {
					type: 'string',
					description:
						"The document's name, as an alternative to `format`. Only its extension is read, and nothing is opened.",
				},
				maxResults: {
					type: 'integer',
					minimum: 1,
					maximum: MAX_MAX_RESULTS,
					default: DEFAULT_MAX_RESULTS,
					description: `Cap on returned findings (default ${DEFAULT_MAX_RESULTS}). meta.truncated reports whether any were dropped.`,
				},
			},
			required: ['content'],
			additionalProperties: false,
		},
		handler: detect,
	}),
]);
