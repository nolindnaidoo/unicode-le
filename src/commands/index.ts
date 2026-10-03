import * as vscode from 'vscode';
import type { Telemetry } from '../telemetry/telemetry';
import type { Notifier } from '../ui/notifier';
import type { StatusBar } from '../ui/statusBar';
import { detectInActiveDocument } from './detect';
import { scanWorkspace } from './scanWorkspace';

export interface CommandDependencies {
	notifier: Notifier;
	statusBar: StatusBar;
	telemetry: Telemetry;
}

export function registerCommands(
	context: vscode.ExtensionContext,
	deps: CommandDependencies,
): void {
	const commands = [
		vscode.commands.registerCommand(
			'unicode-le.detect',
			async () => await detectInActiveDocument(deps),
		),
		vscode.commands.registerCommand(
			'unicode-le.scanWorkspace',
			async () => await scanWorkspace(deps),
		),
	];
	for (const command of commands) context.subscriptions.push(command);
}
