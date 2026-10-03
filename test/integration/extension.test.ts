import * as assert from 'node:assert';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import * as vscode from 'vscode';

const EXTENSION_ID = 'nolindnaidoo.unicode-le';

async function openFile(name: string, content: string): Promise<vscode.TextEditor> {
	const dir = mkdtempSync(join(tmpdir(), 'unicode-le-it-'));
	const filePath = join(dir, name);
	writeFileSync(filePath, content, 'utf8');
	const document = await vscode.workspace.openTextDocument(vscode.Uri.file(filePath));
	return vscode.window.showTextDocument(document);
}

describe('Unicode-LE integration', function () {
	this.timeout(30_000);

	it('activates', async () => {
		const extension = vscode.extensions.getExtension(EXTENSION_ID);
		assert.ok(extension, `extension ${EXTENSION_ID} not found`);
		await extension.activate();
		assert.strictEqual(extension.isActive, true);
	});

	it('registers every declared command', async () => {
		const extension = vscode.extensions.getExtension(EXTENSION_ID);
		await extension?.activate();
		const commands = await vscode.commands.getCommands(true);
		for (const id of ['unicode-le.detect', 'unicode-le.scanWorkspace', 'unicode-le.openSettings', 'unicode-le.help']) {
			assert.ok(commands.includes(id), `missing command: ${id}`);
		}
	});

	it('reports a bidi control in the active document, as a codepoint and never as itself', async () => {
		await openFile('login.ts', 'const isAdmin = false; /* ‮ } if (isAdmin) ⁦ */\n');

		await vscode.commands.executeCommand('unicode-le.detect');

		const report = vscode.workspace.textDocuments.find(
			(doc) => doc.languageId === 'markdown' && doc.getText().includes('U+202E'),
		);
		assert.ok(report, 'no report document found');
		assert.ok(report.getText().includes('bidi-control'));
		assert.ok(!report.getText().includes('‮'), 'the report carried the character it found');
	});

	it('names the key path a finding sits under in JSON', async () => {
		await openFile('messages.json', '{\n  "auth": { "provider": "pаypal and other words in english" }\n}\n');

		await vscode.commands.executeCommand('unicode-le.detect');

		const report = vscode.workspace.textDocuments.find(
			(doc) => doc.languageId === 'markdown' && doc.getText().includes('auth.provider'),
		);
		assert.ok(report, 'no report naming the key path was found');
	});

	it('offers its MCP server to agent mode', async () => {
		// The registration itself is only observable in a real host, which
		// scripts/e2e-vsix.js covers against the installed VSIX.
		const extension = vscode.extensions.getExtension(EXTENSION_ID);
		await extension?.activate();
		assert.strictEqual(
			typeof vscode.lm.registerMcpServerDefinitionProvider,
			'function',
			'this VS Code build predates the MCP provider API',
		);
		const providers = extension?.packageJSON.contributes.mcpServerDefinitionProviders as {
			id: string;
			label: string;
		}[];
		assert.deepStrictEqual(
			providers.map((p) => p.id),
			['unicode-le'],
		);
	});
});
