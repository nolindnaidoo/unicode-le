import type { Draft, Refusal } from '../types';
import { render, renderEach } from './codepoint';
import {
	allScripts,
	COMMON,
	homoglyphOf,
	INHERITED,
	isAlphabetic,
	isNumeric,
	isSingleScript,
	LATIN,
	lookalikeOf,
	scriptName,
	scriptOf,
	UNKNOWN,
} from './tables';
import { charIndices } from './text';

/**
 * Mixed scripts, homoglyphs, and the refusal that makes both usable.
 *
 * The naive confusable check — flag every character that resembles an ASCII one
 * — reports every letter of every Russian, Greek and Chinese string in a tree,
 * gets switched off within a day, and takes the Trojan Source screen with it.
 * Three rules keep this one honest, and they are the crate's, unchanged:
 *
 * - **A word is judged, never a file.** `Привет` is wholly Cyrillic; the
 *   Cyrillic `а` in `pаypal` sits in a Latin word, and only that is a finding.
 * - **A file plainly in another script is refused, not guessed at**, measured
 *   over the scripts the caller did *not* declare.
 * - **A declared script is an expected one**: mixing Latin with it is a
 *   translation, and mixing Latin with an undeclared one is still a finding.
 */

/** Measured, not picked: a `zh-CN` catalogue is 16% Han, a quoted word 0.1%. */
const FOREIGN_SHARE_PERCENT = 10;

function isPlaceholder(script: number): boolean {
	return script === COMMON || script === INHERITED || script === UNKNOWN;
}

/** Whether this file may be judged for confusables and mixed script. */
export function context(
	content: string,
	expected: readonly number[],
): Refusal | undefined {
	let letters = 0;
	const found: [number, number][] = [];
	for (const { cp } of charIndices(content)) {
		if (!isAlphabetic(cp)) continue;
		letters++;
		const script = scriptOf(cp);
		if (script === LATIN || isPlaceholder(script)) continue;
		const entry = found.find(([seen]) => seen === script);
		if (entry) entry[1]++;
		else found.push([script, 1]);
	}
	const undeclaredScripts = found.filter(
		([script]) => !expected.includes(script),
	);
	if (undeclaredScripts.length === 0) return undefined;
	const undeclared = undeclaredScripts.reduce(
		(sum, [, count]) => sum + count,
		0,
	);
	// Integer arithmetic, so the decision and the printed number are one
	// computation and identical on every platform.
	const share = Math.floor((undeclared * 100) / Math.max(letters, 1));
	if (share < FOREIGN_SHARE_PERCENT) return undefined;

	const names = undeclaredScripts
		.map(([script]) => scriptName(script))
		.sort(byName);
	return {
		reason: 'intentional_script_context',
		detail: `${share}% of this file's letters are ${joinNames(names)}, ${declaration(expected)}. The confusable and mixed-script checks did not run here: in a file written in another script there is nothing to tell a forged name from a translated one, and answering anyway would bury the real findings. Every other check did run.`,
	};
}

/** Rust's `str` ordering: by UTF-8 bytes, which for these ASCII names is by code unit. */
function byName(a: string, b: string): number {
	return a < b ? -1 : a > b ? 1 : 0;
}

function declaration(expected: readonly number[]): string {
	return expected.length === 0
		? 'and no expected script was declared for it'
		: 'and none of those is among the scripts declared for it';
}

/** `A`, `A and B`, `A, B and C` — Japanese always brings three. */
function joinNames(names: readonly string[]): string {
	if (names.length === 0) return '';
	if (names.length === 1) return names[0] as string;
	return `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}`;
}

export function scan(
	content: string,
	expected: readonly number[],
	escapes: readonly (readonly [number, number])[],
): Draft[] {
	const drafts: Draft[] = [];
	for (const [index, word] of words(content, escapes)) {
		drafts.push(
			...mixing(index, word, expected),
			...compatibility(index, word),
		);
	}
	return drafts;
}

/** The half `expected` governs, and the only one. */
function mixing(
	index: number,
	word: string,
	expected: readonly number[],
): Draft[] {
	const scripts = scriptsIn(word);
	const undeclared = scripts.filter(
		(script) => script !== LATIN && !expected.includes(script),
	);
	if (undeclared.length === 0 || isSingleScript(word)) return [];
	return mixed(index, word, scripts, undeclared);
}

/**
 * Whether a character is a compatibility form of an ASCII one. `expected` is
 * deliberately not a parameter, so no declaration can gate it.
 */
