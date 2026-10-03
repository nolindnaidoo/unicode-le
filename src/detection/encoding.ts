import type { Refusal } from '../types';

/**
 * Bytes → text, or a refusal. Never a guess.
 *
 * Here a mis-decode does not merely miss findings, it **invents** them: read a
 * UTF-16 file as UTF-8 and every second byte is a NUL or a stray high byte, so a
 * guess reports a document full of invisible and unassigned characters that do
 * not exist. Byte-order mark first, binary sniff second, UTF-8 last.
 *
 * A leading UTF-8 byte-order mark is kept, as the crate keeps it, so offsets
 * stay the file's own; it is excluded from the findings instead.
 */

/** Ripgrep's number: 8 KB without a NUL is text to anything that greps it. */
const BINARY_SNIFF_BYTES = 8192;

/** Longest first: UTF-32LE starts with the UTF-16LE mark. */
const MARKS: readonly (readonly [readonly number[], string])[] = [
	[[0x00, 0x00, 0xfe, 0xff], 'UTF-32BE'],
	[[0xff, 0xfe, 0x00, 0x00], 'UTF-32LE'],
	[[0xfe, 0xff], 'UTF-16BE'],
	[[0xff, 0xfe], 'UTF-16LE'],
];

export type Decoded =
	| { readonly text: string; readonly refusal?: undefined }
	| { readonly text?: undefined; readonly refusal: Refusal };

export function decode(bytes: Uint8Array): Decoded {
	const mark = MARKS.find(([prefix]) =>
		prefix.every((byte, i) => bytes[i] === byte),
	);
	if (mark) {
		const [, encoding] = mark;
		return {
			refusal: {
				reason: 'encoding_unknown',
				detail: `a ${encoding} byte-order mark: this reads UTF-8 only, and decoding ${encoding} as UTF-8 would invent invisible and unassigned characters that are not in the file`,
			},
		};
	}
	const limit = Math.min(bytes.length, BINARY_SNIFF_BYTES);
	for (let i = 0; i < limit; i++) {
		if (bytes[i] === 0) {
			return {
				refusal: {
					reason: 'binary_or_undecodable',
					detail: `a NUL byte at byte ${i}: this was never a text candidate`,
				},
			};
		}
	}
	const invalid = firstInvalid(bytes);
	if (invalid !== -1) {
		return {
			refusal: {
				reason: 'binary_or_undecodable',
				detail: `not valid UTF-8 at byte ${invalid}: the encoding is unknown and is not guessed`,
			},
		};
	}
	// `ignoreBOM` keeps a leading mark in the text, as the crate does.
	return { text: new TextDecoder('utf-8', { ignoreBOM: true }).decode(bytes) };
}

/**
 * Where the first invalid sequence starts, or -1 — the crate's
 * `Utf8Error::valid_up_to`, which `TextDecoder` does not report. Overlong
 * forms, surrogates and codepoints past U+10FFFF are all invalid, as in Rust.
 */
export function firstInvalid(bytes: Uint8Array): number {
	let i = 0;
	while (i < bytes.length) {
		const first = bytes[i] as number;
		if (first < 0x80) {
			i++;
			continue;
		}
		let width: number;
		let low = 0x80;
		let high = 0xbf;
		if (first >= 0xc2 && first <= 0xdf) width = 2;
		else if (first >= 0xe0 && first <= 0xef) {
			width = 3;
			if (first === 0xe0) low = 0xa0;
			if (first === 0xed) high = 0x9f;
		} else if (first >= 0xf0 && first <= 0xf4) {
			width = 4;
			if (first === 0xf0) low = 0x90;
			if (first === 0xf4) high = 0x8f;
		} else return i;
		if (i + width > bytes.length) return i;
		const second = bytes[i + 1] as number;
		if (second < low || second > high) return i;
		for (let j = 2; j < width; j++) {
			const next = bytes[i + j] as number;
			if (next < 0x80 || next > 0xbf) return i;
		}
		i += width;
	}
	return -1;
}
