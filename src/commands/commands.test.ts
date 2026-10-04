import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { beforeEach, describe, expect, it } from 'vitest';
import {
	_clipboardText,
	_createDocument,
	_createExtensionContext,
	_diagnostics,
	_openedDocuments,
	_registeredCommands,
	_resetMockState,
	_respondToOpenDialog,
	_setActiveEditor,
	_setConfig,
	_setWorkspaceFiles,
	_shownMessages,
	executedBuiltins,
	Uri,
	workspace,
} from '../__mocks__/vscode';
import { registerOpenSettingsCommand } from '../config/settings';
import type { Telemetry } from '../telemetry/telemetry';
import { createNotifier } from '../ui/notifier';
import type { StatusBar } from '../ui/statusBar';
import { generateHelpContent, registerHelpCommand } from './help';
import { registerCommands } from './index';

function makeDeps() {
	const events: string[] = [];
	const flashes: string[] = [];
	const telemetry: Telemetry = {
		event: (name, properties) =>
			events.push(properties ? `${name}:${JSON.stringify(properties)}` : name),
		dispose: () => {},
	};
	const statusBar: StatusBar = { flash: (text) => flashes.push(text) };
	return {
		deps: {
			notifier: createNotifier(),
			statusBar,
			telemetry,
			ratingPrompt: { recordSuccess: async () => {} },
		},
		events,
		flashes,
	};
}

async function runCommand(id: string, ...args: unknown[]): Promise<void> {
	const handler = _registeredCommands().get(id);
	if (!handler) throw new Error(`command not registered: ${id}`);
	await handler(...args);
}

function report(): string {
	const last = _openedDocuments().at(-1);
	if (!last) throw new Error('no report was opened');
	return last.getText();
}

beforeEach(() => {
	_resetMockState();
	const { deps } = makeDeps();
	registerCommands(_createExtensionContext() as never, deps);
});

describe('unicode-le.detect', () => {
	it('errors when no editor is active', async () => {
		await runCommand('unicode-le.detect');
		expect(_shownMessages()[0]).toMatchObject({
			kind: 'error',
			message: 'No active editor',
		});
	});

	it('says an empty file is empty rather than clean', async () => {
		_setConfig('unicode-le.notificationsLevel', 'all');
		_setActiveEditor(_createDocument({ content: '' }));
		await runCommand('unicode-le.detect');
		expect(_shownMessages()[0]?.message).toBe('File is empty');
		expect(_openedDocuments()).toHaveLength(0);
	});

	it('opens a report naming each finding, with the key path the format allows', async () => {
		_setActiveEditor(
			_createDocument({
				content: '{\n  "auth": { "provider": "pаypal" }\n}\n',
				languageId: 'json',
				fileName: '/w/a.json',
			}),
		);
		await runCommand('unicode-le.detect');
		expect(report()).toContain('mixed-script');
		expect(report()).toContain('Key `auth.provider`');
		expect(report()).not.toContain('а');
	});

	it('copies the report when asked to', async () => {
		_setConfig('unicode-le.copyToClipboardEnabled', true);
		_setActiveEditor(_createDocument({ content: 'a​b' }));
		await runCommand('unicode-le.detect');
		expect(_clipboardText()).toContain('U+200B');
	});

	it('shows no positions when the setting is off, and still names the codepoint', async () => {
		_setConfig('unicode-le.showPositions', false);
		_setActiveEditor(_createDocument({ content: 'a​b' }));
		await runCommand('unicode-le.detect');
		expect(report()).not.toMatch(/\*\*\d+:\d+\*\*/);
		expect(report()).toContain('U+200B');
	});

	it('decides positions for the clipboard separately from the report', async () => {
		_setConfig('unicode-le.copyToClipboardEnabled', true);
		_setConfig('unicode-le.clipboardIncludesPositions', false);
		_setActiveEditor(_createDocument({ content: 'a​b' }));
		await runCommand('unicode-le.detect');
		expect(report()).toMatch(/\*\*\d+:\d+\*\*/);
		expect(_clipboardText()).not.toMatch(/\*\*\d+:\d+\*\*/);
		expect(_clipboardText()).toContain('U+200B');
	});

	it('runs only the kinds the settings name', async () => {
		_setConfig('unicode-le.detection.kinds', ['bidi']);
		_setActiveEditor(_createDocument({ content: 'a​b ‮' }));
		await runCommand('unicode-le.detect');
		expect(report()).toContain('U+202E');
		expect(report()).not.toContain('U+200B');
	});

	it('judges a translated file once its script is declared', async () => {
		const russian = 'ключ: значение\nдругой: текст\n';
		_setActiveEditor(_createDocument({ content: russian, languageId: 'yaml' }));
		await runCommand('unicode-le.detect');
		expect(report()).toContain('intentional_script_context');

		_setConfig('unicode-le.detection.scripts', ['Cyrillic']);
		await runCommand('unicode-le.detect');
		expect(report()).not.toContain('intentional_script_context');
	});

	it('refuses a script name nothing recognises, by name, and runs nothing', async () => {
		_setConfig('unicode-le.detection.scripts', ['Klingon']);
		_setActiveEditor(_createDocument({ content: 'x' }));
		await runCommand('unicode-le.detect');
		expect(_shownMessages()[0]?.message).toContain(
			'Klingon is not a Unicode script',
		);
		expect(_openedDocuments()).toHaveLength(0);
	});
});

