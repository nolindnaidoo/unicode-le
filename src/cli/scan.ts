/**
 * One file end to end and the run's report, as the Rust CLI's `scan.rs` builds
 * them. A finding carries codepoints, never the characters, and every string
 * that leaves for a terminal or a pipe has its non-ASCII escaped first.
 */
import { readFileSync } from 'node:fs';
import { sep } from 'node:path';
import { decode, examine, resolveFormat } from '../detection';
import type { Finding, Options, Refusal } from '../types';
import { escapeNonAscii } from '../utils/escape';
import { rustIoMessage } from './ioError';

export interface ScanOptions extends Options {
	/** A format for every file, from the MCP scan tool; the name otherwise. */
	readonly format?: string;
}

export interface FileReport {
	readonly file: string;
	readonly findings: readonly Finding[];
	readonly refusals: readonly Refusal[];
	readonly summary: {
		readonly findings: number;
		readonly bidi: number;
		readonly refusals: number;
	};
}

export interface Report {
	readonly schema: 1;
	readonly files: readonly FileReport[];
	readonly summary: {
		readonly files: number;
		readonly findings: number;
		readonly bidi: number;
		readonly refusals: number;
		readonly unexamined: number;
	};
}

export function report(files: readonly FileReport[]): Report {
	const sum = (pick: (file: FileReport) => number) =>
		files.reduce((total, file) => total + pick(file), 0);
	return Object.freeze({
		schema: 1,
		files,
		summary: Object.freeze({
			files: files.length,
			findings: sum((file) => file.summary.findings),
			bidi: sum((file) => file.summary.bidi),
			refusals: sum((file) => file.summary.refusals),
			unexamined: files.filter(wasUnexamined).length,
		}),
	});
}

/** A script-context refusal still judged the rest; any other means unread. */
function wasUnexamined(file: FileReport): boolean {
	return file.refusals.some(
		(refusal) => refusal.reason !== 'intentional_script_context',
	);
}

export function scanFile(path: string, options: ScanOptions): FileReport {
	const file = sep === '/' ? path : path.split(sep).join('/');
	let bytes: Buffer;
	try {
		bytes = readFileSync(path);
	} catch (error) {
		return refused(file, {
			reason: 'binary_or_undecodable',
			detail: `could not be read: ${rustIoMessage(error)}`,
		});
	}
	return scanBytes(bytes, file, options);
}

export function scanBytes(
	bytes: Uint8Array,
	file: string,
	options: ScanOptions,
): FileReport {
	const decoded = decode(bytes);
	if (decoded.refusal) return refused(file, decoded.refusal);
	const format = resolveFormat(options.format, file);
	const examination = examine(decoded.text, format, options);
	const findings = examination.findings.map(crateOrder);
	return Object.freeze({
		file,
		findings,
		refusals: examination.refusals,
		summary: Object.freeze({
			findings: findings.length,
			bidi: findings.filter((finding) => finding.kind === 'bidi-control')
				.length,
			refusals: examination.refusals.length,
		}),
	});
}

/** The crate's field order, with the empty lists it skips left out. */
function crateOrder(finding: Finding): Finding {
	return Object.freeze({
		kind: finding.kind,
		severity: finding.severity,
		line: finding.line,
		column: finding.column,
		offset: finding.offset,
		...(finding.key === undefined ? {} : { key: finding.key }),
		codepoints: finding.codepoints,
		...(finding.scripts && finding.scripts.length > 0
			? { scripts: finding.scripts }
			: {}),
		...(finding.resembles && finding.resembles.length > 0
			? { resembles: finding.resembles }
			: {}),
		detail: finding.detail,
	});
}

function refused(file: string, refusal: Refusal): FileReport {
	return Object.freeze({
		file,
		findings: [],
		refusals: [refusal],
		summary: Object.freeze({ findings: 0, bidi: 0, refusals: 1 }),
	});
}

/** 2 under `--strict` with a refusal, 1 on what the gate counts, else 0. */
export function exitCode(
	run: Report,
	failOn: 'any' | 'bidi',
	strict: boolean,
): number {
	if (strict && run.summary.refusals > 0) return 2;
	const counted = failOn === 'any' ? run.summary.findings : run.summary.bidi;
	return counted > 0 ? 1 : 0;
}

export function describe(file: FileReport, finding: Finding): string {
	const resembles =
		finding.resembles && finding.resembles.length > 0
			? ` (resembles ${finding.resembles.join(' ')})`
			: '';
	const key =
		finding.key === undefined ? '' : `  at ${escapeNonAscii(finding.key)}`;
	return `${escapeNonAscii(file.file)}:${finding.line}:${finding.column}  [${finding.severity}] ${finding.kind} ${finding.codepoints.join(' ')}${resembles}${key}  ${finding.detail}`;
}

export function describeRefusal(file: string, refusal: Refusal): string {
	return `${escapeNonAscii(file)}: ${escapeNonAscii(refusal.detail)}`;
}
