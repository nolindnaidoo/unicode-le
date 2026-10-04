import * as vscode from 'vscode';
import type { Telemetry } from '../telemetry/telemetry';
import type { Notifier } from '../ui/notifier';
import type { RatingPrompt } from '../ui/ratingPrompt';
import type { StatusBar } from '../ui/statusBar';
import { detectInActiveDocument } from './detect';
import { scanFolder, scanWorkspace } from './scanWorkspace';

export interface CommandDependencies {
	notifier: Notifier;
	ratingPrompt: RatingPrompt;
	statusBar: StatusBar;
	telemetry: Telemetry;
}

export function registerCommands(
	context: vscode.ExtensionContext,
	deps: CommandDependencies,
): void {
	const diagnostics = vscode.languages.createDiagnosticCollection('unicode-le');
	const commands = [
		diagnostics,
		vscode.commands.registerCommand(
			'unicode-le.detect',
			async () => await detectInActiveDocument(deps),
		),
		vscode.commands.registerCommand(
			'unicode-le.scanWorkspace',
			async () => await scanWorkspace(deps, diagnostics),
		),
		// The Explorer hands over the folder that was clicked. From the
		// palette there is none, and the command asks.
		vscode.commands.registerCommand(
			'unicode-le.scanFolder',
			async (picked?: vscode.Uri) =>
				await scanFolder(deps, diagnostics, picked),
		),
	];
	for (const command of commands) context.subscriptions.push(command);
}
