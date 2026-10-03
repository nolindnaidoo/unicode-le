import * as vscode from 'vscode';
import { readConfig } from '../config/config';
import { decode, examine, resolveFormat } from '../detection';
import { formatReport } from '../report/format';
import type { DocumentReport, Options } from '../types';
import { errorMessage } from '../utils/errors';
import type { CommandDependencies } from './index';
import { optionsFrom } from './options';
import { showReport } from './output';

/**
 * Scan the workspace's files from disk.
 *
 * Read as bytes and decoded here, not opened as documents: an editor will
 * happily open a UTF-16 file as something, and the whole point of the encoding
 * refusal is that a mis-decode invents findings. So a file is UTF-8, or it is
 * refused by name — never guessed at.
 */
export async function scanWorkspace(deps: CommandDependencies): Promise<void> {
	deps.telemetry.event('command', { name: 'scanWorkspace' });
	if (
		!vscode.workspace.workspaceFolders ||
		vscode.workspace.workspaceFolders.length === 0
	) {
		deps.notifier.warn(
			vscode.l10n.t('No workspace open. Please open a workspace folder first.'),
		);
		return;
	}
	const config = readConfig();
	let options: Options;
	try {
		options = optionsFrom(config);
	} catch (error) {
		deps.notifier.error(
			vscode.l10n.t('Unicode-LE settings: {0}', errorMessage(error)),
		);
		return;
	}

	await vscode.window.withProgress(
		{
			location: vscode.ProgressLocation.Notification,
			title: vscode.l10n.t('Scanning the workspace for Unicode risks...'),
			cancellable: true,
		},
		async (progress, token) => {
			const files = await findFiles(
				config.workspaceScanPatterns,
				config.workspaceScanExcludes,
				config.workspaceScanMaxFiles,
			);
			const reports: DocumentReport[] = [];
			let skipped = 0;
			for (const [index, uri] of files.entries()) {
				if (token.isCancellationRequested) return;
				if (index % 50 === 0) {
					progress.report({
						message: vscode.l10n.t('{0} of {1} files', index, files.length),
					});
				}
				const result = await scanFile(
					uri,
					options,
					config.safetyEnabled ? config.safetyFileSizeWarnBytes : undefined,
				);
				if (result === undefined) {
					skipped++;
					continue;
				}
				if (result.findings.length > 0 || result.refusals.length > 0)
					reports.push(result);
			}

			const report = formatReport(reports, files.length - skipped);
			const withSkips =
				skipped > 0
					? `${report}\n${vscode.l10n.t('{0} file(s) larger than the safety limit were not read.', skipped)}\n`
					: report;
			await showReport(withSkips, config, deps);

			const findings = reports.reduce(
				(sum, entry) => sum + entry.findings.length,
				0,
			);
			deps.telemetry.event('workspace-scanned', {
				files: String(files.length),
				findings: String(findings),
				skipped: String(skipped),
			});
			deps.statusBar.flash(vscode.l10n.t('{0} finding(s)', findings));
			if (findings > 0) {
				deps.notifier.warn(
					vscode.l10n.t(
						'Found {0} Unicode risk(s) in {1} file(s)',
						findings,
						reports.length,
					),
				);
			}
		},
	);
}

async function findFiles(
	patterns: readonly string[],
	excludes: readonly string[],
	maxFiles: number,
): Promise<vscode.Uri[]> {
	const exclude = excludes.length === 0 ? undefined : `{${excludes.join(',')}}`;
	const seen = new Set<string>();
	const out: vscode.Uri[] = [];
	for (const pattern of patterns) {
		for (const uri of await vscode.workspace.findFiles(
			pattern,
			exclude,
			maxFiles,
		)) {
			const key = uri.toString();
			if (seen.has(key)) continue;
			seen.add(key);
			out.push(uri);
		}
	}
	// findFiles promises no order, so two scans of one tree would list files
	// differently. A plain comparison rather than localeCompare: the order must
	// not change with the editor's display language.
	const byPath = (a: vscode.Uri, b: vscode.Uri) =>
		a.path < b.path ? -1 : Number(a.path > b.path);
	return out.sort(byPath).slice(0, maxFiles);
}

/** One file's result, or undefined when it is over the size limit. */
async function scanFile(
	uri: vscode.Uri,
	options: Options,
	sizeLimit: number | undefined,
): Promise<DocumentReport | undefined> {
	const file = vscode.workspace.asRelativePath(uri, false);
	try {
		if (
			sizeLimit !== undefined &&
			(await vscode.workspace.fs.stat(uri)).size > sizeLimit
		)
			return undefined;
		const decoded = decode(await vscode.workspace.fs.readFile(uri));
		if (decoded.refusal)
			return { file, findings: [], refusals: [decoded.refusal] };
		const examination = examine(
			decoded.text,
			resolveFormat(undefined, file),
			options,
		);
		return {
			file,
			findings: examination.findings,
			refusals: examination.refusals,
		};
	} catch (error) {
		// Under the same reason the crate uses for an I/O error, with the
		// system's message: a file that could not be read was not examined.
		return {
			file,
			findings: [],
			refusals: [
				{
					reason: 'binary_or_undecodable',
					detail: `could not be read: ${errorMessage(error)}`,
				},
			],
		};
	}
}