const TREE = {
	'/w/src/clean.ts': 'const total = 1;\n',
	'/w/src/login.ts': 'const pаypal = "‮";\n',
	'/w/notes.txt': new Uint8Array([0xff, 0xfe, 0x41, 0x00]),
	'/w/latin1.txt': new Uint8Array([0x63, 0x61, 0x66, 0xe9]),
	'/w/node_modules/x/index.js': 'const a = "‮";\n',
	'/w/logo.png': new Uint8Array([0x89, 0x50, 0x00, 0x47]),
};

function open(files: Record<string, string | Uint8Array> = TREE): void {
	_setWorkspaceFiles(files);
	workspace.workspaceFolders = [{ uri: Uri.file('/w'), name: 'w', index: 0 }];
}

describe('unicode-le.scanWorkspace and unicode-le.scanFolder', () => {
	it('warns when no workspace is open', async () => {
		_setConfig('unicode-le.notificationsLevel', 'important');
		await runCommand('unicode-le.scanWorkspace');
		expect(_shownMessages()[0]?.kind).toBe('warning');
	});

	it('reports each file with findings, and refuses what is not UTF-8 by name', async () => {
		open();
		await runCommand('unicode-le.scanWorkspace');
		const text = report();
		expect(text).toContain('## `/w/src/login.ts`');
		expect(text).not.toContain('`/w/src/clean.ts`');
		expect(text).toMatch(
			/## `\/w\/notes\.txt`\n\n- \*\*Not judged\*\* \(`encoding_unknown`\)/,
		);
		expect(text).toMatch(
			/## `\/w\/latin1\.txt`\n\n- \*\*Not judged\*\* \(`binary_or_undecodable`\)/,
		);
		// Four were read: the dependency and the image never were.
		expect(text).toContain('in 4 file(s) scanned; 2 file(s) not fully judged.');
		expect(text).not.toContain('node_modules');
		expect(text).not.toContain('logo.png');
		expect(text).toContain(
			'> Not read: dependency folders, build output, caches and lockfiles; images, fonts, archives and other binary files; 0 file(s) ignored by .gitignore. The `unicode-le.workspace.*` settings change this.',
		);
	});

	it('reads a dependency folder and a binary file when the switches are off', async () => {
		open();
		_setConfig('unicode-le.workspace.scanUseDefaultExcludes', false);
		_setConfig('unicode-le.workspace.scanSkipBinaryFiles', false);
		await runCommand('unicode-le.scanWorkspace');
		expect(report()).toContain('## `/w/node_modules/x/index.js`');
		// Read, and refused by name: a NUL byte was never text.
		expect(report()).toMatch(
			/## `\/w\/logo\.png`\n\n- \*\*Not judged\*\* \(`binary_or_undecodable`\)/,
		);
		expect(report()).toContain('in 6 file(s) scanned');
	});

	it('reads one path a switch would skip when it is always included', async () => {
		open();
		_setConfig('unicode-le.workspace.scanAlwaysInclude', [
			'**/node_modules/**',
		]);
		await runCommand('unicode-le.scanWorkspace');
		expect(report()).toContain('## `/w/node_modules/x/index.js`');
	});

	it('orders the report by path, whatever order the search returned', async () => {
		open({ '/w/z.txt': '‮', '/w/src/b.ts': '‮', '/w/a.txt': '‮' });
		await runCommand('unicode-le.scanWorkspace');
		const headings = report().match(/^## .+$/gm);
		expect(headings).toEqual([
			'## `/w/a.txt`',
			'## `/w/src/b.ts`',
			'## `/w/z.txt`',
		]);
	});

	it('scans only the folder it is handed, and names files relative to it', async () => {
		open({ ...TREE, '/w/other/x.ts': '‮' });
		await runCommand('unicode-le.scanFolder', Uri.file('/w/src'));
		expect(report()).toContain('## `login.ts`');
		expect(report()).toContain('in 2 file(s) scanned');
		expect(report()).not.toContain('x.ts');
	});

	it('asks for a folder from the palette, and does nothing when none is picked', async () => {
		open();
		_respondToOpenDialog(() => undefined);
		await runCommand('unicode-le.scanFolder');
		expect(_openedDocuments()).toHaveLength(0);

		_respondToOpenDialog(() => [Uri.file('/w/src')]);
		await runCommand('unicode-le.scanFolder');
		expect(report()).toContain('## `login.ts`');
	});

	it('leaves a file over the safety limit unread, and says so', async () => {
		_setConfig('unicode-le.safety.fileSizeWarnBytes', 1000);
		open({ '/w/big.txt': `${'a'.repeat(2000)}‮`, '/w/small.txt': '​' });
		await runCommand('unicode-le.scanWorkspace');
		expect(report()).toContain(
			'> 1 file(s) larger than the safety limit were not read.',
		);
		expect(report()).not.toContain('`/w/big.txt`');
	});

	it('stops at the results limit and says the rest was not read', async () => {
		open({ '/w/a.txt': '‮‮‮', '/w/b.txt': '‮' });
		_setConfig('unicode-le.workspace.scanMaxResults', 2);
		await runCommand('unicode-le.scanWorkspace');
		expect(report()).toContain('2 finding(s) in 1 file(s) scanned');
		expect(report()).not.toContain('`/w/b.txt`');
		expect(report()).toContain(
			'> The results limit was reached. The rest of the files were not read.',
		);
	});

	it('keeps findings out of the Problems panel unless asked, and replaces them each scan', async () => {
		open();
		await runCommand('unicode-le.scanWorkspace');
		expect(_diagnostics().size).toBe(0);

		_setConfig('unicode-le.workspace.scanProblemsEnabled', true);
		await runCommand('unicode-le.scanWorkspace');
		const problems = _diagnostics().get('/w/src/login.ts') ?? [];
		expect(problems.length).toBeGreaterThan(0);
		// The codepoint, never the character it names.
		expect(problems.map((p) => p.message).join('\n')).toContain('U+202E');
		expect(problems.map((p) => p.message).join('\n')).not.toContain('‮');
		expect(problems.every((p) => p.source === 'unicode-le')).toBe(true);

		open({ '/w/clean.txt': 'clean\n' });
		await runCommand('unicode-le.scanWorkspace');
		expect(_diagnostics().size).toBe(0);
	});

	it('follows the positions setting on screen, and decides the copy separately', async () => {
		open();
		_setConfig('unicode-le.showPositions', false);
		_setConfig('unicode-le.copyToClipboardEnabled', true);
		await runCommand('unicode-le.scanWorkspace');
		expect(report()).not.toMatch(/\*\*\d+:\d+\*\*/);
		expect(_clipboardText()).toMatch(/\*\*\d+:\d+\*\*/);
		expect(_clipboardText()).toContain('> Not read: ');
	});

	it('prints the report the README shows as its sample', async () => {
		open();
		await runCommand('unicode-le.scanFolder', Uri.file('/w'));

		const readme = readFileSync(
			join(__dirname, '..', '..', 'README.md'),
			'utf8',
		);
		const shown = report()
			.split('\n')
			.filter((line) => line.startsWith('- ') || line.startsWith('## '));
		expect(shown).toHaveLength(8);
		for (const line of shown) expect(readme).toContain(line);
		expect(readme).toContain(
			'3 finding(s) in 4 file(s) scanned; 2 file(s) not fully judged.',
		);
	});
});

describe('settings and help', () => {
	it('opens the settings filtered to this extension', async () => {
		const { deps } = makeDeps();
		registerOpenSettingsCommand(
			_createExtensionContext() as never,
			deps.telemetry,
		);
		await runCommand('unicode-le.openSettings');
		expect(executedBuiltins.at(-1)).toMatchObject({ args: ['unicode-le.'] });
	});

	it('opens the help, which names every kind', async () => {
		const { deps } = makeDeps();
		registerHelpCommand(_createExtensionContext() as never, deps.telemetry);
		await runCommand('unicode-le.help');
		for (const kind of [
			'bidi-control',
			'confusable',
			'mixed-script',
			'invisible',
			'non-nfc',
			'unusual-whitespace',
		]) {
			expect(generateHelpContent()).toContain(kind);
		}
	});
});
