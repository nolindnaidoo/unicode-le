import type { Format } from './format';
import * as csv from './formats/csv';
import * as dotenv from './formats/dotenv';
import * as ini from './formats/ini';
import * as json from './formats/json';
import * as toml from './formats/toml';
import * as yaml from './formats/yaml';

/**
 * Where a finding sits in the document's own vocabulary.
 *
 * Every reader answers one question — which ranges hold a value, and what is
 * the key path of each — and none decides what a finding is. So a reader that
 * is wrong about a key path costs a key path and never a finding. They are line
 * scanners over the raw text, as the crate's are, so the ranges stay the raw
 * document's; ranges here are UTF-16 indices.
 */
export interface KeySpan {
	readonly start: number;
	readonly end: number;
	readonly path: string;
}

export function keySpans(text: string, format: Format): KeySpan[] {
	switch (format) {
		case 'json':
			return json.keySpans(text);
		case 'yaml':
			return yaml.keySpans(text);
		case 'toml':
			return toml.keySpans(text);
		case 'ini':
			return ini.keySpans(text);
		case 'env':
			return dotenv.keySpans(text);
		case 'csv':
			return csv.keySpans(text, ',');
		case 'tsv':
			return csv.keySpans(text, '\t');
		default:
			return [];
	}
}

/** The key path covering an index, if one does. Spans are ordered and disjoint. */
export function keyAt(
	spans: readonly KeySpan[],
	index: number,
): string | undefined {
	let low = 0;
	let high = spans.length;
	while (low < high) {
		const mid = (low + high) >>> 1;
		if ((spans[mid] as KeySpan).start <= index) low = mid + 1;
		else high = mid;
	}
	const span = spans[low - 1];
	return span !== undefined && index < span.end ? span.path : undefined;
}

/** Ranges no word may be built across. Only JSON can know one. */
export function escapeSpans(text: string, format: Format): [number, number][] {
	return format === 'json' ? json.escapeSpans(text) : [];
}

/**
 * Every line with the index it starts at, without its line terminator — the
 * crate's `split_inclusive('\n')` and `trim_end_matches(['\n', '\r'])`.
 */
export function lines(text: string): [number, string][] {
	const out: [number, string][] = [];
	let start = 0;
	while (start < text.length) {
		const newline = text.indexOf('\n', start);
		const end = newline === -1 ? text.length : newline + 1;
		out.push([start, text.slice(start, end).replace(/[\n\r]+$/, '')]);
		start = end;
	}
	return out;
}

/** A dotted path, skipping the empty segments a document's root leaves. */
export function join(segments: readonly string[]): string {
	return segments.filter((segment) => segment.length > 0).join('.');
}
