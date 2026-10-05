import { beforeEach, describe, expect, it } from 'vitest';
import {
	_clipboardText,
	_createDocument,
	_createExtensionContext,
	_openedDocuments,
	_registeredCommands,
	_resetMockState,
	_setActiveEditor,
	_setConfig,
	_setWorkspaceFiles,
	_shownMessages,
	executedBuiltins,
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
		deps: { notifier: createNotifier(), statusBar, telemetry },
		events,
		flashes,
	};
}

async function runCommand(id: string): Promise<void> {
	const handler = _registeredCommands().get(id);
	if (!handler) throw new Error(`command not registered: ${id}`);
	await handler();
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

describe('unicode-le.scanWorkspace', () => {
	it('warns when no workspace is open', async () => {
		_setConfig('unicode-le.notificationsLevel', 'important');
		await runCommand('unicode-le.scanWorkspace');
		expect(_shownMessages()[0]?.kind).toBe('warning');
	});

	it('reports each file with findings, and refuses what is not UTF-8 by name', async () => {
		_setWorkspaceFiles({
			'src/clean.ts': 'const total = 1;\n',
			'src/login.ts': 'const pаypal = "‮";\n',
			'notes.txt': new Uint8Array([0xff, 0xfe, 0x41, 0x00]),
			'latin1.txt': new Uint8Array([0x63, 0x61, 0x66, 0xe9]),
		});
		await runCommand('unicode-le.scanWorkspace');
		const text = report();
		expect(text).toContain('## `src/login.ts`');
		expect(text).not.toContain('`src/clean.ts`');
		expect(text).toContain('`encoding_unknown`');
		expect(text).toContain('`binary_or_undecodable`');
		expect(text).toContain('in 4 file(s) scanned');
	});

	it('orders the report by path, whatever order the search returned', async () => {
		_setWorkspaceFiles({ 'z.txt': '‮', 'src/b.ts': '‮', 'a.txt': '‮' });
		await runCommand('unicode-le.scanWorkspace');
		const headings = report().match(/^## .+$/gm);
		expect(headings).toEqual(['## `a.txt`', '## `src/b.ts`', '## `z.txt`']);
	});

	it('leaves a file over the safety limit unread, and says so', async () => {
		_setConfig('unicode-le.safety.fileSizeWarnBytes', 1000);
		_setWorkspaceFiles({ 'big.txt': `${'a'.repeat(2000)}‮`, 'small.txt': '​' });
		await runCommand('unicode-le.scanWorkspace');
		expect(report()).toContain(
			'1 file(s) larger than the safety limit were not read.',
		);
		expect(report()).not.toContain('`big.txt`');
	});

	it('skips what the excludes name', async () => {
		_setWorkspaceFiles({ 'node_modules/x/index.js': '‮', 'src/a.js': 'clean' });
		await runCommand('unicode-le.scanWorkspace');
		expect(report()).not.toContain('node_modules');
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
