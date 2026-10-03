/**
 * Fails when the extension's detection drifts from the shared corpus, which
 * the Rust CLI (crate/) also builds against.
 *
 * - fixtures/detection.json must reproduce under the extension's own
 *   `examine` — every finding, field for field, and every refusal.
 * - fixtures/mcp-detect-unicode.json must reproduce under the npm server's
 *   `detect_unicode_risks`, the tool both servers offer.
 * - fixtures/unicode-tables.json must be the tables this extension loads. The
 *   crate's own test holds the file equal to the crate; this holds the
 *   extension equal to the file, so the two frontends cannot read different
 *   Unicode versions.
 * - Nothing either side emits for the corpus may carry a non-ASCII
 *   character once escaped for the wire. That is the property this tool
 *   rests on, and it is checked rather than assumed.
 *
 * This checks only the extension's side. `cargo test` runs the crate's
 * implementation over the same files.
 *
 * Run: bun scripts/check-detection-parity.ts
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { decode, examine, formatOf, parseScript } from '../src/detection';
import { allScripts, UNICODE_VERSION } from '../src/detection/tables';
import { TOOLS } from '../src/mcp/tools';
import { escapeNonAscii } from '../src/utils/escape';
import type { Finding, Reason } from '../src/types';

const ROOT = join(import.meta.dir, '..');
/** The corpus lives inside the crate so the published package is self-contained. */
const CORPUS = join(ROOT, 'crate', 'fixtures');
const failures: string[] = [];

function fail(message: string): void {
	failures.push(message);
}

function canonical(value: unknown): string {
	if (value === null || typeof value !== 'object') return JSON.stringify(value);
	if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
	const entries = Object.entries(value as Record<string, unknown>)
		.filter(([, item]) => item !== undefined)
		.sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
	return `{${entries.map(([k, v]) => `${JSON.stringify(k)}:${canonical(v)}`).join(',')}}`;
}

function readCorpus(name: string): unknown {
	return JSON.parse(readFileSync(join(CORPUS, name), 'utf8'));
}

function readDocument(file: string): string {
	return readFileSync(join(CORPUS, 'documents', file), 'utf8');
}

function isAscii(text: string): boolean {
	return /^[\x00-\x7f]*$/.test(text);
}

function checkDetection(): void {
	const corpus = readCorpus('detection.json') as {
		documents: { name: string; file: string; scripts?: string[]; expected: Finding[]; refusals?: Reason[] }[];
		encodings: { name: string; file: string; reason: Reason }[];
	};
	if (corpus.documents.length === 0) fail('detection.json holds no documents');

	for (const testCase of corpus.documents) {
		const examination = examine(readDocument(testCase.file), formatOf(testCase.file), {
			kinds: [],
			expectedScripts: (testCase.scripts ?? []).map(parseScript),
		});
		if (canonical(examination.findings) !== canonical(testCase.expected)) {
			fail(
				`detection "${testCase.name}":\n  expected: ${escapeNonAscii(canonical(testCase.expected))}\n  got:      ${escapeNonAscii(canonical(examination.findings))}`,
			);
		}
		const reasons = examination.refusals.map((refusal) => refusal.reason);
		if (canonical(reasons) !== canonical(testCase.refusals ?? [])) {
			fail(`detection "${testCase.name}": refusals ${canonical(reasons)}, expected ${canonical(testCase.refusals ?? [])}`);
		}
		const emitted = escapeNonAscii(JSON.stringify(examination));
		if (!isAscii(emitted)) fail(`detection "${testCase.name}": the escaped answer is not ASCII`);
	}

	for (const testCase of corpus.encodings) {
		const decoded = decode(new Uint8Array(readFileSync(join(CORPUS, 'documents', testCase.file))));
		if (decoded.refusal?.reason !== testCase.reason) {
			fail(`encoding "${testCase.name}": expected ${testCase.reason}, got ${decoded.refusal?.reason ?? 'text'}`);
		}
	}
}

/**
 * `detect_unicode_risks` is offered by BOTH MCP servers. They are meant to be
 * the same tool, not two similar ones, so the same corpus runs against both:
 * this function here, and `crate/src/mcp/detect.rs`'s own test there.
 */
async function checkMcp(): Promise<void> {
	const cases = readCorpus('mcp-detect-unicode.json') as {
		name: string;
		file?: string;
		content?: string;
		arguments: Record<string, unknown>;
		expected?: unknown;
		expectedError?: string;
	}[];
	const tool = TOOLS.find((t) => t.name === 'detect_unicode_risks');
	if (!tool) {
		fail('the extension no longer offers detect_unicode_risks');
		return;
	}
	for (const testCase of cases) {
		const args: Record<string, unknown> = { ...testCase.arguments };
		if (testCase.file !== undefined) args.content = readDocument(testCase.file);
		else if (testCase.content !== undefined) args.content = testCase.content;

		if (testCase.expectedError !== undefined) {
			try {
				await tool.handler(args);
				fail(`mcp "${testCase.name}": expected it to fail with ${JSON.stringify(testCase.expectedError)}`);
			} catch (error) {
				const message = error instanceof Error ? error.message : String(error);
				if (message !== testCase.expectedError) {
					fail(`mcp "${testCase.name}": expected error ${JSON.stringify(testCase.expectedError)}, got ${JSON.stringify(message)}`);
				}
			}
			continue;
		}
		const actual = JSON.parse(JSON.stringify(await tool.handler(args)));
		if (canonical(actual) !== canonical(testCase.expected)) {
			fail(
				`mcp "${testCase.name}":\n  expected: ${escapeNonAscii(canonical(testCase.expected))}\n  got:      ${escapeNonAscii(canonical(actual))}`,
			);
		}
	}
}

function checkTables(): void {
	const file = readCorpus('unicode-tables.json') as { unicode: { script: string }; scripts: [string, string][] };
	if (file.unicode.script !== UNICODE_VERSION) {
		fail(`the extension reads Unicode ${UNICODE_VERSION} and the shared tables are ${file.unicode.script}`);
	}
	if (canonical(file.scripts) !== canonical(allScripts())) {
		fail('the extension and the shared tables list different scripts');
	}
}

checkDetection();
await checkMcp();
checkTables();

if (failures.length > 0) {
	console.error(`Detection parity FAILED (${failures.length}):\n`);
	for (const failure of failures) console.error(`- ${failure}\n`);
	process.exit(1);
}
console.log(
	'OK: every corpus case reproduces, both MCP servers agree on the pinned cases, the extension reads the shared Unicode tables, and nothing escaped for the wire is anything but ASCII.',
);
