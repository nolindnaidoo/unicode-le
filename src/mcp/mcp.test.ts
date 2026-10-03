import { EventEmitter } from 'node:events';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { resolveFormat, SUPPORTED_FORMATS } from '../detection';
import { escapeNonAscii } from '../utils/escape';
import { capped, isOk, readMaxResults } from './envelope';
import { TOOLS } from './tools';
import { createResponder, serve } from './transport';

/**
 * The MCP layer: the envelope, the one tool, and the protocol.
 *
 * The tool is shared with the crate's server, so its answers are pinned by the
 * crate's own corpus here as well as there. What this file adds is the rule
 * this tool exists under: nothing the server writes may carry a character it
 * found.
 */

const CORPUS = join(__dirname, '..', '..', 'crate', 'fixtures');
const tool = TOOLS[0] as (typeof TOOLS)[number];
// Async, so a refusal thrown before the first await arrives as a rejection,
// which is how the transport sees it.
const call = async (args: Record<string, unknown>) =>
	(await tool.handler(args)) as Record<string, any>;

describe('envelope', () => {
	it('is ok with no diagnostics and with a warning, and not with an error', () => {
		expect(isOk([])).toBe(true);
		expect(
			isOk([
				{
					severity: 'warning',
					code: 'intentional_script_context',
					message: 'm',
				},
			]),
		).toBe(true);
		expect(isOk([{ severity: 'error', code: 'x', message: 'm' }])).toBe(false);
	});

	it('reports truncation honestly when it drops items', () => {
		expect(capped([1, 2, 3], 2)).toEqual({ items: [1, 2], truncated: true });
		expect(capped([1, 2], 5)).toEqual({ items: [1, 2], truncated: false });
	});

	it('rejects a maxResults a tool cannot honour, and clamps an oversized one', () => {
		expect(() => readMaxResults({ maxResults: 0 })).toThrow(/positive integer/);
		expect(() => readMaxResults({ maxResults: 1.5 })).toThrow();
		expect(readMaxResults({ maxResults: 999999 })).toBe(5000);
	});
});

describe('format resolution', () => {
	it('resolves names, extensions and dotfiles, and falls back to text', () => {
		expect(resolveFormat('yml', undefined)).toBe('yaml');
		expect(resolveFormat(' .TOML ', undefined)).toBe('toml');
		expect(resolveFormat(undefined, 'src/config/.env')).toBe('env');
		expect(resolveFormat(undefined, 'notes.json/readme')).toBe('text');
		expect(resolveFormat('klingon', 'a.properties')).toBe('ini');
		expect(resolveFormat(undefined, undefined)).toBe('text');
	});

	it('advertises exactly the formats the crate does', () => {
		expect(SUPPORTED_FORMATS).toEqual([
			'json',
			'yaml',
			'csv',
			'tsv',
			'toml',
			'ini',
			'env',
			'text',
		]);
	});
});

describe('tool table', () => {
	it('pins the tool name', () => {
		expect(TOOLS.map((t) => t.name)).toEqual(['detect_unicode_risks']);
	});

	it('offers no way to rewrite a document, because nothing here does', () => {
		const properties = Object.keys(tool.inputSchema.properties as object);
		for (const absent of ['fix', 'normalize', 'form', 'output', 'replace']) {
			expect(properties).not.toContain(absent);
		}
		expect(tool.inputSchema.additionalProperties).toBe(false);
	});
});

describe('detect_unicode_risks: the shared corpus', () => {
	const cases = JSON.parse(
		readFileSync(join(CORPUS, 'mcp-detect-unicode.json'), 'utf8'),
	) as {
		name: string;
		file?: string;
		content?: string;
		arguments: Record<string, unknown>;
		expected?: unknown;
		expectedError?: string;
	}[];

	for (const testCase of cases) {
		it(testCase.name, async () => {
			const args = { ...testCase.arguments };
			if (testCase.file !== undefined) {
				args.content = readFileSync(
					join(CORPUS, 'documents', testCase.file),
					'utf8',
				);
			} else if (testCase.content !== undefined) {
				args.content = testCase.content;
			}
			if (testCase.expectedError !== undefined) {
				await expect(call(args)).rejects.toThrow(testCase.expectedError);
				return;
			}
			expect(JSON.parse(JSON.stringify(await call(args)))).toEqual(
				testCase.expected,
			);
		});
	}
});

