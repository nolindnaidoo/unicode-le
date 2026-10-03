/**
 * Every non-ASCII code unit, as `\uXXXX` — on a serialized JSON document for
 * the wire, and on a path or key path for the report.
 *
 * Applied to the finished document, never to a field: escaping first would have
 * `JSON.stringify` escape the backslash, and the value would parse back as six
 * literal characters. Sound because JSON's own syntax is ASCII, so a non-ASCII
 * code unit can only sit inside a string literal, where the escape is defined.
 * Code units, not codepoints: an astral character leaves as its surrogate pair,
 * which is the only spelling the four-digit escape has for it.
 */
export function escapeNonAscii(json: string): string {
	return json.replace(
		/[\u0080-\uffff]/g,
		(unit) =>
			`\\u${unit.charCodeAt(0).toString(16).toUpperCase().padStart(4, '0')}`,
	);
}
