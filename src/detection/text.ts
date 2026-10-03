import { isWhitespace } from './tables';

/**
 * The few string operations the scanners need, with Rust's meaning.
 *
 * The crate trims with `str::trim`, which strips `White_Space` — U+0085 and
 * U+3000 included, U+FEFF not. JavaScript's `trim` disagrees on both. A YAML
 * line indented with an ideographic space is indented to Rust and not to
 * JavaScript, and the key path would move. So nothing here calls `trim`.
 *
 * Every whitespace codepoint is in the Basic Multilingual Plane, so comparing
 * code units is exact: a surrogate is never whitespace.
 */

export function trimStart(value: string): string {
	let start = 0;
	while (start < value.length && isWhitespace(value.charCodeAt(start))) start++;
	return value.slice(start);
}

export function trimEnd(value: string): string {
	let end = value.length;
	while (end > 0 && isWhitespace(value.charCodeAt(end - 1))) end--;
	return value.slice(0, end);
}

export function trim(value: string): string {
	return trimEnd(trimStart(value));
}

/** `str::trim_matches` over a set of ASCII characters. */
export function trimMatches(value: string, characters: string): string {
	let start = 0;
	let end = value.length;
	while (start < end && characters.includes(value.charAt(start))) start++;
	while (end > start && characters.includes(value.charAt(end - 1))) end--;
	return value.slice(start, end);
}

/** How many bytes a codepoint takes in UTF-8. */
export function utf8Length(cp: number): number {
	if (cp < 0x80) return 1;
	if (cp < 0x800) return 2;
	if (cp < 0x10000) return 3;
	return 4;
}

/** A codepoint and the UTF-16 index it starts at. */
export interface Indexed {
	readonly index: number;
	readonly cp: number;
}

/** Every codepoint in a string with its UTF-16 index — `str::char_indices`. */
export function charIndices(value: string): Indexed[] {
	const out: Indexed[] = [];
	let index = 0;
	while (index < value.length) {
		const cp = value.codePointAt(index) as number;
		out.push({ index, cp });
		index += cp > 0xffff ? 2 : 1;
	}
	return out;
}

/** Whether a string holds an unpaired surrogate — something no `&str` can. */
export function hasLoneSurrogate(value: string): boolean {
	for (let i = 0; i < value.length; i++) {
		const unit = value.charCodeAt(i);
		if (unit >= 0xd800 && unit <= 0xdbff) {
			const next = value.charCodeAt(i + 1);
			if (next >= 0xdc00 && next <= 0xdfff) {
				i++;
				continue;
			}
			return true;
		}
		if (unit >= 0xdc00 && unit <= 0xdfff) return true;
	}
	return false;
}
