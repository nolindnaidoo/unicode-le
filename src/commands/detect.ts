import * as vscode from 'vscode';
import { readConfig } from '../config/config';
import { examine, resolveFormat } from '../detection';
import { formatReport } from '../report/format';
import { errorMessage } from '../utils/errors';
import type { CommandDependencies } from './index';
import { optionsFrom } from './options';
import { showReport } from './output';

/**
 * Detect Unicode risks in the active document.
 *
 * The document is read as the editor holds it, so a line and column in the
 * report are the editor's own. The format comes from the language mode first
 * and the file name second; it decides only whether a finding can carry a key
 * path, never whether it exists.
 */
export async function detectInActiveDocument(
	deps: CommandDependencies,
): Promise<void> {
	deps.telemetry.event('command', { name: 'detect' });
	const editor = vscode.window.activeTextEditor;
	if (!editor) {
		deps.notifier.error(vscode.l10n.t('No active editor'));
		return;
	}
	const document = editor.document;
	const text = document.getText();
	if (text.length === 0) {
		deps.notifier.info(vscode.l10n.t('File is empty'));
		return;
	}

	const config = readConfig();
	if (config.safetyEnabled && !document.isUntitled) {
		try {
			const { size } = await vscode.workspace.fs.stat(document.uri);
			if (size > config.safetyFileSizeWarnBytes) {
				deps.notifier.warn(
					vscode.l10n.t(
						'Large file detected ({0} bytes). Detection may take longer.',
						size,
					),
				);
			}
		} catch {
			// A document with no file behind it has no size to warn about.
		}
	}

	let options: ReturnType<typeof optionsFrom>;
	try {
		options = optionsFrom(config);
	} catch (error) {
		deps.notifier.error(
			vscode.l10n.t('Unicode-LE settings: {0}', errorMessage(error)),
		);
		return;
	}

	const examination = examine(
		text,
		resolveFormat(document.languageId, document.fileName),
		options,
	);
	const file = document.isUntitled
		? document.fileName
		: vscode.workspace.asRelativePath(document.uri, false);
	const reports = [
		{ file, findings: examination.findings, refusals: examination.refusals },
	];
	await showReport(
		formatReport(reports, 1, config.showPositions),
		config,
		deps,
		formatReport(reports, 1, config.clipboardIncludesPositions),
	);

	deps.telemetry.event('detected', {
		findings: String(examination.findings.length),
		refusals: String(examination.refusals.length),
	});
	deps.statusBar.flash(
		vscode.l10n.t('{0} finding(s)', examination.findings.length),
	);
	if (examination.findings.length > 0) {
		deps.notifier.warn(
			vscode.l10n.t(
				'Found {0} Unicode risk(s) in this document',
				examination.findings.length,
			),
		);
	}
}
