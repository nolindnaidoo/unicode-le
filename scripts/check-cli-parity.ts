/**
 * Requires the npm command line to behave exactly as the Rust one does.
 *
 * Both install a command named `unicode-le`, so a person or a CI step cannot tell
 * which one they have. This is a different contract from
 * `check-extraction-parity.ts`, which holds the extension's engine equal to
 * the crate's: here the whole terminal surface is compared, the walk, the
 * findings and refusals, the escaping, the exit codes, stdout and stderr
 * byte for byte, and every reply `unicode-le mcp` writes, byte for byte too.
 *
 * Every case runs both binaries over one generated tree with stdout piped,
 * which is how a script sees them. `--version` is compared too: every surface
 * carries one version, and two binaries of one name disagreeing about it is
 * the first thing a person comparing them would see.
 *
 * Run: bun run build:cli && cargo build --locked --manifest-path crate/Cargo.toml
 *      bun scripts/check-cli-parity.ts
 */
import { spawnSync } from 'node:child_process';
import {
	chmodSync,
	existsSync,
	mkdirSync,
	mkdtempSync,
	rmdirSync,
	symlinkSync,
	unlinkSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const ROOT = join(import.meta.dir, '..');
const RUST =
	process.env.UNICODE_LE_BIN ??
	join(
		ROOT,
		'crate',
		'target',
		'debug',
		`unicode-le${process.platform === 'win32' ? '.exe' : ''}`,
	);
const NODE_CLI = join(ROOT, 'cli', 'cli.js');
/**
 * Node, by name, not `process.execPath`: this script runs under Bun, and the
 * package is run by Node wherever `npx` runs it. Bun's errors are not Node's.
 */
const NODE = process.env.NODE_BIN ?? 'node';

interface Case {
	readonly args: readonly string[];
	readonly stdin?: string | Buffer;
}

interface Outcome {
	readonly status: number | null;
	readonly stdout: string;
	readonly stderr: string;
}

for (const [label, path] of [
	['the Rust binary', RUST],
	['cli/cli.js', NODE_CLI],
] as const) {
	if (!existsSync(path)) {
		console.error(`FAIL: ${label} is missing at ${path}. Build it first.`);
		process.exit(1);
	}
}

const tree = mkdtempSync(join(tmpdir(), 'unicode-le-cli-parity-'));
const created: string[] = [];
const dirs: string[] = [tree];

function dir(relative: string): void {
	mkdirSync(join(tree, relative));
	dirs.push(join(tree, relative));
}

function file(relative: string, content: string | Buffer): void {
	writeFileSync(join(tree, relative), content);
	created.push(join(tree, relative));
}

function link(relative: string, target: string): void {
	// Windows grants symlinks to administrators and developer mode only, and
	// both binaries then see the same tree without the link.
	try {
		symlinkSync(target, join(tree, relative));
	} catch {
		return;
	}
	created.push(join(tree, relative));
}

const canLock = process.platform !== 'win32' && process.getuid?.() !== 0;

const TROJAN = [
	'const access = "user\u202E \u2066// admin\u2069";',
	'let p\u0430ypal = 1;',
	'const caf\u0065\u0301 = "x\u00A0y\u200Bz";',
	'const pua = "\uE000";',
	'const astral = "🎯 then \u{1F3FB}";',
	'',
].join('\n');

function build(): void {
	dir('repo');
	dir('repo/.git');
	file('repo/.gitignore', 'ignored.js\nbuild/\n');
	file('repo/.ignore', 'dot-ignored.js\n');
	file('repo/app.js', TROJAN);
	file('repo/config.json', '{"title": "Hello", "nested": {"key": "a\u200Bb", "name": "p\u0430ypal"}}\n');
	file('repo/config.yaml', 'title: "x\u202Ey"\nlist:\n  - "a\u00A0b"\n');
	file('repo/settings.toml', '[server]\nhost = "ex\u0430mple.com"\n');
	file('repo/.env', 'KEY=va\u200Blue\n');
	file('repo/setup.ini', '[a]\nb = c\u00A0d\n');
	file('repo/rows.csv', 'a,b\nx\u200B,y\n');
	file('repo/ru.json', '{"greeting": "Привет мир", "x": "a\u200Bb"}\n');
	file('repo/ja.md', 'これは日本語の文章です。\u200B\n');
	file('repo/nfd.txt', 'cafe\u0301\n');
	file('repo/clean.ts', 'export const x = 1;\n');
	file('repo/crlf.ts', 'a\u200B\r\nb\r\n');
	file('repo/bom.ts', '\uFEFFconst a = "\u202E";\n');
	file('repo/empty.txt', '');
	file('repo/utf16le.txt', Buffer.from([0xff, 0xfe, 0x68, 0x00, 0x0b, 0x20]));
	file('repo/utf16be.txt', Buffer.from([0xfe, 0xff, 0x00, 0x68, 0x20, 0x0b]));
	file('repo/latin1.txt', Buffer.from([0x63, 0x61, 0x66, 0xe9, 0x0a]));
	file('repo/binary.dat', Buffer.from([0x00, 0x01, 0x02, 0x03]));
	// A refusal whose path is not ASCII: its line is escaped like any other.
	file('repo/bïnäry.dat', Buffer.from([0x00, 0x01, 0x02, 0x03]));
	file('repo/ünïcödé.txt', 'a\u200Bb\n');
	file('repo/ignored.js', '"\u202E"\n');
	file('repo/dot-ignored.js', '"\u202E"\n');
	file('repo/.hidden.js', '"\u202E"\n');
	dir('repo/build');
	file('repo/build/out.js', '"\u202E"\n');
	dir('repo/nested');
	file('repo/nested/.gitignore', 'local.js\n');
	file('repo/nested/local.js', '"\u202E"\n');
	file('repo/nested/kept.js', '"\u2066x\u2069"\n');
	link('repo/linked.js', 'app.js');
	link('repo/linked-dir', 'nested');
	dir('loose');
	file('loose/plain.txt', 'x\u200By\n');
	dir('many');
	file('many/big.txt', 'word\u200B with\u00A0spaces and p\u0430ypal\n'.repeat(3000));
	if (canLock) {
		file('repo/locked.txt', 'x\n');
		chmodSync(join(tree, 'repo', 'locked.txt'), 0o000);
		dir('closed');
		dir('closed/shut');
		file('closed/shut/inside.txt', 'x\n');
		file('closed/open.txt', '\u202E\n');
		chmodSync(join(tree, 'closed', 'shut'), 0o000);
	}
}

function cases(): readonly Case[] {
	const repo = join(tree, 'repo');
	const at = (name: string) => join(repo, name);
	const corpus = join(ROOT, 'crate', 'fixtures', 'documents');
	return [
		{ args: ['--help'] },
		{ args: ['-h'] },
		{ args: ['--version'] },
		{ args: ['-V'] },
		{ args: [repo, '--help'] },
		{ args: [] },
		{ args: [repo] },
		{ args: [`${repo}/`] },
		{ args: ['repo'] },
		{ args: ['./repo/nested/'] },
		{ args: [repo, '--hidden'] },
		{ args: [repo, '--no-ignore'] },
		{ args: [repo, '--hidden', '--no-ignore'] },
		{ args: [repo, '--strict'] },
		{ args: [repo, '--kind', 'bidi'] },
		{ args: [repo, '--kind', 'bidi,invisible', '--kind', 'confusable'] },
		{ args: [repo, '--kind', ' bidi , non-nfc '] },
		{ args: [repo, '--kind', 'bidi-control,mixed-script'] },
		{ args: [repo, '--kind', 'emoji'] },
		{ args: [repo, '--kind', ',,'] },
		{ args: [repo, '--kind'] },
		{ args: [repo, '--script', 'Cyrillic'] },
		{ args: [repo, '--script', 'Cyrl,Han,Hiragana'] },
		{ args: [repo, '--script', 'Klingon'] },
		{ args: [repo, '--script'] },
		{ args: [repo, '--fail-on', 'bidi'] },
		{ args: [repo, '--fail-on', 'any'] },
		{ args: [repo, '--fail-on', 'bidi', '--kind', 'invisible'] },
		{ args: [repo, '--fail-on', 'bidi', '--kind', 'bidi,invisible'] },
		{ args: [repo, '--fail-on', 'some'] },
		{ args: [repo, '--fail-on', 'any,bidi'] },
		{ args: [repo, '--fail-on'] },
		{ args: [repo, '--nope'] },
		{ args: [repo, '-'] },
		{ args: [at('app.js')] },
		{ args: [at('config.json'), at('config.yaml'), at('settings.toml'), at('.env')] },
		{ args: [at('setup.ini'), at('rows.csv'), at('ru.json'), at('ja.md')] },
		{ args: [at('ru.json'), '--script', 'Cyrillic'] },
		{ args: [at('nfd.txt'), at('clean.ts'), at('crlf.ts'), at('bom.ts'), at('empty.txt')] },
		{ args: [at('utf16le.txt'), at('utf16be.txt'), at('latin1.txt'), at('binary.dat')] },
		{ args: [at('binary.dat'), '--strict'] },
		{ args: [at('bïnäry.dat')] },
		{ args: [at('ünïcödé.txt')] },
		{ args: [at('ignored.js'), at('.hidden.js'), at('dot-ignored.js')] },
		{ args: [at('app.js'), at('app.js')] },
		{ args: [at('nested'), at('nested/kept.js')] },
		{ args: [at('linked.js')] },
		{ args: [at('linked-dir')] },
		{ args: [join(tree, 'loose')] },
		{ args: [join(tree, 'many')] },
		...(canLock
			? [
					{ args: [join(tree, 'closed')] },
					{ args: [at('locked.txt')] },
					{ args: [at('locked.txt'), '--strict'] },
				]
			: []),
		{ args: [join(tree, 'missing')] },
		{ args: [repo, join(tree, 'missing')] },
		{ args: [join(tree, 'mïssing')] },
		{ args: ['--stdin'] },
		{ args: ['--stdin', repo] },
		{ args: ['--stdin'], stdin: TROJAN },
		{ args: ['--stdin', '--kind', 'bidi'], stdin: TROJAN },
		{ args: ['--stdin'], stdin: `\uFEFF${TROJAN}` },
		{ args: ['--stdin'], stdin: Buffer.from([0xff, 0xfe, 0x68, 0x00]) },
		{ args: ['--stdin'], stdin: Buffer.from([0x63, 0xe9, 0x0a]) },
		{ args: ['--stdin'], stdin: '' },
		{ args: [corpus] },
		{ args: [corpus, '--hidden', '--no-ignore', '--script', 'Cyrillic,Han'] },
	];
}

function run(command: readonly string[], testCase: Case): Outcome {
	const [program, ...rest] = command as [string, ...string[]];
	const result = spawnSync(program, [...rest, ...testCase.args], {
		input: testCase.stdin ?? '',
		cwd: tree,
		maxBuffer: 256 * 1024 * 1024,
	});
	return {
		status: result.status,
		stdout: result.stdout.toString('utf8'),
		stderr: result.stderr.toString('utf8'),
	};
}

function compareCli(): string[] {
	const failures: string[] = [];
	for (const testCase of cases()) {
		const rust = run([RUST], testCase);
		const node = run([NODE, NODE_CLI], testCase);
		for (const field of ['status', 'stdout', 'stderr'] as const) {
			if (rust[field] === node[field]) continue;
			failures.push(
				`unicode-le ${testCase.args.join(' ')}\n  ${field} differs\n` +
					describeDifference(String(rust[field]), String(node[field])),
			);
		}
	}
	return failures;
}

function mcpLines(): readonly string[] {
	const repo = join(tree, 'repo');
	let id = 0;
	const request = (method: string, params?: unknown) =>
		JSON.stringify({
			jsonrpc: '2.0',
			id: ++id,
			method,
			...(params === undefined ? {} : { params }),
		});
	const call = (name: string, args?: unknown) =>
		request('tools/call', args === undefined ? { name } : { name, arguments: args });
	const detect = (args: Record<string, unknown>) => call('detect_unicode_risks', args);
	const scan = (args: Record<string, unknown>) => call('unicode_le_scan', args);
	return [
		request('initialize', { protocolVersion: '2024-11-05' }),
		JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }),
		request('tools/list'),
		request('ping'),
		request('resources/list'),
		request('tools/call'),
		request('tools/call', { arguments: {} }),
		request('tools/call', null),
		call('no_such_tool', {}),
		JSON.stringify({ jsonrpc: '2.0', id: 'tëxt-id', method: 'ping' }),
		JSON.stringify({ jsonrpc: '2.0', id: null, method: 'ping' }),
		JSON.stringify({ id: 77, method: 'ping' }),
		JSON.stringify({ jsonrpc: '2.0', id: 78 }),
		'not json',
		'',
		'   ',
		'[1, 2]',
		`${request('ping')}\r`,
		detect({ content: TROJAN }),
		detect({ content: TROJAN, kinds: ['bidi'], maxResults: 1 }),
		detect({ content: '{"k": "Привет"}', format: 'json', scripts: ['Cyrillic'] }),
		detect({ content: 'host = "ex\u0430mple"', filename: 'a.toml' }),
		detect({ content: TROJAN, kinds: ['emoji'] }),
		detect({ content: TROJAN, scripts: ['Klingon'] }),
		detect({ content: TROJAN, kinds: 'bidi' }),
		detect({ content: 7 }),
		detect({}),
		call('detect_unicode_risks', null),
		scan({ path: repo }),
		scan({ paths: [repo, join(tree, 'loose')], kinds: ['bidi', 'invisible'] }),
		scan({ path: repo, hidden: true, ignored: true, scripts: ['Cyrillic'] }),
		scan({ path: repo, format: 'json' }),
		scan({ path: repo, format: 7 }),
		scan({ path: repo, kinds: ['emoji'] }),
		scan({ path: repo, kinds: 'bidi' }),
		scan({ path: join(repo, 'app.js'), hidden: 'yes' }),
		scan({ paths: [] }),
		scan({ paths: [3] }),
		scan({ path: 3, paths: [join(repo, 'clean.ts')] }),
		scan({}),
		scan({ path: join(tree, 'missing') }),
		...(canLock ? [scan({ path: join(tree, 'closed') })] : []),
	];
}

