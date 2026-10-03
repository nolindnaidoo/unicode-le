import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import type { Finding, Reason } from '../types';
import { decode, examine, formatOf, parseScript } from './index';

/**
 * The shared corpus, run against this side. `cargo test` runs the crate over
 * the same files, so a difference between the two frontends fails one of them.
 */
const CORPUS = join(__dirname, '..', '..', 'crate', 'fixtures');

interface DocumentCase {
	readonly name: string;
	readonly file: string;
	readonly scripts?: readonly string[];
	readonly expected: readonly Finding[];
	readonly refusals?: readonly Reason[];
}

const corpus = JSON.parse(
	readFileSync(join(CORPUS, 'detection.json'), 'utf8'),
) as {
	readonly documents: readonly DocumentCase[];
	readonly encodings: readonly {
		readonly name: string;
		readonly file: string;
		readonly reason: Reason;
	}[];
};

function document(file: string): string {
	return readFileSync(join(CORPUS, 'documents', file), 'utf8');
}

describe('the shared corpus', () => {
	for (const testCase of corpus.documents) {
		it(testCase.name, () => {
			const examination = examine(
				document(testCase.file),
				formatOf(testCase.file),
				{
					kinds: [],
					expectedScripts: (testCase.scripts ?? []).map(parseScript),
				},
			);
			expect(examination.findings).toEqual(testCase.expected);
			expect(examination.refusals.map((refusal) => refusal.reason)).toEqual(
				testCase.refusals ?? [],
			);
		});
	}

	for (const testCase of corpus.encodings) {
		it(testCase.name, () => {
			const decoded = decode(
				new Uint8Array(readFileSync(join(CORPUS, 'documents', testCase.file))),
			);
			expect(decoded.refusal?.reason).toBe(testCase.reason);
		});
	}
});
