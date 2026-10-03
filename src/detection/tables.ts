import data from '../../crate/fixtures/unicode-tables.json';

/**
 * The Unicode data every finding rests on, read from the crate's own answers.
 *
 * Whether a character is a letter, which script it belongs to, what it
 * decomposes into and what it impersonates are all properties of a Unicode
 * version. The editor's JavaScript engine and the Node that runs the MCP server
 * each carry their own, and neither is the crate's. Asking them would make the
 * two frontends disagree on precisely the newest characters — the ones an
 * attacker reaches for because nothing has caught up with them yet.
 *
 * So nothing here asks the engine. `crate/fixtures/unicode-tables.json` is
 * rendered by the crate from the same dependencies its scanner uses, and a
 * crate test fails the moment the two disagree. No `\p{…}` regex, no
 * `String.prototype.normalize`, no `toUpperCase` decides a finding in this
 * extension.
 */

type Extension = 'Common' | 'Inherited' | readonly number[];

/** `[delta, value, delta, value, …]` → absolute starts and their values. */
function decodeRuns(encoded: readonly number[]): {
	readonly starts: Uint32Array;
	readonly values: Uint32Array;
} {
	const count = encoded.length / 2;
	const starts = new Uint32Array(count);
	const values = new Uint32Array(count);
	let start = 0;
	for (let i = 0; i < count; i++) {
		start += encoded[i * 2] as number;
		starts[i] = start;
		values[i] = encoded[i * 2 + 1] as number;
	}
	return { starts, values };
}

/** `[delta to start, length, …]` → `[start, end)` pairs, flattened. */
function decodeSet(encoded: readonly number[]): Uint32Array {
	const out = new Uint32Array(encoded.length);
	let start = 0;
	for (let i = 0; i < encoded.length; i += 2) {
		start += encoded[i] as number;
		out[i] = start;
		out[i + 1] = start + (encoded[i + 1] as number);
	}
	return out;
}

/** `[delta to key, count, values…]` → a map. */
function decodeMap(
	encoded: readonly number[],
): ReadonlyMap<number, readonly number[]> {
	const out = new Map<number, readonly number[]>();
	let key = 0;
	let at = 0;
	while (at < encoded.length) {
		key += encoded[at] as number;
		const count = encoded[at + 1] as number;
		out.set(key, Object.freeze(encoded.slice(at + 2, at + 2 + count)));
		at += 2 + count;
	}
	return out;
}

/** The run a codepoint falls in: the last start at or below it. */
function lookupRun(
	runs: { readonly starts: Uint32Array; readonly values: Uint32Array },
	cp: number,
): number {
	let low = 0;
	let high = runs.starts.length - 1;
	while (low < high) {
		const mid = (low + high + 1) >>> 1;
		if ((runs.starts[mid] as number) <= cp) low = mid;
		else high = mid - 1;
	}
	return runs.values[low] as number;
}

function inSet(ranges: Uint32Array, cp: number): boolean {
	let low = 0;
	let high = ranges.length / 2 - 1;
	while (low <= high) {
		const mid = (low + high) >>> 1;
		const start = ranges[mid * 2] as number;
		const end = ranges[mid * 2 + 1] as number;
		if (cp < start) high = mid - 1;
		else if (cp >= end) low = mid + 1;
		else return true;
	}
	return false;
}

const SCRIPT_NAMES: readonly (readonly [string, string])[] = data.scripts as [
	string,
	string,
][];
const SCRIPT_RUNS = decodeRuns(data.script);
const EXTENSIONS: readonly Extension[] = (data.extensions as Extension[]).map(
	(ext) => (typeof ext === 'string' ? ext : Object.freeze([...ext])),
);
const EXTENSION_RUNS = decodeRuns(data.extension);
const ALPHABETIC = decodeSet(data.alphabetic);
const NUMERIC = decodeSet(data.numeric);
const WHITESPACE = decodeSet(data.whitespace);
const COMBINING_RUNS = decodeRuns(data.combining);
const DECOMPOSITION = decodeMap(data.decomposition);
const HOMOGLYPH = decodeMap(data.homoglyph);
const LOOKALIKE = decodeMap(data.lookalike);
const COMPOSITION: ReadonlyMap<number, number> = (() => {
	const out = new Map<number, number>();
	const triples = data.composition;
	for (let i = 0; i < triples.length; i += 3) {
		out.set(
			pairKey(triples[i] as number, triples[i + 1] as number),
			triples[i + 2] as number,
		);
	}
	return out;
})();

function pairKey(first: number, second: number): number {
	return first * 0x110000 + second;
}

/** The Unicode version `unicode-script` is on, for the unassigned message. */
export const UNICODE_VERSION: string = data.unicode.script;

/** A script's index in the table, by its full name. */
export function scriptIndex(fullName: string): number {
	const index = SCRIPT_NAMES.findIndex(([name]) => name === fullName);
	if (index === -1) throw new Error(`no script is named ${fullName}`);
	return index;
}