function serve(command: readonly string[], input: string | Buffer): Outcome {
	const [program, ...rest] = command as [string, ...string[]];
	const result = spawnSync(program, [...rest, 'mcp'], { input, cwd: tree });
	return {
		status: result.status,
		stdout: result.stdout.toString('utf8'),
		stderr: result.stderr.toString('utf8'),
	};
}

function compareMcp(): string[] {
	const failures: string[] = [];
	const lines = mcpLines();
	const sessions: readonly (readonly [string, string | Buffer])[] = [
		['the whole session', `${lines.join('\n')}\n`],
		['a last line with no newline', lines[3] as string],
		[
			'a line that is not UTF-8',
			Buffer.concat([
				Buffer.from(`${lines[3]}\n`),
				Buffer.from([0x7b, 0xff, 0x7d, 0x0a]),
				Buffer.from(`${lines[3]}\n`),
			]),
		],
	];
	for (const [label, input] of sessions) {
		const rust = serve([RUST], input);
		const node = serve([NODE, NODE_CLI], input);
		for (const field of ['status', 'stdout', 'stderr'] as const) {
			if (rust[field] === node[field]) continue;
			failures.push(
				`unicode-le mcp, ${label}\n  ${field} differs\n` +
					describeDifference(String(rust[field]), String(node[field])),
			);
		}
	}
	return failures;
}

