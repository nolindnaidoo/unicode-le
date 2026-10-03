import { trim } from './text';

/**
 * Which key-path reader a document gets.
 *
 * **An unresolved format is not an error, and never a lost finding.** A bidi
 * control is a bidi control in a `.md`, a `.json` and a file with no extension,
 * so the format decides one thing: whether a finding can be given the
 * document's own name for where it sits.
 */

/**
 * Every name a caller might send, mapped to the reader it means. Both a VS Code
 * `languageId` and a file extension appear, and the table is the crate's
 * `ALIASES`, entry for entry.
 */
const ALIASES: Readonly<Record<string, Format>> = Object.freeze({
	json: 'json',
	jsonc: 'json',
	yaml: 'yaml',
	yml: 'yaml',
	csv: 'csv',
	tsv: 'tsv',
	toml: 'toml',
	ini: 'ini',
	cfg: 'ini',
	conf: 'ini',
	properties: 'ini',
	env: 'env',
	dotenv: 'env',
	text: 'text',
	txt: 'text',
});

export const SUPPORTED_FORMATS = [
	'json',
	'yaml',
	'csv',
	'tsv',
	'toml',
	'ini',
	'env',
	'text',
] as const;

export type Format = (typeof SUPPORTED_FORMATS)[number];

/** `text`, not `unknown`: nothing was skipped, the document had no keys. */
export const FALLBACK_FORMAT: Format = 'text';

function normalise(value: string): string {
	return trim(value).toLowerCase().replace(/^\.+/, '');
}

export function canonical(format: string): Format {
	return Object.hasOwn(ALIASES, format)
		? (ALIASES[format] as Format)
		: FALLBACK_FORMAT;
}

/** An explicit format, else the filename, else the fallback. */
export function resolveFormat(
	format: string | undefined,
	filename: string | undefined,
): Format {
	if (format !== undefined) {
		const direct = canonical(normalise(format));
		if (direct !== FALLBACK_FORMAT) return direct;
	}
	if (filename === undefined) return FALLBACK_FORMAT;
	// A path, not a name: a directory called `notes.json` above an
	// extensionless file must not name the file's format.
	const base = filename.split(/[/\\]/).pop() ?? filename;
	// A dotfile like `.env` has no extension to split on; its name is the type.
	const whole = canonical(normalise(base));
	if (whole !== FALLBACK_FORMAT) return whole;
	const dot = base.lastIndexOf('.');
	return dot === -1
		? FALLBACK_FORMAT
		: canonical(normalise(base.slice(dot + 1)));
}
