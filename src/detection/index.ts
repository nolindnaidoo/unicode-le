import {
	type Draft,
	type Examination,
	type Finding,
	KINDS,
	type Kind,
	type Options,
	type Refusal,
} from '../types';
import * as characters from './characters';
import { type Format, resolveFormat } from './format';
import { escapeSpans, keyAt, keySpans } from './locate';
import * as normalize from './normalize';
import { Locator } from './position';
import * as scripts from './scripts';

export { decode } from './encoding';
export { resolveFormat, SUPPORTED_FORMATS } from './format';
export { parseScript } from './scripts';

/** The crate's `Kind` order, which breaks ties between findings at one offset. */
const KIND_ORDER: ReadonlyMap<Kind, number> = new Map(
	KINDS.map(([kind], order) => [kind, order]),
);

/**
 * Resolve a kind from its report name or its short filter word, refusing by
 * name anything else.
 */
export function parseKind(token: string): Kind {
	const found = KINDS.find(
		([name, short]) => name === token || short === token,
	);
	if (!found) {
		throw new Error(
			`${token} is not a kind; one of: ${KINDS.map(([, short]) => short).join(', ')}`,
		);
	}
	return found[0];
}

/**
 * Examine one document — the crate's `detect::examine`, step for step.
 *
 * `format` decides only how a finding is addressed, never which findings exist:
 * every scanner runs over the same raw text, so a document whose format cannot
 * be read loses its key paths and keeps every finding.
 */
export function examine(
	content: string,
	format: Format,
	options: Options,
): Examination {
	const wants = (kind: Kind): boolean =>
		options.kinds.length === 0 || options.kinds.includes(kind);
	const drafts: Draft[] = [
		...characters.scan(content),
		...normalize.scan(content),
	];

	// The script pair is asked for together or not at all: a caller who wants
	// neither has no use for a refusal about them.
	const refusals: Refusal[] = [];
	if (wants('confusable') || wants('mixed-script')) {
		const refusal = scripts.context(content, options.expectedScripts);
		if (refusal) refusals.push(refusal);
		else
			drafts.push(
				...scripts.scan(
					content,
					options.expectedScripts,
					escapeSpans(content, format),
				),
			);
	}

	const kept = drafts
		.filter((draft) => wants(draft.kind))
		// Document order, and a total order within an offset, so two runs over
		// an unchanged document produce an identical report.
		.sort(
			(a, b) =>
				a.index - b.index ||
				(KIND_ORDER.get(a.kind) as number) - (KIND_ORDER.get(b.kind) as number),
		);

	const locator = new Locator(content);
	const spans = keySpans(content, format);
	const findings = kept.map((draft): Finding => {
		const { line, column, offset } = locator.at(draft.index);
		// An empty path is the document's root, which names nothing.
		const key = keyAt(spans, draft.index);
		return {
			kind: draft.kind,
			severity: draft.severity,
			line,
			column,
			offset,
			...(key !== undefined && key.length > 0 ? { key } : {}),
			codepoints: draft.codepoints,
			...(draft.scripts.length > 0 ? { scripts: draft.scripts } : {}),
			...(draft.resembles.length > 0 ? { resembles: draft.resembles } : {}),
			detail: draft.detail,
		};
	});
	return { findings, refusals };
}

/** The format a named document is read as — the CLI's resolution. */
export function formatOf(filename: string): Format {
	return resolveFormat(undefined, filename);
}