describe('detect_unicode_risks: arguments', () => {
	it("refuses each malformed argument by name, in the crate's order", async () => {
		await expect(call({})).rejects.toThrow(
			'content is required and must be a string',
		);
		await expect(
			call({ content: 'x', maxResults: 0, kinds: 'bidi' }),
		).rejects.toThrow('maxResults');
		await expect(call({ content: 'x', kinds: 'bidi' })).rejects.toThrow(
			'kinds must be a list of strings',
		);
		await expect(call({ content: 'x', kinds: ['homoglyph'] })).rejects.toThrow(
			'homoglyph is not a kind',
		);
		await expect(call({ content: 'x', scripts: ['Klingon'] })).rejects.toThrow(
			'Klingon is not a Unicode script',
		);
		await expect(call({ content: 'x', scripts: ['Common'] })).rejects.toThrow(
			'not a writing system',
		);
		await expect(call({ content: 'x', format: 3 })).rejects.toThrow(
			'format must be a string',
		);
	});

	it('accepts a format nothing recognises, which costs key paths and no findings', async () => {
		const result = await call({
			content: '{"a":"x\u202Ey"}',
			format: 'klingon',
		});
		expect(result.meta.count).toBe(1);
		expect(result.data.findings[0].key).toBeUndefined();
	});

	it('names where a finding sits when the format allows it', async () => {
		const result = await call({
			content: '{"a":{"b":"x\u202Ey"}}',
			filename: 'messages.json',
		});
		expect(result.data.findings[0].key).toBe('a.b');
	});

	it('refuses an unpaired surrogate rather than scanning a character that is not there', async () => {
		await expect(call({ content: 'a\ud800b' })).rejects.toThrow(
			'unpaired surrogate',
		);
	});
});

describe('the report-safety rule', () => {
	const hostile =
		'{"a\u202Eb":"p\u0430ypal \u200B \uFF26 caf\u0065\u0301 \u{1D41A}dmin"}';

	it('never returns a character it found, even inside a key path', async () => {
		const respond = createResponder(
			{ name: 'unicode-le', version: '1.0.0' },
			TOOLS,
		);
		const reply = await respond({
			jsonrpc: '2.0',
			id: 1,
			method: 'tools/call',
			params: {
				name: 'detect_unicode_risks',
				arguments: { content: hostile, format: 'json' },
			},
		});
		const wire = escapeNonAscii(JSON.stringify(reply));
		expect(/^[\x20-\x7e]*$/.test(wire)).toBe(true);
		// And the escape is lossless: a client reads the identical reply back.
		expect(JSON.parse(wire)).toEqual(JSON.parse(JSON.stringify(reply)));
	});

	it('escapes an astral character as its surrogate pair', () => {
		expect(escapeNonAscii('"\u{1D41A}"')).toBe('"\\uD835\\uDC1A"');
		expect(escapeNonAscii('plain')).toBe('plain');
	});
});