export const LATIN = scriptIndex('Latin');
export const COMMON = scriptIndex('Common');
export const INHERITED = scriptIndex('Inherited');
export const UNKNOWN = scriptIndex('Unknown');
export const HAN = scriptIndex('Han');
const HIRAGANA = scriptIndex('Hiragana');
const KATAKANA = scriptIndex('Katakana');
const HANGUL = scriptIndex('Hangul');
const BOPOMOFO = scriptIndex('Bopomofo');

export function scriptOf(cp: number): number {
	return lookupRun(SCRIPT_RUNS, cp);
}

export function scriptName(index: number): string {
	return (SCRIPT_NAMES[index] as readonly [string, string])[0];
}

/** Every script, as `[full name, ISO 15924 tag]`. */
export function allScripts(): readonly (readonly [string, string])[] {
	return SCRIPT_NAMES;
}

/** Rust's `char::is_alphabetic`. */
export function isAlphabetic(cp: number): boolean {
	return inSet(ALPHABETIC, cp);
}

/** Rust's `char::is_numeric`. */
export function isNumeric(cp: number): boolean {
	return inSet(NUMERIC, cp);
}

/** Rust's `char::is_whitespace` — which is not JavaScript's `\s`. */
export function isWhitespace(cp: number): boolean {
	return inSet(WHITESPACE, cp);
}

export function combiningClass(cp: number): number {
	return lookupRun(COMBINING_RUNS, cp);
}

/** The full canonical decomposition, Hangul excluded (it is algorithmic). */
export function decomposition(cp: number): readonly number[] | undefined {
	return DECOMPOSITION.get(cp);
}

/** The canonical composition of a pair, Hangul excluded. */
export function composition(first: number, second: number): number | undefined {
	return COMPOSITION.get(pairKey(first, second));
}

/**
 * The UTS #39 prototype of a character that is potentially confusable across
 * scripts, when it is not itself. Both of `homoglyph_draft`'s conditions are
 * already applied by the crate that wrote the table.
 */
export function homoglyphOf(cp: number): readonly number[] | undefined {
	return HOMOGLYPH.get(cp);
}

/** The ASCII character a compatibility form reads as — `ascii_lookalike`. */
export function lookalikeOf(cp: number): number | undefined {
	return LOOKALIKE.get(cp)?.[0];
}

/**
 * A UTS #39 augmented script set, the shape `unicode-security` resolves a word
 * to. `all` is Common or Inherited, which stand for every script; otherwise
 * `scripts` holds the script indices.
 */
export interface AugmentedScriptSet {
	readonly all: boolean;
	readonly common: boolean;
	readonly scripts: ReadonlySet<number>;
	readonly hanb: boolean;
	readonly jpan: boolean;
	readonly kore: boolean;
}

const EVERY: ReadonlySet<number> = new Set();

function augment(
	all: boolean,
	common: boolean,
	scripts: ReadonlySet<number>,
): AugmentedScriptSet {
	if (all || scripts.has(HAN)) {
		return { all, common, scripts, hanb: true, jpan: true, kore: true };
	}
	return {
		all,
		common,
		scripts,
		hanb: scripts.has(BOPOMOFO),
		jpan: scripts.has(HIRAGANA) || scripts.has(KATAKANA),
		kore: scripts.has(HANGUL),
	};
}

function augmentedFor(cp: number): AugmentedScriptSet {
	const ext = EXTENSIONS[lookupRun(EXTENSION_RUNS, cp)] as Extension;
	if (ext === 'Common') return augment(true, true, EVERY);
	if (ext === 'Inherited') return augment(true, false, EVERY);
	return augment(false, false, new Set(ext));
}

function intersect(
	a: AugmentedScriptSet,
	b: AugmentedScriptSet,
): AugmentedScriptSet {
	let all: boolean;
	let scripts: ReadonlySet<number>;
	if (a.all && b.all) {
		all = true;
		scripts = EVERY;
	} else if (a.all) {
		all = false;
		scripts = b.scripts;
	} else if (b.all) {
		all = false;
		scripts = a.scripts;
	} else {
		all = false;
		scripts = new Set([...a.scripts].filter((s) => b.scripts.has(s)));
	}
	return {
		all,
		common: a.common && b.common,
		scripts,
		hanb: a.hanb && b.hanb,
		jpan: a.jpan && b.jpan,
		kore: a.kore && b.kore,
	};
}

/**
 * `MixedScript::is_single_script`: the resolved script set of the word is not
 * empty. Starts from Common, as `AugmentedScriptSet::default` does.
 */
export function isSingleScript(word: string): boolean {
	let set = augment(true, true, EVERY);
	for (const character of word) {
		set = intersect(set, augmentedFor(character.codePointAt(0) as number));
	}
	const empty =
		!set.all && set.scripts.size === 0 && !set.hanb && !set.jpan && !set.kore;
	return !empty;
}
