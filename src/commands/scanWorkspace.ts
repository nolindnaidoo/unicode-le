import * as vscode from 'vscode';
import { readConfig } from '../config/config';
import { decode, examine, resolveFormat } from '../detection';
import { formatReport } from '../report/format';
import type { DocumentReport, Finding, Options } from '../types';
import { errorMessage } from '../utils/errors';
import {
	listFiles,
	type ScanLimits,
	type ScanSummary,
	unreadNotes,
} from '../workspace/scan';
import type { CommandDependencies } from './index';
import { optionsFrom } from './options';
import { showReport } from './output';

/**
 * Scan every file under a folder from disk, or in the whole workspace when no
 * folder is given.
 *
 * Read as bytes and decoded here, not opened as documents: an editor will
 * happily open a UTF-16 file as something, and the whole point of the encoding
 * refusal is that a mis-decode invents findings. So a file is UTF-8, or it is
 * refused by name — never guessed at.
 *
 * Which files are read is the shared listing's decision. Reading them is not
 * the shared reader's: that one counts a file that is not UTF-8 and moves
 * on, and here such a file is named with the reason.
 */
export async function scanWorkspace(
	deps: CommandDependencies,
	diagnostics: vscode.DiagnosticCollection,
	root?: vscode.Uri,
): Promise<void> {
	deps.telemetry.event('command', {
		name: root === undefined ? 'scanWorkspace' : 'scanFolder',
	});
	if (
		root === undefined &&
		(vscode.workspace.workspaceFolders ?? []).length === 0
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
	const limits: ScanLimits = {
		patterns: config.workspaceScanPatterns,
		excludes: config.workspaceScanExcludes,
		useDefaultExcludes: config.workspaceScanUseDefaultExcludes,
		skipBinaryFiles: config.workspaceScanSkipBinaryFiles,
		alwaysInclude: config.workspaceScanAlwaysInclude,
		maxFiles: config.workspaceScanMaxFiles,
		maxFileBytes: config.safetyEnabled
			? config.safetyFileSizeWarnBytes
			: undefined,
		respectGitignore: config.workspaceScanRespectGitignore,
	};

	await vscode.window.withProgress(
		{
			location: vscode.ProgressLocation.Notification,
			title: vscode.l10n.t('Scanning files...'),
			cancellable: true,
		},
		async (progress, token) => {
			const { files, fileLimitReached, ignored } = await listFiles(
				root,
				limits,
			);
			const reports: Scanned[] = [];
			let read = 0;
			let tooLarge = 0;
			let findings = 0;
			let stoppedEarly = false;
			for (const [index, uri] of files.entries()) {
				// A cancelled scan read part of the tree. Reporting that as
				// the project's risks would understate it without saying so.
				if (token.isCancellationRequested) return;
				if (index % 50 === 0) {
					progress.report({
						message: vscode.l10n.t('{0} of {1} files', index, files.length),
					});
				}
				const result = await scanFile(
					uri,
					labelOf(root, uri),
					options,
					limits.maxFileBytes,
				);
				if (result === undefined) {
					tooLarge++;
					continue;
				}
				read++;
				// The limit is on what the report lists, so the file that
				// crosses it is cut and the scan stops there.
				const kept = result.findings.slice(
					0,
					config.workspaceScanMaxResults - findings,
				);
				findings += kept.length;
				if (kept.length > 0 || result.refusals.length > 0)
					reports.push({ ...result, findings: kept, uri });
				if (findings >= config.workspaceScanMaxResults) {
					stoppedEarly =
						kept.length < result.findings.length || index < files.length - 1;
					break;
				}
			}
			const summary: ScanSummary = {
				read,
				tooLarge,
				// Never counted here: a file that is not UTF-8 is refused by name.
				notText: 0,
				fileLimitReached,
				ignored,
				stoppedEarly,
				cancelled: false,
			};

			// Each scan replaces the last one's problems, and a scan that
			// publishes none still clears them.
			publish(diagnostics, config.workspaceScanProblemsEnabled ? reports : []);
			const notes = unreadNotes(summary, limits, '`unicode-le.workspace.*`');
			const report = (positions: boolean): string => {
				const body = formatReport(reports, read, positions);
				const gap = body.endsWith('\n\n') ? '' : '\n';
				return `${body}${gap}${notes.map((note) => `> ${note}\n`).join('\n')}`;
			};
			await showReport(
				report(config.showPositions),
				config,
				deps,
				report(config.clipboardIncludesPositions),
			);

			deps.telemetry.event('workspace-scanned', {
				files: String(read),
				findings: String(findings),
				skipped: String(tooLarge),
			});
			deps.statusBar.flash(vscode.l10n.t('{0} finding(s)', findings));
			if (findings > 0) {
				deps.notifier.warn(
					vscode.l10n.t(
						'Found {0} Unicode risk(s) in {1} file(s)',
						findings,
						reports.filter((entry) => entry.findings.length > 0).length,
					),
				);
			}
		},
	);
}

/** Scan the folder picked in the Explorer, or ask for one. */
export async function scanFolder(
	deps: CommandDependencies,
	diagnostics: vscode.DiagnosticCollection,
	picked?: vscode.Uri,
): Promise<void> {
	const folder = picked ?? (await askForFolder());
	if (folder === undefined) return;
	await scanWorkspace(deps, diagnostics, folder);
}

async function askForFolder(): Promise<vscode.Uri | undefined> {
	const start = vscode.workspace.workspaceFolders?.[0]?.uri;
	const chosen = await vscode.window.showOpenDialog({
		canSelectFiles: false,
		canSelectFolders: true,
		canSelectMany: false,
		...(start === undefined ? {} : { defaultUri: start }),
		openLabel: vscode.l10n.t('Scan Folder'),
	});
	return chosen?.[0];
}

type Scanned = DocumentReport & { readonly uri: vscode.Uri };

/**
 * A file as the report names it: relative to the folder that was scanned, or
 * to the workspace when the whole of it was.
 *
 * The prefix is compared without regard to case. Windows hands the same drive
 * over as `/C:/` from one call and `/c:/` from another.
 */
function labelOf(root: vscode.Uri | undefined, uri: vscode.Uri): string {
	if (root === undefined) return vscode.workspace.asRelativePath(uri, false);
	const base = root.path.replace(/\/+$/, '');
	return uri.path.toLowerCase().startsWith(`${base.toLowerCase()}/`)
		? uri.path.slice(base.length + 1)
		: uri.path;
}

/**
 * The findings, in the Problems panel.
 *
 * Only when asked for: a scan of a project that writes in more than one
 * script can find thousands, and a panel full of them hides whatever else is
 * there.
 */
function publish(
	diagnostics: vscode.DiagnosticCollection,
	reports: readonly Scanned[],
): void {
	diagnostics.clear();
	for (const { uri, findings } of reports) {
		if (findings.length === 0) continue;
		diagnostics.set(uri, findings.map(problem));
	}
}

function problem(finding: Finding): vscode.Diagnostic {
	const start = new vscode.Position(finding.line - 1, finding.column - 1);
	// The message is the codepoints and the scan's own English, never the
	// character: the panel renders what it is given.
	const diagnostic = new vscode.Diagnostic(
		new vscode.Range(start, start.translate(0, 1)),
		`${finding.kind} (${finding.codepoints.join(' ')}): ${finding.detail}`,
		vscode.DiagnosticSeverity.Warning,
	);
	diagnostic.source = 'unicode-le';
	return diagnostic;
}

/** One file's result, or undefined when it is over the size limit. */
async function scanFile(
	uri: vscode.Uri,
	file: string,
	options: Options,
	sizeLimit: number | undefined,
): Promise<DocumentReport | undefined> {
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
