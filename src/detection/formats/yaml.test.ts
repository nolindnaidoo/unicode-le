import { describe, expect, it } from 'vitest';
import { keySpans } from './yaml';

describe('yaml key paths', () => {
	it('names a sequence item by the column its key starts at', () => {
		const text = 'items:\n  - id: A\n    name: B\n';
		expect(keySpans(text).map((span) => span.path)).toEqual([
			'items.[0].id',
			'items.[0].name',
		]);
	});

	/**
	 * The crate compares one line's indent with another's in UTF-8 bytes. Two
	 * ogham space marks are two code units and six bytes, so the line nests
	 * under `id` there; measuring in code units nested it under `a` and the
	 * differential caught the two servers naming the same finding differently.
	 */
	it('compares indents in UTF-8 bytes, as the crate does', () => {
		const text = 'a:\n  - id: A\n\u1680\u1680b: x\n';
		expect(keySpans(text).map((span) => span.path)).toEqual([
			'a.[0].id',
			'a.[0].id.b',
		]);
	});
});
