/**
 * `detect_unicode_risks` is offered by BOTH servers — the npm one in
 * `src/mcp/tools.ts` and the Rust one in `crate/src/mcp/detect.rs`. One tool
 * name, one schema, two implementations. An agent asking that tool must get the
 * same answer whichever server it reaches, so the contract is identical output,
 * not similar output.
 *
 * `crate/fixtures/mcp-detect-unicode.json` pins that over cases somebody thought
 * of. This generates them instead: documents assembled from hazards, words in a
 * dozen scripts, structured wrappers and codepoints drawn from the whole of
 * Unicode — including the newest, which is where an engine's own tables and the
 * crate's would part company — fed to both servers and compared whole.
 *
 * It also holds the property the product rests on, over every generated
 * document: **no answer from either server may contain a non-ASCII character.**
 * A report that carried one could be carrying the thing it found.
 *
 * What is NOT compared: the CLI against the extension. Those are two surfaces
 * with two jobs, and only the shared tool is held to identity.
 *
 * Run: bun scripts/check-detection-differential.ts
 *   UNICODE_LE_DIFFERENTIAL_SEED=<n>  reproduce a specific failure
 *   UNICODE_LE_DIFFERENTIAL_CASES=<n> how many documents (default 1500)
 *   UNICODE_LE_BIN=<path>             the Rust binary (default the release build)
 */
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { TOOLS } from '../src/mcp/tools';
import { escapeNonAscii } from '../src/utils/escape';

const ROOT = join(import.meta.dir, '..');
const BINARY = process.env.UNICODE_LE_BIN ?? join(ROOT, 'crate', 'target', 'release', 'unicode-le');
const SEED = Number(process.env.UNICODE_LE_DIFFERENTIAL_SEED ?? 20261003);
const CASES = Number(process.env.UNICODE_LE_DIFFERENTIAL_CASES ?? 1500);

