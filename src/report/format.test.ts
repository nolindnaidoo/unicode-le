import { describe, expect, it } from 'vitest';
import { examine } from '../detection';
import { formatReport } from './format';

const HAZARDS = ['‮', '⁦', '​', 'а', 'Ｆ', ' ', ''];

describe('the report', () => {
	it('names every finding by line, column, kind and codepoint', () => {
		const { findings, refusals } = examine(
			'let a = "‮";\nlet pаypal = 1;\n',
			'text',
			{
				kinds: [],
				expectedScripts: [],
			},
		);
		const report = formatReport([{ file: 'src/a.ts', findings, refusals }], 1);
		expect(report).toContain('## src/a.ts');
		expect(report).toContain('| 1 | 10 | bidi-control | high | U+202E |');
		expect(report).toContain('U+0430 (resembles U+0061)');
		expect(report).toContain('3 finding(s) in 1 file(s) scanned');
	});

	/**
	 * The rule the tool exists under. A path and a key path are text from the
	 * document, so they are escaped; everything else is written here.
	 */
	it('never carries a character it found, even in a path or a key', () => {
		const content = `{"a${'‮'}b":{"c":"pаypal ​ Ｆ x y "}}`;
		const { findings, refusals } = examine(content, 'json', {
			kinds: [],
			expectedScripts: [],
		});
		const report = formatReport(
			[{ file: `invoice${'‮'}fdp.ts`, findings, refusals }],
			1,
		);
		for (const hazard of HAZARDS) expect(report).not.toContain(hazard);
		expect(report).toContain('invoice\\u202Efdp.ts');
	});

	it('keeps a key path from breaking out of its table cell', () => {
		const { findings, refusals } = examine('{"a|b`c":"x‮y"}', 'json', {
			kinds: [],
			expectedScripts: [],
		});
		const report = formatReport([{ file: 'k.json', findings, refusals }], 1);
		expect(report).toContain("`a\\|b'c`");
	});

	it('says what was not judged, and that silence is not a clearance', () => {
		const refused = examine('ключ: значение\nдругой: текст\n', 'yaml', {
			kinds: [],
			expectedScripts: [],
		});
		expect(formatReport([{ file: 'ru.yaml', ...refused }], 1)).toContain(
			'**Not judged** (`intentional_script_context`)',
		);
		expect(formatReport([], 4)).toContain('Silence is not a clearance');
	});
});
