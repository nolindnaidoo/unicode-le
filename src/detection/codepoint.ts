/**
 * How an offending character reaches the report: as `U+XXXX`, and never as
 * itself.
 *
 * **This is the safety rule the whole tool rests on.** Every other scanner in
 * this family can quote what it found, because quoting it is inert. Here it is
 * not: a report that pasted a raw U+202E carries the attack out of the file and
 * into the terminal, the pull request and the chat window of whoever reads it.
 * So `codepoints` and `resembles` are the only representation of the characters
 * involved, and `detail` is English written here, never text from the document.
 */

/** Upper-case hex, at least four digits — the spelling Unicode documents use. */
export function render(cp: number): string {
	return `U+${cp.toString(16).toUpperCase().padStart(4, '0')}`;
}

export function renderEach(cps: Iterable<number>): string[] {
	return [...cps].map(render);
}
