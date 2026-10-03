import { type KeySpan, lines } from '../locate';
import { trim, trimStart } from '../text';

/** Key paths in `.env`: the key is the path, and `export` is not part of it. */
export function keySpans(text: string): KeySpan[] {
	const spans: KeySpan[] = [];
	for (const [offset, line] of lines(text)) {
		const trimmed = trimStart(line);
		if (trimmed.length === 0 || trimmed.startsWith('#')) continue;
		const equals = line.indexOf('=');
		if (equals === -1) continue;
		const declared = trim(line.slice(0, equals));
		const key = trim(
			declared.startsWith('export ')
				? declared.slice('export '.length)
				: declared,
		);
		if (key.length === 0) continue;
		const raw = line.slice(equals + 1);
		spans.push({
			start: offset + equals + 1,
			end: offset + equals + 1 + valueLength(raw),
			path: key,
		});
	}
	return spans;
}

/** A `#` inside a quoted value is part of it; only an unquoted one is cut. */
function valueLength(raw: string): number {
	const body = trimStart(raw);
	const leading = raw.length - body.length;
	if (body.startsWith('"') || body.startsWith("'")) return raw.length;
	const comment = body.indexOf(' #');
	return leading + (comment === -1 ? body.length : comment);
}
