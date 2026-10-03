import { type KeySpan, lines } from '../locate';
import { trim } from '../text';

/** Key paths in INI and `.properties`: `[section]`, then `key = value` or `key: value`. */
export function keySpans(text: string): KeySpan[] {
	let section = '';
	const spans: KeySpan[] = [];

	for (const [offset, line] of lines(text)) {
		const trimmed = trim(line);
		if (
			trimmed.length === 0 ||
			trimmed.startsWith(';') ||
			trimmed.startsWith('#')
		)
			continue;
		if (
			trimmed.startsWith('[') &&
			trimmed.endsWith(']') &&
			trimmed.length >= 2
		) {
			section = trim(trimmed.slice(1, -1));
			continue;
		}
		const at = separator(line);
		if (at === -1) continue;
		const key = trim(line.slice(0, at));
		if (key.length === 0) continue;
		spans.push({
			start: offset + at + 1,
			end: offset + line.length,
			path: section.length === 0 ? key : `${section}.${key}`,
		});
	}
	return spans;
}

/** `=` where there is one, else `:` — a value is very often a URL. */
function separator(line: string): number {
	const equals = line.indexOf('=');
	return equals !== -1 ? equals : line.indexOf(':');
}
