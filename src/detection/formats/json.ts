import { join, type KeySpan } from '../locate';
import { utf8Length } from '../text';

/**
 * Key paths in JSON and JSONC, from a scanner rather than a parser — a parser
 * hands back decoded values and loses the offsets the report is indexed by.
 * Array elements are `[0]`, `[1]`, joined by the same rule as every segment.
 * Comments and trailing commas are tolerated; a document that does not parse
 * still yields the key paths it managed to read.
 */

interface Frame {
	readonly array: boolean;
	index: number;
	/** Set on `{` and every `,`; cleared by `:`. */
	expectKey: boolean;
}

export function keySpans(text: string): KeySpan[] {
	const frames: Frame[] = [];
	const path: string[] = [];
	const spans: KeySpan[] = [];
	let at = 0;

	while (at < text.length) {
		const ch = text.charAt(at);
		if (ch === '{' || ch === '[') {
			const array = ch === '[';
			frames.push({ array, index: 0, expectKey: !array });
			path.push(array ? '[0]' : '');
			at++;
		} else if (ch === '}' || ch === ']') {
			frames.pop();
			path.pop();
			at++;
		} else if (ch === ':') {
			const frame = frames[frames.length - 1];
			if (frame) frame.expectKey = false;
			at++;
		} else if (ch === ',') {
			const frame = frames[frames.length - 1];
			if (frame) {
				frame.expectKey = !frame.array;
				if (frame.array) {
					frame.index++;
					if (path.length > 0) path[path.length - 1] = `[${frame.index}]`;
				}
			}
			at++;
		} else if (ch === '"') {
			const [start, end, next] = readString(text, at);
			at = next;
			// A key renames the innermost segment and is not a value region.
			if (frames[frames.length - 1]?.expectKey && path.length > 0) {
				path[path.length - 1] = text.slice(start, end);
				continue;
			}
			spans.push({ start, end, path: join(path) });
		} else if (ch === '/') {
			at = skipComment(text, at);
		} else if (isScalar(ch)) {
			const start = at;
			while (at < text.length && isScalar(text.charAt(at))) at++;
			spans.push({ start, end: at, path: join(path) });
		} else {
			at++;
		}
	}
	return spans;
}

/**
 * The ranges of every escape sequence inside a JSON string. Inside one, `\n` is
 * a line feed and not the letter `n`, so a word ends before it.
 */
export function escapeSpans(text: string): [number, number][] {
	const spans: [number, number][] = [];
	let at = 0;
	while (at < text.length) {
		if (text.charAt(at) !== '"') {
			at++;
			continue;
		}
		at = escapesInString(text, at + 1, spans);
	}
	return spans;
}

function escapesInString(
	text: string,
	from: number,
	spans: [number, number][],
): number {
	let at = from;
	while (at < text.length) {
		const ch = text.charAt(at);
		if (ch === '"') return at + 1;
		if (ch === '\\') {
			// Six bytes for `\uXXXX`, two otherwise — counted in UTF-8 bytes as
			// the crate counts them, then rounded up to a character, because a
			// malformed escape can hold a multi-byte character.
			const width = text.charAt(at + 1) === 'u' ? 6 : 2;
			const end = afterBytes(text, at, width);
			spans.push([at, end]);
			at = end;
			continue;
		}
		at++;
	}
	return text.length;
}

/** The first character boundary at least `bytes` UTF-8 bytes past `from`. */
function afterBytes(text: string, from: number, bytes: number): number {
	let at = from;
	let counted = 0;
	while (at < text.length && counted < bytes) {
		const cp = text.codePointAt(at) as number;
		counted += utf8Length(cp);
		at += cp > 0xffff ? 2 : 1;
	}
	return at;
}

/** The range between the quotes and the index past the closing one. */
function readString(text: string, open: number): [number, number, number] {
	let at = open + 1;
	while (at < text.length) {
		const ch = text.charAt(at);
		if (ch === '\\') {
			at += 2;
			continue;
		}
		if (ch === '"') return [open + 1, at, at + 1];
		at++;
	}
	return [open + 1, text.length, text.length];
}

function isScalar(ch: string): boolean {
	return /^[A-Za-z0-9+\-._]$/.test(ch);
}

function skipComment(text: string, at: number): number {
	const next = text.charAt(at + 1);
	if (next === '/') {
		const newline = text.indexOf('\n', at);
		return newline === -1 ? text.length : newline;
	}
	if (next === '*') {
		const close = text.indexOf('*/', at + 2);
		return close === -1 ? text.length : close + 2;
	}
	return at + 1;
}