describe('protocol', () => {
	const respond = createResponder(
		{ name: 'unicode-le', version: '1.0.0' },
		TOOLS,
	);

	it('echoes the protocol version the client asked for', async () => {
		const reply = await respond({
			jsonrpc: '2.0',
			id: 1,
			method: 'initialize',
			params: { protocolVersion: '2024-11-05' },
		});
		expect(reply?.result?.protocolVersion).toBe('2024-11-05');
		expect(reply?.result?.serverInfo).toEqual({
			name: 'unicode-le',
			version: '1.0.0',
		});
	});

	it('does not reply to a notification', async () => {
		// A reply to a notification is the classic way to wedge a client.
		expect(
			await respond({ jsonrpc: '2.0', method: 'notifications/initialized' }),
		).toBeNull();
	});

	it('reports an unknown method as a JSON-RPC error', async () => {
		const reply = await respond({ jsonrpc: '2.0', id: 2, method: 'nope' });
		expect(reply?.error?.code).toBe(-32601);
	});

	it('reports an unknown tool without killing the connection', async () => {
		const reply = await respond({
			jsonrpc: '2.0',
			id: 3,
			method: 'tools/call',
			params: { name: 'no_such_tool', arguments: {} },
		});
		expect(reply?.error?.code).toBe(-32602);
	});

	it('returns a tool failure as a result, not a protocol error', async () => {
		// A model can read an isError result and correct itself; a JSON-RPC error
		// reads as "the server is broken".
		const reply = await respond({
			jsonrpc: '2.0',
			id: 4,
			method: 'tools/call',
			params: { name: 'detect_unicode_risks', arguments: {} },
		});
		expect(reply?.error).toBeUndefined();
		expect(reply?.result?.isError).toBe(true);
	});
});

describe('serve: the stdio loop', () => {
	/** A fake stdin/stdout pair so the loop can be driven without a process. */
	function harness() {
		const input = new EventEmitter() as EventEmitter & {
			setEncoding?: (e: string) => void;
		};
		const written: string[] = [];
		const output = {
			write: (chunk: string) => {
				written.push(chunk);
				return true;
			},
		};
		serve(
			{ name: 'unicode-le', version: '1.0.0' },
			TOOLS,
			input as never,
			output as never,
		);
		const replies = () =>
			written
				.join('')
				.split('\n')
				.filter(Boolean)
				.map((l) => JSON.parse(l));
		return { input, replies };
	}

	const settle = () => new Promise((r) => setTimeout(r, 20));

	it('answers a request delivered as one line', async () => {
		const { input, replies } = harness();
		input.emit('data', '{"jsonrpc":"2.0","id":1,"method":"tools/list"}\n');
		await settle();
		expect(replies()[0]?.result?.tools).toHaveLength(1);
	});

	it('reassembles a request split across chunks', async () => {
		// stdin delivers whatever the OS gives it; a request arriving in two
		// pieces must not be dropped or double-parsed.
		const { input, replies } = harness();
		input.emit('data', '{"jsonrpc":"2.0","id":2,"me');
		input.emit('data', 'thod":"ping"}\n');
		await settle();
		expect(replies()[0]?.id).toBe(2);
	});

	it('handles several requests in one chunk', async () => {
		const { input, replies } = harness();
		input.emit(
			'data',
			'{"jsonrpc":"2.0","id":3,"method":"ping"}\n{"jsonrpc":"2.0","id":4,"method":"ping"}\n',
		);
		await settle();
		expect(replies().map((r) => r.id)).toEqual([3, 4]);
	});

	it('reports malformed JSON without dying', async () => {
		// One bad line from a client must not take the server down for everyone.
		const { input, replies } = harness();
		input.emit('data', 'not json at all\n');
		input.emit('data', '{"jsonrpc":"2.0","id":5,"method":"ping"}\n');
		await settle();
		expect(replies()[0]?.error?.code).toBe(-32700);
		expect(replies()[1]?.id).toBe(5);
	});

	it('rejects a payload that is not a JSON-RPC request', async () => {
		const { input, replies } = harness();
		input.emit('data', '{"hello":"world"}\n');
		await settle();
		expect(replies()[0]?.error?.code).toBe(-32700);
	});

	it('ignores blank lines', async () => {
		const { input, replies } = harness();
		input.emit('data', '\n\n{"jsonrpc":"2.0","id":6,"method":"ping"}\n');
		await settle();
		expect(replies()).toHaveLength(1);
	});

	it('writes nothing for a notification', async () => {
		const { input, replies } = harness();
		input.emit(
			'data',
			'{"jsonrpc":"2.0","method":"notifications/initialized"}\n',
		);
		await settle();
		expect(replies()).toHaveLength(0);
	});
});
