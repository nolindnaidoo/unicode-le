import type { Draft } from '../types';
import { render } from './codepoint';
import { scriptName, scriptOf, UNICODE_VERSION, UNKNOWN } from './tables';
import { charIndices } from './text';

/**
 * The four findings a single character decides on its own: bidirectional
 * controls, invisibles, spaces that are not the space, and codepoints with no
 * assigned meaning. None needs a word or a script to judge, so none is gated by
 * the script context — a right-to-left override is the same override in a
 * Chinese file as in an English one.
 *
 * The tables are written out by hand, as the crate's are, because a property
 * would sweep in characters that are not hazards and miss ones that are.
 */

/** The Trojan Source class — CVE-2021-42574. */
export const BIDI_CONTROLS: readonly (readonly [number, string])[] = [
	[0x061c, 'arabic letter mark'],
	[0x202a, 'left-to-right embedding'],
	[0x202b, 'right-to-left embedding'],
	[0x202c, 'pop directional formatting'],
	[0x202d, 'left-to-right override'],
	[0x202e, 'right-to-left override'],
	[0x2066, 'left-to-right isolate'],
	[0x2067, 'right-to-left isolate'],
	[0x2068, 'first strong isolate'],
	[0x2069, 'pop directional isolate'],
];

/** Characters that occupy a position and render as nothing. */
export const INVISIBLES: readonly (readonly [number, string])[] = [
	[0x00ad, 'soft hyphen'],
	[0x180e, 'mongolian vowel separator'],
	[0x200b, 'zero width space'],
	[0x200c, 'zero width non-joiner'],
	[0x200d, 'zero width joiner'],
	[0x2060, 'word joiner'],
	[0xfeff, 'zero width no-break space'],
];

/** Spaces that are not U+0020: they survive a trim and fail an equality test. */
export const UNUSUAL_SPACES: readonly (readonly [number, string])[] = [
	[0x00a0, 'no-break space'],
	[0x1680, 'ogham space mark'],
	[0x2000, 'en quad'],
	[0x2001, 'em quad'],
	[0x2002, 'en space'],
	[0x2003, 'em space'],
	[0x2004, 'three-per-em space'],
	[0x2005, 'four-per-em space'],
	[0x2006, 'six-per-em space'],
	[0x2007, 'figure space'],
	[0x2008, 'punctuation space'],
	[0x2009, 'thin space'],
	[0x200a, 'hair space'],
	[0x202f, 'narrow no-break space'],
	[0x205f, 'medium mathematical space'],
	[0x3000, 'ideographic space'],
];

/** Fixed by the standard and never extended, so safe to write down. */
const PRIVATE_USE: readonly (readonly [number, number])[] = [
	[0xe000, 0xf8ff],
	[0xf0000, 0xffffd],
	[0x100000, 0x10fffd],
];

const BY_CODEPOINT = new Map<
	number,
	Omit<Draft, 'index' | 'codepoints' | 'scripts' | 'resembles'>
>();
for (const [cp, name] of BIDI_CONTROLS) {
	BY_CODEPOINT.set(cp, {
		kind: 'bidi-control',
		severity: 'high',
		detail: `${name}: a bidirectional control reorders how the rest of the line renders, so the text a reviewer reads is not the text that runs`,
	});
}
for (const [cp, name] of INVISIBLES) {
	BY_CODEPOINT.set(cp, {
		kind: 'invisible',
		severity: 'medium',
		detail: `${name}: takes up no width, so two strings that are not equal look identical`,
	});
}
for (const [cp, name] of UNUSUAL_SPACES) {
	BY_CODEPOINT.set(cp, {
		kind: 'unusual-whitespace',
		severity: 'low',
		detail: `${name}: renders like a space and is not one, so a trim, a split or a comparison against U+0020 does not see it`,
	});
}

export function scan(content: string): Draft[] {
	const drafts: Draft[] = [];
	for (const { index, cp } of charIndices(content)) {
		// A byte-order mark at the very front is the file announcing its
		// encoding. The same character anywhere else is a zero-width no-break
		// space inside a string, and that is a finding.
		if (index === 0 && cp === 0xfeff) continue;
		const found = classify(cp);
		if (!found) continue;
		drafts.push({
			index,
			...found,
			codepoints: [render(cp)],
			scripts: [scriptName(scriptOf(cp))],
			resembles: [],
		});
	}
	return drafts;
}

function classify(
	cp: number,
): Omit<Draft, 'index' | 'codepoints' | 'scripts' | 'resembles'> | undefined {
	if (cp < 0x80) return undefined;
	const named = BY_CODEPOINT.get(cp);
	if (named) return named;
	if (scriptOf(cp) !== UNKNOWN) return undefined;
	// Unassigned, private-use, noncharacter and surrogate all resolve to
	// `Unknown` in UAX #24, which is exactly this finding.
	return {
		kind: 'unassigned-or-private-use',
		severity: 'medium',
		detail: inPrivateUse(cp)
			? 'a private-use codepoint: its meaning is whatever the system that produced it decided, and no other system agrees'
			: `not assigned in Unicode ${UNICODE_VERSION}: no renderer, no collation and no agreed meaning`,
	};
}

export function inPrivateUse(cp: number): boolean {
	return PRIVATE_USE.some(([first, last]) => cp >= first && cp <= last);
}
