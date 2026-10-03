import { utf8Length } from './text';

/**
 * UTF-16 index → line, column and UTF-8 byte offset, in one forward sweep.
 *
 * **Columns are UTF-16 code units, 1-based**, because that is what an editor's
 * ruler counts, and an index into a JavaScript string already is one. The byte
 * offset is the crate's, for callers that address the file rather than the
 * editor. Findings arrive in document order, so each lookup resumes where the
 * last one stopped and a minified single-line bundle costs one pass, not one
 * pass per finding.
 */
export interface Located {
	readonly line: number;
	readonly column: number;
	readonly offset: number;
}

export class Locator {
	private index = 0;
	private offset = 0;
	private line = 1;
	private lineStart = 0;

	constructor(private readonly content: string) {}

	at(target: number): Located {
		if (target < this.index) {
			this.index = 0;
			this.offset = 0;
			this.line = 1;
			this.lineStart = 0;
		}
		while (this.index < target && this.index < this.content.length) {
			const cp = this.content.codePointAt(this.index) as number;
			const width = cp > 0xffff ? 2 : 1;
			this.offset += utf8Length(cp);
			this.index += width;
			if (cp === 0x0a) {
				this.line++;
				this.lineStart = this.index;
			}
		}
		return {
			line: this.line,
			column: this.index - this.lineStart + 1,
			offset: this.offset,
		};
	}
}