/** Mulberry32: a named, seeded source, so a failing run reproduces from one number. */
function seeded(seed: number): () => number {
	let state = seed >>> 0;
	return () => {
		state = (state + 0x6d2b79f5) >>> 0;
		let t = state;
		t = Math.imul(t ^ (t >>> 15), t | 1);
		t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

const WORDS: readonly string[] = [
	'paypal', 'login', 'admin', 'total', 'snake_case_2', 'café', 'naïve', 'Łódź', 'İstanbul',
	'Привет', 'мир', 'Καλημέρα', 'κόσμε', '你好', '世界', 'こんにちは', '日本語テキスト', 'CSVストリーミング',
	'안녕하세요', '한국어와漢字', 'مرحبا', 'שלום', 'नमस्ते', 'สวัสดี', 'ㄅㄆㄇ', 'ᎠᎡᎢ',
];

const HAZARDS: readonly string[] = [
	'‪', '‫', '‬', '‭', '‮', '⁦', '⁧', '⁨', '⁩', '؜',
	'​', '‌', '‍', '⁠', '­', '᠎', '﻿',
	' ', ' ', ' ', ' ', ' ', ' ', ' ', '　',
	'', '', '\u{f0000}', '\u{10fffd}', '͸', '￿', '﷐',
	'pаypal', 'lοgin', 'ＦＩＬＥ', '\u{1d41a}dmin', 'K', '1ª 2º x² Hₙ',
	'設ＦＩ', 'café', 'résumé', '가', '각', '각',
	'ẹ́', 'ą́', '̈́', 'क़', 'Ω', 'Å', 'ȫ',
	'ൊ', 'ൊ', '\u{11131}\u{11127}', '\u{1d15e}', '\u{2f800}',
	'\u{1f468}‍\u{1f469}‍\u{1f467}', '\u{1f3f3}️‍\u{1f308}',
];

/** Escape-shaped text, which only the JSON reader may resolve. */
const ESCAPES: readonly string[] = ['\\n', '\\t', '\\"', '\\\\', '\\u0041', '\\u043f', '\\uП12', '\\П', '\\'];

/** A codepoint from anywhere in Unicode — every plane, assigned or not. */
function anyCodepoint(random: () => number): string {
	const pick = random();
	let cp: number;
	if (pick < 0.35) cp = 0x80 + Math.floor(random() * (0x3000 - 0x80));
	else if (pick < 0.6) cp = 0x3000 + Math.floor(random() * (0x10000 - 0x3000));
	else if (pick < 0.85) cp = 0x10000 + Math.floor(random() * 0x20000);
	else cp = 0x30000 + Math.floor(random() * (0x110000 - 0x30000));
	if (cp >= 0xd800 && cp <= 0xdfff) cp = 0xfffd;
	return String.fromCodePoint(cp);
}

type Wrapper = (key: string, value: string) => string;
const WRAPPERS: readonly (readonly [string, Wrapper, string])[] = [
	['json', (k, v) => `{\n  "${k}": { "nested": "${v}", "list": ["${v}", 12] }\n}\n`, 'json'],
	['yaml', (k, v) => `${k}:\n  - id: A\n    name: ${v}\n  - ${v}\n`, 'yaml'],
	['toml', (k, v) => `[${k}]\nvalue = "${v}"\n[[items]]\nname = ${v}\n`, 'toml'],
	['ini', (k, v) => `[${k}]\nurl = https://example.invalid/${v}\nother: ${v}\n`, 'ini'],
	['env', (k, v) => `export ${k.toUpperCase()}=${v} # note\nQUOTED="${v} # kept"\n`, 'env'],
	['csv', (k, v) => `id,${k},city\n1,"${v}",Paris\n2,${v},"x,y"\n`, 'csv'],
	['tsv', (k, v) => `id\t${k}\tcity\n1\t${v}\tParis\n`, 'tsv'],
	['text', (k, v) => `${k} ${v}\n`, 'text'],
	['source', (k, v) => `const ${k} = "${v}"; // ${v}\n`, 'text'],
];

const KIND_CHOICES: readonly (readonly string[])[] = [
	[], [], [], ['bidi'], ['confusable', 'mixed-script'], ['non-nfc', 'whitespace'], ['invisible', 'unassigned'],
];
const SCRIPT_CHOICES: readonly (readonly string[])[] = [
	[], [], [], ['Han'], ['Cyrillic'], ['han', 'Hira', 'Katakana'], ['Greek', 'Arabic'], ['Hangul', 'Han'],
];

interface Generated {
	readonly name: string;
	readonly args: Record<string, unknown>;
}

function generate(count: number, seed: number): Generated[] {
	const random = seeded(seed);
	const pick = <T>(list: readonly T[]): T => list[Math.floor(random() * list.length)] as T;
	const fragment = (): string => {
		const roll = random();
		if (roll < 0.35) return pick(WORDS);
		if (roll < 0.6) return pick(HAZARDS);
		if (roll < 0.72) return pick(ESCAPES);
		if (roll < 0.9) return Array.from({ length: 1 + Math.floor(random() * 4) }, () => anyCodepoint(random)).join('');
		return pick([' ', '_', '-', '.', ':', '"', '\t', '\r\n', '\n', '#', ',', '=']);
	};
	const value = (): string => Array.from({ length: 1 + Math.floor(random() * 6) }, fragment).join(pick(['', ' ', '']));

	const out: Generated[] = [];
	for (let index = 0; index < count; index++) {
		const [wrapperName, wrapper, format] = pick(WRAPPERS);
		let content = wrapper(pick(WORDS.slice(0, 8)), value());
		if (random() < 0.3) content += wrapper(pick(WORDS), value());
		if (random() < 0.15) content = `﻿${content}`;
		if (random() < 0.1) content = content.replace(/\n/g, '\r\n');
		// Pad some documents with English, so the script checks are judged
		// rather than refused.
		if (random() < 0.5) content = `${'the quick brown fox jumps over the lazy dog '.repeat(3)}\n${content}`;

		const args: Record<string, unknown> = { content };
		const addressing = random();
		if (addressing < 0.4) args.format = format;
		else if (addressing < 0.7) args.filename = `dir.json/file.${format}`;
		const kinds = pick(KIND_CHOICES);
		if (kinds.length > 0) args.kinds = kinds;
		const scripts = pick(SCRIPT_CHOICES);
		if (scripts.length > 0) args.scripts = scripts;
		if (random() < 0.1) args.maxResults = 1 + Math.floor(random() * 4);
		out.push({ name: `${index}:${wrapperName}`, args });
	}
	return out;
}

/** JSON with object keys in a fixed order, so "identical" means identical. */
function canonical(value: unknown): string {
	if (value === null || typeof value !== 'object') return JSON.stringify(value);
	if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
	const entries = Object.entries(value as Record<string, unknown>)
		.filter(([, item]) => item !== undefined)
		.sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
	return `{${entries.map(([k, v]) => `${JSON.stringify(k)}:${canonical(v)}`).join(',')}}`;
}

function show(text: string): string {
	const escaped = escapeNonAscii(JSON.stringify(text));
	return escaped.length > 600 ? `${escaped.slice(0, 600)}…"` : escaped;
}

async function fromNpm(documents: readonly Generated[]): Promise<string[]> {
	const tool = TOOLS.find((candidate) => candidate.name === 'detect_unicode_risks');
	if (!tool) throw new Error('the npm server no longer offers detect_unicode_risks');
	const answers: string[] = [];
	for (const document of documents) {
		try {
			answers.push(canonical(JSON.parse(JSON.stringify(await tool.handler(document.args)))));
		} catch (error) {
			answers.push(`error: ${error instanceof Error ? error.message : String(error)}`);
		}
	}
	return answers;
}

async function fromCrate(documents: readonly Generated[]): Promise<{ answers: string[]; wire: string }> {
	if (!existsSync(BINARY)) {
		throw new Error(`no binary at ${BINARY} — build it first: cd crate && cargo build --release`);
	}
	const child = Bun.spawn([BINARY, 'mcp'], { stdin: 'pipe', stdout: 'pipe', stderr: 'pipe' });
	// Drained concurrently with the write: the replies outgrow a pipe buffer.
	const draining = new Response(child.stdout).text();
	const requests = documents
		.map((document, id) =>
			JSON.stringify({
				jsonrpc: '2.0',
				id,
				method: 'tools/call',
				params: { name: 'detect_unicode_risks', arguments: document.args },
			}),
		)
		.join('\n');
	child.stdin.write(`${requests}\n`);
	child.stdin.end();
	const stdout = await draining;
	await child.exited;

	const answers: string[] = new Array(documents.length);
	for (const line of stdout.split('\n')) {
		if (line.trim().length === 0) continue;
		const response = JSON.parse(line) as {
			id: number;
			result?: { structuredContent?: unknown; isError?: boolean; content?: { text: string }[] };
			error?: unknown;
		};
		if (response.error !== undefined) {
			throw new Error(`the crate server refused document ${response.id}: ${JSON.stringify(response.error)}`);
		}
		answers[response.id] = response.result?.isError
			? `error: ${response.result.content?.[0]?.text}`
			: canonical(response.result?.structuredContent);
	}
	const missing = answers.findIndex((answer) => answer === undefined);
	if (missing !== -1) {
		throw new Error(`the crate server never answered document ${missing}: ${await new Response(child.stderr).text()}`);
	}
	return { answers, wire: stdout };
}

const documents = generate(CASES, SEED);
console.log(`differential: ${documents.length} generated documents, seed ${SEED}, binary ${BINARY.replace(ROOT, '.')}`);

const [npm, crate] = await Promise.all([fromNpm(documents), fromCrate(documents)]);
const failures: string[] = [];

if (!/^[\x00-\x7f]*$/.test(crate.wire)) {
	failures.push('the crate server wrote a non-ASCII character to stdout');
}
for (const [index, document] of documents.entries()) {
	const ours = npm[index] as string;
	if (!/^[\x00-\x7f]*$/.test(escapeNonAscii(ours))) {
		failures.push(`the npm server's escaped answer for "${document.name}" is not ASCII`);
	}
	if (ours !== crate.answers[index]) {
		failures.push(
			`the two detect_unicode_risks servers disagree on "${document.name}"\n` +
				`  seed:      ${SEED} (reproduce with UNICODE_LE_DIFFERENTIAL_SEED=${SEED})\n` +
				`  arguments: ${show(JSON.stringify({ ...document.args, content: undefined }))}\n` +
				`  content:   ${show(document.args.content as string)}\n` +
				`  npm:       ${show(ours)}\n` +
				`  crate:     ${show(crate.answers[index] ?? '')}\n` +
				'  This is the SHARED tool. One name, one schema, two servers — a caller\n' +
				'  must not be able to tell which one it reached. This is a bug, not a\n' +
				'  surface difference.',
		);
	}
}

if (failures.length > 0) {
	console.error(`\nDetection differential FAILED (${failures.length}):\n`);
	for (const failure of failures.slice(0, 8)) console.error(`- ${failure}\n`);
	if (failures.length > 8) console.error(`… and ${failures.length - 8} more`);
	process.exit(1);
}
const findings = npm.reduce((sum, answer) => sum + (answer.match(/"kind":/g)?.length ?? 0), 0);
console.log(
	`OK: both detect_unicode_risks servers answered identically for all ${documents.length} documents (${findings} findings), and neither wrote a non-ASCII character.`,
);
