/** The seven kinds, in the crate's `Kind` order — which is also sort order. */
export const KINDS = [
	['bidi-control', 'bidi'],
	['invisible', 'invisible'],
	['confusable', 'confusable'],
	['mixed-script', 'mixed-script'],
	['non-nfc', 'non-nfc'],
	['unusual-whitespace', 'whitespace'],
	['unassigned-or-private-use', 'unassigned'],
] as const;

export type Kind = (typeof KINDS)[number][0];
export type KindFilter = (typeof KINDS)[number][1];
export type Severity = 'high' | 'medium' | 'low';
export type Reason =
	| 'binary_or_undecodable'
	| 'encoding_unknown'
	| 'intentional_script_context';

/**
 * A finding before it knows where it is. `index` is a UTF-16 index into the
 * document; it becomes a line, a column and a byte offset in one place.
 */
export interface Draft {
	readonly index: number;
	readonly kind: Kind;
	readonly severity: Severity;
	readonly codepoints: readonly string[];
	readonly scripts: readonly string[];
	readonly resembles: readonly string[];
	readonly detail: string;
}

/**
 * One located finding, field for field the crate's `Finding`.
 *
 * `key`, `scripts` and `resembles` are absent rather than empty when there is
 * nothing to say, because a consumer branching on presence would read an empty
 * key as a key that is blank.
 */
export interface Finding {
	readonly kind: Kind;
	readonly severity: Severity;
	readonly line: number;
	readonly column: number;
	/** A byte offset into the UTF-8 document, as the crate reports it. */
	readonly offset: number;
	readonly key?: string;
	readonly codepoints: readonly string[];
	readonly scripts?: readonly string[];
	readonly resembles?: readonly string[];
	readonly detail: string;
}

/** Why a file, or part of one, was not judged. A result, not an error. */
export interface Refusal {
	readonly reason: Reason;
	readonly detail: string;
}

export interface Examination {
	readonly findings: readonly Finding[];
	readonly refusals: readonly Refusal[];
}

export interface Options {
	/** Empty means every kind — the only setting where a clean report means clean. */
	readonly kinds: readonly Kind[];
	/** Script indices the document is expected to contain. */
	readonly expectedScripts: readonly number[];
}

export type NotificationLevel = 'all' | 'important' | 'silent';

/** The extension's settings, read once per command and frozen. */
export interface Configuration {
	/** Whether the copy on the clipboard carries positions, whatever the screen shows. */
	readonly clipboardIncludesPositions: boolean;
	readonly copyToClipboardEnabled: boolean;
	/** Kind filter words; empty means every kind. */
	readonly detectionKinds: readonly KindFilter[];
	/** Script names or ISO 15924 tags the workspace is expected to contain. */
	readonly detectionScripts: readonly string[];
	readonly notificationsLevel: NotificationLevel;
	readonly openResultsSideBySide: boolean;
	readonly safetyEnabled: boolean;
	readonly safetyFileSizeWarnBytes: number;
	/** Whether the output gives the line and column of each finding. */
	readonly showPositions: boolean;
	readonly statusBarEnabled: boolean;
	readonly telemetryEnabled: boolean;
	readonly workspaceScanExcludes: readonly string[];
	readonly workspaceScanMaxFiles: number;
	readonly workspaceScanPatterns: readonly string[];
}

/** One document's result, as the report renders it. */
export interface DocumentReport {
	/** The path the reader knows the document by: relative, or the editor title. */
	readonly file: string;
	readonly findings: readonly Finding[];
	readonly refusals: readonly Refusal[];
}
