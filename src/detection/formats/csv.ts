import { type KeySpan, lines } from '../locate';
import { trim, trimMatches } from '../text';

/**
 * Key paths in CSV and TSV: the column's header name, or `[n]` where the header
 * has none. A quoted field keeps its quotes in the range, because the offsets
 * are what the report is indexed by.
 */
export function keySpans(text: string, delimiter: string): KeySpan[] {
	const rows = lines(text).filter(([, line]) => trim(line).length > 0);
	const first = rows.shift();
	if (first === undefined) return [];
	const [, headerLine] = first;
	const headers = fields(headerLine, delimiter).map(([start, end]) =>
		trimMatches(trim(headerLine.slice(start, end)), '"'),
	);

	const spans: KeySpan[] = [];
	for (const [offset, line] of rows) {
		fields(line, delimiter).forEach(([start, end], column) => {
			const name = headers[column];
			spans.push({
				start: offset + start,
				end: offset + end,
				path: name !== undefined && name.length > 0 ? name : `[${column}]`,
			});
		});
	}
	return spans;
}

function fields(line: string, delimiter: string): [number, number][] {
	const out: [number, number][] = [];
	let start = 0;
	let quoted = false;
	for (let at = 0; at < line.length; at++) {
		const ch = line.charAt(at);
		if (ch === '"') quoted = !quoted;
		else if (ch === delimiter && !quoted) {
			out.push([start, at]);
			start = at + 1;
		}
	}
	out.push([start, line.length]);
	return out;
}