/** The first differing line, which is the one worth reading in a long output. */
function describeDifference(rust: string, node: string): string {
	const left = rust.split('\n');
	const right = node.split('\n');
	const index = left.findIndex((line, at) => line !== right[at]);
	const at = index === -1 ? left.length : index;
	const clip = (line: string | undefined) =>
		line === undefined || line.length <= 2000 ? line : `${line.slice(0, 2000)}…`;
	return `  line ${at + 1}\n  rust: ${clip(left[at])}\n  node: ${clip(right[at])}`;
}

function cleanUp(): void {
	const unlock: readonly (readonly [string, number])[] = [
		[join(tree, 'repo', 'locked.txt'), 0o600],
		[join(tree, 'closed', 'shut'), 0o700],
	];
	for (const [path, mode] of unlock) if (existsSync(path)) chmodSync(path, mode);
	for (const path of created) unlinkSync(path);
	for (const path of [...dirs].reverse()) rmdirSync(path);
}

build();
let failures: string[];
try {
	failures = [...compareCli(), ...compareMcp()];
} finally {
	cleanUp();
}

if (failures.length > 0) {
	console.error(failures.join('\n\n'));
	console.error(`\nFAIL: ${failures.length} difference(s) between the npm and Rust CLIs`);
	process.exit(1);
}
console.log(
	`OK: the npm CLI matches the Rust CLI on ${cases().length} commands and ${mcpLines().length} MCP lines`,
);
