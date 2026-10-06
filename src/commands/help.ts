import * as vscode from 'vscode';
import type { Telemetry } from '../telemetry/telemetry';

/**
 * Register the help command
 */
export function registerHelpCommand(
	context: vscode.ExtensionContext,
	telemetry: Telemetry,
): void {
	const disposable = vscode.commands.registerCommand(
		'unicode-le.help',
		async () => {
			telemetry.event('command', { name: 'help' });
			await showHelp();
		},
	);
	context.subscriptions.push(disposable);
}

async function showHelp(): Promise<void> {
	const doc = await vscode.workspace.openTextDocument({
		content: generateHelpContent(),
		language: 'markdown',
	});
	await vscode.window.showTextDocument(doc, {
		preview: false,
		viewColumn: vscode.ViewColumn.Beside,
	});
}

/** Every claim here is the crate's behaviour, and the corpus pins it. */
export function generateHelpContent(): string {
	return [
		'# Unicode-LE Help',
		'',
		'Finds the Unicode characters that hide meaning, and reports each one as `U+XXXX` — never as the character itself.',
		'',
		'## Commands',
		'',
		'- **Detect Unicode Risks**: the active document, as the editor holds it.',
		'- **Scan Workspace for Unicode Risks**: every file in the workspace, read from disk as UTF-8. Dependency folders, files `.gitignore` leaves out and binary files are skipped unless the `unicode-le.workspace.*` settings say otherwise.',
		'- **Scan Folder for Unicode Risks**: the same for one folder. Also on a folder in the Explorer.',
		'- **Open Settings** and **Help & Troubleshooting**.',
		'',
		'## What is found',
		'',
		'| Kind | Severity | What |',
		'|---|---|---|',
		'| `bidi-control` | high | The Trojan Source class (CVE-2021-42574): reorders how the rest of the line renders. |',
		'| `confusable` | high | A homoglyph in a word of another script, or a compatibility form of an ASCII character. |',
		'| `mixed-script` | high | One word no single script accounts for. |',
		'| `invisible` | medium | Zero-width characters, the soft hyphen, and a byte-order mark anywhere but the start. |',
		'| `unassigned-or-private-use` | medium | A codepoint with no agreed meaning. |',
		'| `non-nfc` | low | A line that is not in Normalization Form C. Reported, never rewritten. |',
		'| `unusual-whitespace` | low | A space that is not U+0020. |',
		'',
		'## Translated files',
		'',
		'A file where at least 10% of the letters belong to a script you have not declared is **not judged** for homoglyphs and mixed scripts, and the report says so. Every other check still runs. Name the scripts your workspace is written in with `unicode-le.detection.scripts` — for example `Han`, `Cyrillic` or `Hira` — and the checks run on those files too. Declaring a script only ever narrows the findings; it never switches a check off.',
		'',
		'## Key paths',
		'',
		'In JSON, YAML, TOML, INI, `.properties`, `.env`, CSV and TSV a finding also carries the key path it sits under, such as `metrics.headline.eyebrow`. The format decides only how a finding is addressed, never whether it exists.',
		'',
		'## Refusals',
		'',
		'The workspace scan reads UTF-8 only. A UTF-16 or UTF-32 file, a binary file and a file that is not valid UTF-8 are refused by name rather than decoded as something else, because a wrong decode invents findings.',
		'',
		'## Agents',
		'',
		'The bundled MCP server offers `detect_unicode_risks` to agent mode. It answers exactly as the `unicode-le` command-line tool does.',
		'',
		'## Troubleshooting',
		'',
		'- **Nothing found** is not a clearance: a confusable pair newer than the tables is not flagged.',
		'- **A setting names an unknown script**: the command says which, and runs nothing until it is fixed.',
		'- **Large workspaces**: lower `unicode-le.workspace.scanMaxFiles`, or narrow `unicode-le.workspace.scanPatterns`.',
		'',
	].join('\n');
}
