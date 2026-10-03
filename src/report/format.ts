import * as vscode from 'vscode';
import type { DocumentReport, Finding } from '../types';
import { escapeNonAscii } from '../utils/escape';

/**
 * The report a person reads, as Markdown.
 *
 * **Nothing in it is ever a character the scan found.** Codepoints are
 * `U+XXXX` and the detail is the crate's English. The two fields the scan did
 * not write — the path and the key path, both taken from the document — are
 * escaped to `\uXXXX`, as the crate escapes them on stderr, so a file named with
 * a right-to-left override cannot reorder the report that names it.
 */
export function formatReport(
	reports: readonly DocumentReport[],
	filesScanned: number,
): string {
	const findings = reports.reduce(
		(sum, report) => sum + report.findings.length,
		0,
	);
	const refused = reports.filter((report) => report.refusals.length > 0).length;
	const lines: string[] = [];

	lines.push(`# ${vscode.l10n.t('Unicode-LE report')}`, '');
	lines.push(
		vscode.l10n.t(
			'{0} finding(s) in {1} file(s) scanned; {2} file(s) not fully judged.',
			findings,
			filesScanned,
			refused,
		),
		'',
	);
	lines.push(
		`> ${vscode.l10n.t('Characters are written as U+XXXX and never as themselves: a report that pasted one would carry it into whatever renders this.')}`,
		'',
	);

	if (findings === 0 && refused === 0) {
		lines.push(
			vscode.l10n.t(
				'Nothing found. Silence is not a clearance: a confusable added after the tables were built is not flagged.',
			),
		);
		return `${lines.join('\n')}\n`;
	}

	for (const report of reports) {
		if (report.findings.length === 0 && report.refusals.length === 0) continue;
		lines.push(`## ${code(report.file)}`, '');
		for (const finding of report.findings) lines.push(...item(finding));
		for (const refusal of report.refusals) {
			lines.push(
				`- **${vscode.l10n.t('Not judged')}** (\`${refusal.reason}\`): ${escapeNonAscii(refusal.detail)}`,
				'',
			);
		}
	}
	return `${lines.join('\n')}\n`;
}

/**
 * One finding as a list item: where, what and how bad on the first line, the
 * detail as its own paragraph beneath it. A table was tried and read badly at any editor width — the
 * detail crushed every other column into a tall strip.
 */
function item(finding: Finding): string[] {
	const codepoints = finding.resembles
		? `${finding.codepoints.join(' ')} (${vscode.l10n.t('resembles {0}', finding.resembles.join(' '))})`
		: finding.codepoints.join(' ');
	const key =
		finding.key === undefined
			? ''
			: ` · ${vscode.l10n.t('Key')} ${code(finding.key)}`;
	return [
		`- **${finding.line}:${finding.column}** · ${finding.kind} · ${finding.severity} · ${codepoints}${key}`,
		'',
		`  ${escapeNonAscii(finding.detail)}`,
		'',
	];
}

/**
 * Text the scan did not write — a path or a key path — as a code span.
 * Backslashes are doubled before non-ASCII is escaped, JSON's way, so a file
 * actually named `\u202E` reads `\\u202E` and never like the character it is
 * not. A code span cannot escape a backtick, so one becomes a quote.
 */
function code(text: string): string {
	return `\`${escapeNonAscii(text.replace(/\\/g, '\\\\')).replace(/`/g, "'").replace(/\r?\n/g, ' ')}\``;
}
