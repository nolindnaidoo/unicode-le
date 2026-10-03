import type { Draft } from '../types';
import { renderEach } from './codepoint';
import { combiningClass, composition, decomposition } from './tables';
import { charIndices } from './text';

/**
 * Which normalization form the document is in — reported, never fixed.
 *
 * `é` as one codepoint and `é` as `e` plus a combining acute are the same
 * character to a reader and two different keys to a hash, a unique index and a
 * `Map`. This reports a line that is not NFC and does not rewrite it: a fixture
 * may be NFD on purpose.
 *
 * **Normalization is implemented here rather than borrowed.**
 * `String.prototype.normalize` is the engine's ICU, on the engine's Unicode
 * version, and the crate's `unicode-normalization` is on its own. They agree on
 * old text and part company on the newest decompositions — so the canonical
 * algorithm runs over the crate's tables, step for step as `unicode-
 * normalization` runs it, including the order it emits characters in.
 */

const S_BASE = 0xac00;
const L_BASE = 0x1100;
const V_BASE = 0x1161;
const T_BASE = 0x11a7;
const L_COUNT = 19;
const V_COUNT = 21;
const T_COUNT = 28;
const N_COUNT = V_COUNT * T_COUNT;
const S_COUNT = L_COUNT * N_COUNT;

function decomposeInto(cp: number, out: number[]): void {
	const s = cp - S_BASE;
	if (s >= 0 && s < S_COUNT) {
		out.push(L_BASE + Math.floor(s / N_COUNT));
		out.push(V_BASE + Math.floor((s % N_COUNT) / T_COUNT));
		if (s % T_COUNT !== 0) out.push(T_BASE + (s % T_COUNT));
		return;
	}
	const full = decomposition(cp);
	if (full) out.push(...full);
	else out.push(cp);
}

function compose(first: number, second: number): number | undefined {
	if (
		first >= L_BASE &&
		first < L_BASE + L_COUNT &&
		second >= V_BASE &&
		second < V_BASE + V_COUNT
	) {
		return S_BASE + (first - L_BASE) * N_COUNT + (second - V_BASE) * T_COUNT;
	}
	const s = first - S_BASE;
	if (
		s >= 0 &&
		s < S_COUNT &&
		second > T_BASE &&
		second < T_BASE + T_COUNT &&
		s % T_COUNT === 0
	) {
		return first + (second - T_BASE);
	}
	return composition(first, second);
}

/** NFD: full canonical decomposition, then a stable sort of each run of non-starters. */
export function nfd(cps: readonly number[]): number[] {
	const out: number[] = [];
	for (const cp of cps) decomposeInto(cp, out);
	let runStart = 0;
	for (let i = 0; i <= out.length; i++) {
		if (i === out.length || combiningClass(out[i] as number) === 0) {
			if (i - runStart > 1) {
				const run = out
					.slice(runStart, i)
					.map((cp, order) => ({ cp, order, ccc: combiningClass(cp) }))
					.sort((a, b) => a.ccc - b.ccc || a.order - b.order);
				for (let j = 0; j < run.length; j++)
					out[runStart + j] = (run[j] as { cp: number }).cp;
			}
			runStart = i + 1;
		}
	}
	return out;
}

/** NFC: `unicode-normalization`'s `Recompositions`, transcribed. */
export function nfc(cps: readonly number[]): number[] {
	const decomposed = nfd(cps);
	const out: number[] = [];
	let composee: number | undefined;
	let lastCcc: number | undefined;
	let buffer: number[] = [];

	for (const ch of decomposed) {
		const chClass = combiningClass(ch);
		if (composee === undefined) {
			if (chClass !== 0) {
				out.push(ch);
				continue;
			}
			composee = ch;
			continue;
		}
		if (lastCcc === undefined) {
			const composed = compose(composee, ch);
			if (composed !== undefined) {
				composee = composed;
				continue;
			}
			if (chClass === 0) {
				out.push(composee);
				composee = ch;
				continue;
			}
			buffer.push(ch);
			lastCcc = chClass;
			continue;
		}
		if (lastCcc >= chClass) {
			if (chClass === 0) {
				out.push(composee, ...buffer);
				buffer = [];
				composee = ch;
				lastCcc = undefined;
				continue;
			}
			buffer.push(ch);
			lastCcc = chClass;
			continue;
		}
		const composed = compose(composee, ch);
		if (composed !== undefined) {
			composee = composed;
			continue;
		}
		buffer.push(ch);
		lastCcc = chClass;
	}
	if (composee !== undefined) out.push(composee);
	out.push(...buffer);
	return out;
}

function sameSequence(a: readonly number[], b: readonly number[]): boolean {
	return a.length === b.length && a.every((cp, i) => cp === b[i]);
}

export function scan(content: string): Draft[] {
	const drafts: Draft[] = [];
	let lineStart = 0;
	while (lineStart < content.length) {
		const newline = content.indexOf('\n', lineStart);
		const end = newline === -1 ? content.length : newline + 1;
		const draft = lineDraft(content.slice(lineStart, end), lineStart);
		if (draft) drafts.push(draft);
		lineStart = end;
	}
	return drafts;
}

function lineDraft(line: string, lineStart: number): Draft | undefined {
	const indexed = charIndices(line);
	const cps = indexed.map((entry) => entry.cp);
	const composed = nfc(cps);
	if (sameSequence(cps, composed)) return undefined;

	// The first character that differs from the line's NFC form, and the
	// combining sequence that follows it — for `e` + U+0301, both.
	let at = 0;
	while (at < cps.length && cps[at] === composed[at]) at++;
	// Only a line that is a strict prefix of its own NFC form gets this far
	// without diverging, and the crate answers that with the line's start and
	// no codepoints rather than inventing a position.
	const sequence: number[] = [];
	if (at < cps.length) {
		sequence.push(cps[at] as number);
		for (
			let j = at + 1;
			j < cps.length && combiningClass(cps[j] as number) !== 0;
			j++
		) {
			sequence.push(cps[j] as number);
		}
	}
	// NFKD is a subset of NFD, so naming it buys nothing; what a reader needs
	// is whether this is a decomposed document or a mixture.
	const form = sameSequence(cps, nfd(cps)) ? 'NFD' : 'neither NFC nor NFD';
	return {
		index: lineStart + (indexed[at]?.index ?? 0),
		kind: 'non-nfc',
		severity: 'low',
		codepoints: renderEach(sequence),
		scripts: [],
		resembles: [],
		detail: `this line is in ${form}, not NFC: it compares unequal, hashes differently and indexes differently from the same text written in NFC`,
	};
}
