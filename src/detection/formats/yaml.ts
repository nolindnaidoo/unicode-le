import { join, type KeySpan, lines } from '../locate';
import { trim, trimMatches, trimStart, utf8Length } from '../text';

/**
 * Key paths in YAML block style: indentation is the structure. Flow style,
 * anchors, multi-line scalars and multiple documents are not modelled — a value
 * inside one carries the key path of its line, or none.
 *
 * A key is pushed at the column its own text starts at, not the line's indent,
 * so `name` under `- id: A` replaces `id` rather than nesting under it.
 */
export function keySpans(text: string): KeySpan[] {
	let stack: [number, string][] = [];
	let counters: [number, number][] = [];
	const spans: KeySpan[] = [];

	for (const [offset, line] of lines(text)) {
		const content = trimStart(line);
		if (
			content.length === 0 ||
			content.startsWith('#') ||
			content.startsWith('---')
		)
			continue;
		// Two measures of the same indent. Positions are UTF-16, like every
		// index here. Nesting compares one line's indent with another's, and
		// that comparison is the crate's — in UTF-8 bytes — or a line indented
		// with U+1680 nests three deep there and one deep here.
		const indent = line.length - content.length;
		const depth = utf8Width(line.slice(0, indent));
		stack = stack.filter(([at]) => at < depth);
		counters = counters.filter(([at]) => at <= depth);

		let column = indent;
		let columnDepth = depth;
		let body = content;
		if (body.startsWith('-')) {
			let rest = body.slice(1);
			if (rest.startsWith(' ')) rest = rest.slice(1);
			stack.push([depth, `[${nextIndex(counters, depth)}]`]);
			// `- ` is ASCII, so it is as wide in bytes as in code units.
			column += body.length - rest.length;
			columnDepth += body.length - rest.length;
			body = rest;
		}

		const split = splitKey(body);
		if (split === undefined) {
			// A sequence item holding a scalar rather than a mapping.
			if (body.length > 0) {
				spans.push({
					start: offset + column,
					end: offset + line.length,
					path: pathOf(stack),
				});
			}
			continue;
		}
		const [key, valueAt] = split;
		stack.push([columnDepth, key]);

		const value = body.slice(valueAt);
		const leading = value.length - trimStart(value).length;
		// A key introducing a nested block; its children carry it.
		if (trim(value).length === 0) continue;
		spans.push({
			start: offset + column + valueAt + leading,
			end: offset + line.length,
			path: pathOf(stack),
		});
	}
	return spans;
}

function nextIndex(counters: [number, number][], indent: number): number {
	const entry = counters.find(([at]) => at === indent);
	if (entry) {
		entry[1]++;
		return entry[1] - 1;
	}
	counters.push([indent, 1]);
	return 0;
}

/** `key: value` — the colon must end the line or be followed by a space. */
function splitKey(body: string): [string, number] | undefined {
	for (let at = 0; at < body.length; at++) {
		if (body.charAt(at) !== ':') continue;
		if (at + 1 === body.length || body.charAt(at + 1) === ' ') {
			const key = trimMatches(trim(body.slice(0, at)), '"\'');
			return key.length > 0 ? [key, at + 1] : undefined;
		}
	}
	return undefined;
}

function pathOf(stack: readonly [number, string][]): string {
	return join(stack.map(([, segment]) => segment));
}

function utf8Width(text: string): number {
	let width = 0;
	for (const character of text)
		width += utf8Length(character.codePointAt(0) as number);
	return width;
}
