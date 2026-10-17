import { Readable } from 'node:stream';
import { describe, expect, it } from 'vitest';
import { parse } from './args';
import { serve } from './mcp';
import { run } from './run';
import { exitCode, report, scanBytes } from './scan';
import { FLAGS, USAGE } from './usage';

const OPTIONS = { kinds: [], expectedScripts: [] };
const isAscii = (text: string) =>
	[...text].every((character) => character.charCodeAt(0) < 0x80);
const TROJAN = 'const access = "user‮ admin";\nlet pаypal = 1;\n';

describe('usage', () => {
	it('names every flag the parser accepts, and no other', () => {
		const named = new Set(
			USAGE.split(/\s+/)
				.filter((word) => word.startsWith('--'))
				.map((word) => word.replace(/[,.:;)]+$/, ''))
				.filter((word) => word !== '--version' && word !== '--help'),
		);
		expect([...named].sort()).toEqual([...FLAGS].sort());
	});
});

describe('parse', () => {
	it('reads repeated, comma-separated kinds trimmed as Rust trims', () => {
		const cli = parse(['--kind', ' bidi ,invisible', '--kind', 'non-nfc', 'x']);
		expect(cli.detect.kinds).toEqual(['bidi-control', 'invisible', 'non-nfc']);
	});

	it('refuses what the crate refuses, in its words', () => {
		expect(() => parse(['--kind', ',,', 'x'])).toThrow('--kind needs a value');
		expect(() => parse(['--fail-on', 'some', 'x'])).toThrow(
			'some is not something to fail on; one of: any, bidi',
		);
		expect(() =>
			parse(['--fail-on', 'bidi', '--kind', 'invisible', 'x']),
		).toThrow('--fail-on bidi cannot be answered while --kind excludes bidi');
		expect(() => parse(['-', 'x'])).toThrow('- is not an option. Try --help.');
		expect(() => parse(['--stdin', 'x'])).toThrow('takes no file arguments');
	});
});

describe('scanBytes', () => {
	it('reports codepoints in the crate order and counts the bidi apart', () => {
		const file = scanBytes(Buffer.from(TROJAN), 'a.js', OPTIONS);
		expect(Object.keys(file)).toEqual([
			'file',
			'findings',
			'refusals',
			'summary',
		]);
		expect(Object.keys(file.findings[0] ?? {})).toEqual([
			'kind',
			'severity',
			'line',
			'column',
			'offset',
			'codepoints',
			'scripts',
			'detail',
		]);
		expect(file.summary.bidi).toBe(1);
		expect(JSON.stringify(file)).not.toContain('‮');
	});

	it('refuses a UTF-16 file rather than decoding it as UTF-8', () => {
		const file = scanBytes(
			Buffer.from([0xff, 0xfe, 0x68, 0x00]),
			'a.txt',
			OPTIONS,
		);
		expect(file.refusals[0]?.reason).toBe('encoding_unknown');
		expect(report([file]).summary.unexamined).toBe(1);
	});
});

describe('exitCode', () => {
	it('gates on bidi alone when asked, and on refusals under --strict', () => {
		const run = report([scanBytes(Buffer.from('a​b'), 'x', OPTIONS)]);
		expect(exitCode(run, 'any', false)).toBe(1);
		expect(exitCode(run, 'bidi', false)).toBe(0);
		const refused = report([scanBytes(Buffer.from([0, 1]), 'y', OPTIONS)]);
		expect(exitCode(refused, 'any', true)).toBe(2);
	});
});

describe('run', () => {
	it('escapes every non-ASCII character it writes', async () => {
		const out: string[] = [];
		const err: string[] = [];
		const code = await run(['--stdin'], {
			stdout: (text) => out.push(text),
			stderr: (text) => err.push(text),
			stdoutIsTerminal: false,
			readStdin: async () => Buffer.from(TROJAN),
			version: '9.9.9',
		});
		expect(code).toBe(1);
		expect(isAscii(`${out.join('')}${err.join('')}`)).toBe(true);
	});

	it('escapes a refusal that names a non-ASCII path', async () => {
		const err: string[] = [];
		const code = await run(['mïssing-here'], {
			stdout: () => {},
			stderr: (text) => err.push(text),
			stdoutIsTerminal: false,
			readStdin: async () => Buffer.alloc(0),
			version: '9.9.9',
		});
		expect(code).toBe(2);
		expect(err.join('')).toMatch(/^unicode-le: m\\u00EFssing-here: /);
	});
});

describe('mcp', () => {
	it('sorts every key and escapes the line it writes', async () => {
		const written: string[] = [];
		const request = {
			jsonrpc: '2.0',
			id: 'ïd',
			method: 'tools/call',
			params: { name: 'detect_unicode_risks', arguments: { content: TROJAN } },
		};
		await serve(
			Readable.from([Buffer.from(`${JSON.stringify(request)}\n`)]),
			(reply) => written.push(reply),
			'9.9.9',
		);
		expect(
			written[0]?.startsWith('{"id":"\\u00EFd","jsonrpc":"2.0","result":'),
		).toBe(true);
		expect(isAscii(written[0] ?? '')).toBe(true);
	});
});
