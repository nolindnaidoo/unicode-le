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
			(reply) => {
				written.push(reply);
				return undefined;
			},
			'9.9.9',
		);
		expect(
			written[0]?.startsWith('{"id":"\\u00EFd","jsonrpc":"2.0","result":'),
		).toBe(true);
		expect(isAscii(written[0] ?? '')).toBe(true);
	});
});

describe('mcp: the crate framing', () => {
	// The protocol around the tools answers as the extension's server and
	// the crate's answer, so a client cannot tell the three apart by what it
	// sent. Replies are parsed: key order is the crate's concern, not this one.
	async function session(
		lines: readonly (string | Uint8Array)[],
	): Promise<{ status: number; replies: unknown[] }> {
		const written: string[] = [];
		const input = Readable.from(lines.map((item) => Buffer.from(item)));
		const status = await serve(
			input,
			(line) => {
				written.push(line);
				return undefined;
			},
			'9.9.9',
		);
		return { status, replies: written.map((line) => JSON.parse(line)) };
	}

	it("answers a malformed frame in the extension server's words and serves the next line", async () => {
		// A frame that is not JSON, or not UTF-8, was dropped without a word,
		// and a client lost every request it then waited on.
		const { status, replies } = await session([
			'{"jsonrpc":"2.0","id":1,"method":"initialize"}\n',
			'{"jsonrpc":"2.0","method":"notifications/initialized"}\n',
			'not json\n',
			Uint8Array.from([0x7b, 0xff, 0x7d, 0x0a]),
			'   \n',
			'{"jsonrpc":"2.0","id":2,"method":"nope"}\n',
		]);
		expect(status).toBe(0);
		expect(replies).toEqual([
			{
				jsonrpc: '2.0',
				id: 1,
				result: {
					protocolVersion: '2025-06-18',
					capabilities: { tools: {} },
					serverInfo: { name: 'unicode-le', version: '9.9.9' },
				},
			},
			{
				jsonrpc: '2.0',
				id: null,
				error: { code: -32700, message: 'Invalid JSON' },
			},
			{
				jsonrpc: '2.0',
				id: null,
				error: { code: -32700, message: 'Invalid JSON' },
			},
			{
				jsonrpc: '2.0',
				id: 2,
				error: { code: -32601, message: 'Unknown method: nope' },
			},
		]);
	});

	it('refuses malformed frames in the words the other two servers use, and a null id is a notification', async () => {
		const { replies } = await session([
			'{"id":1,"method":"ping"}\n',
			'{"jsonrpc":"1.0","id":2,"method":"ping"}\n',
			'[{"jsonrpc":"2.0","id":3,"method":"ping"}]\n',
			'{"jsonrpc":"2.0","id":null,"method":"ping"}\n',
			'{"jsonrpc":"2.0","id":4,"method":"notifications/initialized"}\n',
			'{"jsonrpc":"2.0","id":5,"method":"tools/call","params":5}\n',
			'{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"nope"}}\n',
			'{"jsonrpc":"2.0","id":7,"method":"tools/call"}\n',
			'{"jsonrpc":"2.0","id":8,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}\n',
		]);
		const refused = {
			jsonrpc: '2.0',
			id: null,
			error: { code: -32700, message: 'Not a JSON-RPC 2.0 request' },
		};
		expect(replies).toEqual([
			refused,
			refused,
			refused,
			{
				jsonrpc: '2.0',
				id: 4,
				error: {
					code: -32601,
					message: 'Unknown method: notifications/initialized',
				},
			},
			{
				jsonrpc: '2.0',
				id: 5,
				error: { code: -32602, message: 'tools/call requires a string `name`' },
			},
			{
				jsonrpc: '2.0',
				id: 6,
				error: { code: -32602, message: 'Unknown tool: nope' },
			},
			{
				jsonrpc: '2.0',
				id: 7,
				error: { code: -32602, message: 'tools/call requires a string `name`' },
			},
			{
				jsonrpc: '2.0',
				id: 8,
				result: {
					capabilities: { tools: {} },
					protocolVersion: '2025-06-18',
					serverInfo: { name: 'unicode-le', version: '9.9.9' },
				},
			},
		]);
	});

	it('waits for the pipe to drain before reading the next frame', async () => {
		const lines: string[] = [];
		let drained = 0;
		const input = Readable.from([
			Buffer.from(
				'{"jsonrpc":"2.0","id":1,"method":"ping"}\n{"jsonrpc":"2.0","id":2,"method":"ping"}\n',
			),
		]);
		const write = (line: string) => {
			lines.push(line);
			return new Promise<void>((resolve) =>
				setTimeout(() => {
					drained += 1;
					resolve();
				}, 1),
			);
		};
		await serve(input, write, '9.9.9');
		expect(lines).toHaveLength(2);
		expect(drained).toBe(2);
	});
});
