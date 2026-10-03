import { describe, expect, it } from 'vitest';
import { examine } from '../detection';
import { formatReport } from './format';

const HAZARDS = [
	'\u202e',
	'\u2066',
	'\u200b',
	'\u0430',
	'\uff26',
	'\u00a0',
	'\ue000',
];
const OPTIONS = { kinds: [], expectedScripts: [] } as const;

describe('the report', () => {
	it('names every finding by position, kind, severity and codepoint, with its detail beneath', () => {
		const { findings, refusals } = examine(
			'let a = "\u202e";\nlet p\u0430ypal = 1;\n',
			'text',
			OPTIONS,
		);
		const report = formatReport([{ file: 'src/a.ts', findings, refusals }], 1);
		expect(report).toContain('## `src/a.ts`');
		expect(report).toContain(
			'- **1:10** · bidi-control · high · U+202E\n\n  right-to-left override:',
		);
		expect(report).toContain('U+0430 (resembles U+0061)');
		expect(report).toContain('3 finding(s) in 1 file(s) scanned');
	});

	it('names the key path a finding sits under', () => {
		const { findings, refusals } = examine(
			'{"auth":{"provider":"x\u202ey"}}',
			'json',
			OPTIONS,
		);
		expect(formatReport([{ file: 'a.json', findings, refusals }], 1)).toContain(
			'· Key `auth.provider`',
		);
	});

	/**
	 * The rule the tool exists under. A path and a key path are text from the
	 * document, so they are escaped; everything else is written here.
	 */
	it('never carries a character it found, even in a path or a key', () => {
		const content = `{"a${'\u202e'}b":{"c":"p\u0430ypal \u200b \uff26 x\u00a0y \ue000"}}`;
		const { findings, refusals } = examine(content, 'json', OPTIONS);
		const report = formatReport(
			[{ file: `invoice${'\u202e'}fdp.ts`, findings, refusals }],
			1,
		);
		for (const hazard of HAZARDS) expect(report).not.toContain(hazard);
		expect(report).toContain('`invoice\\u202Efdp.ts`');
	});

	it('doubles a backslash, so a file named like an escape is not read as one', () => {
		const { findings, refusals } = examine('\u202e', 'text', OPTIONS);
		const report = formatReport(
			[{ file: 'C:\\dir\\u202E.txt', findings, refusals }],
			1,
		);
		expect(report).toContain('## `C:\\\\dir\\\\u202E.txt`');
	});

	it('keeps a backtick in a key from closing its code span', () => {
		const { findings, refusals } = examine(
			'{"a`b":"x\u202ey"}',
			'json',
			OPTIONS,
		);
		expect(formatReport([{ file: 'k.json', findings, refusals }], 1)).toContain(
			"`a'b`",
		);
	});

	it('says what was not judged, and that silence is not a clearance', () => {
		const refused = examine('ключ: значение\nдругой: текст\n', 'yaml', OPTIONS);
		expect(formatReport([{ file: 'ru.yaml', ...refused }], 1)).toContain(
			'**Not judged** (`intentional_script_context`)',
		);
		expect(formatReport([], 4)).toContain('Silence is not a clearance');
	});
});
