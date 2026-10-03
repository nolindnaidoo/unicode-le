import { type KeySpan, lines } from '../locate';
import { trim, trimMatches } from '../text';

/**
 * Key paths in TOML: `[table]` and `[[array]]` headers, then `key = value`.
 * A multi-line array has its key on the first line; the index of an array of
 * tables is deliberately absent rather than guessed.
 */
export function keySpans(text: string): KeySpan[] {
	let table = '';
	const spans: KeySpan[] = [];

	for (const [offset, line] of lines(text)) {
		const trimmed = trim(line);
		if (trimmed.length === 0 || trimmed.startsWith('#')) continue;
		const name = header(trimmed);
		if (name !== undefined) {
			table = name;
			continue;
		}
		const equals = line.indexOf('=');
		if (equals === -1) continue;
		const key = trimMatches(trim(line.slice(0, equals)), '"');
		if (key.length === 0) continue;
		spans.push({
			start: offset + equals + 1,
			end: offset + line.length,
			path: table.length === 0 ? key : `${table}.${key}`,
		});
	}
	return spans;
}

/** The name inside `[table]` or `[[array]]`. */
function header(trimmed: string): string | undefined {
	if (!trimmed.startsWith('[') || !trimmed.endsWith(']') || trimmed.length < 2)
		return undefined;
	const inner = trimmed.slice(1, -1);
	const nested =
		inner.startsWith('[') && inner.endsWith(']') && inner.length >= 2
			? inner.slice(1, -1)
			: inner;
	return trim(nested);
}
