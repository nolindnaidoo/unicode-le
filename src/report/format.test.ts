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

	it('keeps a backslash before a pipe from breaking out of its cell', () => {
		// The key path is the document's own text: `a`, two backslashes, `|b`.
		const document = `{"a${'\\'.repeat(2)}|b":"x\u202ey"}`;
		const { findings, refusals } = examine(document, 'json', {
			kinds: [],
			expectedScripts: [],
		});
		expect(findings[0]?.key).toBe(`a${'\\'.repeat(2)}|b`);
		const report = formatReport(
			[{ file: 'C:\\dir\\k.json', findings, refusals }],
			1,
		);
		// Markdown escapes a pipe only after an odd run of backslashes. Doubling
		// every backslash first makes the run odd whatever the document held.
		const run = report.match(/`a(\\+)\|b`/)?.[1] ?? '';
		expect(run.length % 2).toBe(1);
		expect(report).toContain('## C:\\\\dir\\\\k.json');
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