function compatibility(index: number, word: string): Draft[] {
	const drafts: Draft[] = [];
	for (const { index: at, cp } of charIndices(word)) {
		const script = scriptOf(cp);
		if (script !== LATIN && script !== COMMON) continue;
		// The table is the crate's `ascii_lookalike` written out, ordinals
		// and superscripts already excluded.
		const plain = lookalikeOf(cp);
		if (plain === undefined) continue;
		drafts.push({
			index: index + at,
			kind: 'confusable',
			severity: 'high',
			codepoints: [render(cp)],
			scripts: [scriptName(script)],
			resembles: [render(plain)],
			detail:
				'a compatibility form of an ASCII character: it reads as the codepoint under `resembles` and does not compare equal to it',
		});
	}
	return drafts;
}

/** Every script written in the word, placeholders excluded, sorted by name. */
function scriptsIn(word: string): number[] {
	const scripts: number[] = [];
	for (const { cp } of charIndices(word)) {
		const script = scriptOf(cp);
		if (isPlaceholder(script) || scripts.includes(script)) continue;
		scripts.push(script);
	}
	return scripts.sort((a, b) => byName(scriptName(a), scriptName(b)));
}

function mixed(
	index: number,
	word: string,
	scripts: readonly number[],
	undeclared: readonly number[],
): Draft[] {
	const names = scripts.map(scriptName);
	const foreign: number[] = [];
	for (const { cp } of charIndices(word)) {
		if (cp >= 0x80 && !foreign.includes(cp)) foreign.push(cp);
	}
	const drafts: Draft[] = [
		{
			index,
			kind: 'mixed-script',
			severity: 'high',
			codepoints: renderEach(foreign),
			scripts: names,
			resembles: [],
			detail: `one word written in ${joinNames(names)}: no single script accounts for it, which is how a name that reads as familiar is forged`,
		},
	];
	for (const { index: at, cp } of charIndices(word)) {
		const script = scriptOf(cp);
		if (!undeclared.includes(script)) continue;
		const prototype = homoglyphOf(cp);
		if (prototype === undefined) continue;
		const name = scriptName(script);
		drafts.push({
			index: index + at,
			kind: 'confusable',
			severity: 'high',
			codepoints: [render(cp)],
			scripts: [name],
			resembles: renderEach(prototype),
			detail: `a ${name} character in a word that is not ${name}, and it reduces to the codepoint under \`resembles\`: the two are indistinguishable on screen`,
		});
	}
	return drafts;
}

/**
 * Letters, digits, underscores and combining marks — an identifier in every
 * language this is pointed at. `escapes` are UTF-16 ranges no word is built
 * across, and only the JSON reader supplies any.
 */
function words(
	content: string,
	escapes: readonly (readonly [number, number])[],
): [number, string][] {
	const out: [number, string][] = [];
	let start: number | undefined;
	for (const { index, cp } of charIndices(content)) {
		if (isWordCharacter(cp) && !inEscape(escapes, index)) {
			start ??= index;
			continue;
		}
		if (start !== undefined) {
			out.push([start, content.slice(start, index)]);
			start = undefined;
		}
	}
	if (start !== undefined) out.push([start, content.slice(start)]);
	return out;
}

function inEscape(
	escapes: readonly (readonly [number, number])[],
	index: number,
): boolean {
	let low = 0;
	let high = escapes.length;
	while (low < high) {
		const mid = (low + high) >>> 1;
		if ((escapes[mid] as readonly [number, number])[0] <= index) low = mid + 1;
		else high = mid;
	}
	const span = escapes[low - 1];
	return span !== undefined && index < span[1];
}

function isWordCharacter(cp: number): boolean {
	return (
		isAlphabetic(cp) ||
		isNumeric(cp) ||
		cp === 0x5f ||
		scriptOf(cp) === INHERITED
	);
}

/**
 * A script named by the caller: full UAX #24 name or ISO 15924 tag, any case.
 * The placeholder values parse and would do nothing, so they are refused.
 */
export function parseScript(tag: string): number {
	const canonical = canonicalise(tag);
	const scripts = allScripts();
	let index = scripts.findIndex(([full]) => full === canonical);
	if (index === -1)
		index = scripts.findIndex(([, short]) => short === canonical);
	if (index === -1) {
		throw new Error(
			`${tag} is not a Unicode script; try a name like Han, Cyrillic or Devanagari`,
		);
	}
	if (isPlaceholder(index)) {
		throw new Error(
			`${tag} is a Unicode script property value, not a writing system; name the script the text is actually written in`,
		);
	}
	return index;
}

function canonicalise(tag: string): string {
	return tag
		.split(/[-_]/)
		.map((segment) => {
			const [first, ...rest] = [...segment];
			if (first === undefined) return '';
			return first.toUpperCase() + rest.join('').toLowerCase();
		})
		.join('_');
}
