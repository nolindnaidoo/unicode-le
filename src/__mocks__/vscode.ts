/**
 * Mock VS Code API for unit tests (aliased via vitest.config.ts).
 * Stateful pieces (config store, message log, command registry, opened
 * documents) expose `_reset()`/`_set()` helpers prefixed with
 * underscore — test-only API.
 */

export interface WorkspaceFolder {
	readonly uri: Uri;
	readonly name: string;
	readonly index: number;
}

// ---------------------------------------------------------------- Uri

export class Uri {
	scheme: string;
	authority: string;
	path: string;
	query: string;
	fragment: string;

	constructor(
		scheme: string,
		authority: string,
		path: string,
		query: string,
		fragment: string,
	) {
		this.scheme = scheme;
		this.authority = authority;
		this.path = path;
		this.query = query;
		this.fragment = fragment;
	}

	get fsPath(): string {
		return this.path;
	}

	toString(_skipEncoding?: boolean): string {
		return `${this.scheme}://${this.authority}${this.path}`;
	}

	static file(path: string): Uri {
		return new Uri('file', '', path, '', '');
	}

	static parse(value: string): Uri {
		const match = value.match(/^(\w+):\/\/([^/]*)(.*)$/);
		if (match?.[1] && match[2] !== undefined && match[3] !== undefined) {
			return new Uri(match[1], match[2], match[3], '', '');
		}
		return new Uri('file', '', value, '', '');
	}
}

// ---------------------------------------------- positions and ranges

export class Position {
	constructor(
		public readonly line: number,
		public readonly character: number,
	) {}
}

export class Range {
	constructor(
		public readonly start: Position,
		public readonly end: Position,
	) {}
}

export class Selection extends Range {}

// ---------------------------------------------------------- documents

export interface MockDocumentInit {
	readonly content: string;
	readonly languageId?: string;
	readonly fileName?: string;
}

export function _createDocument(init: MockDocumentInit) {
	let content = init.content;
	const doc = {
		get lineCount() {
			return content.split('\n').length;
		},
		getText: () => content,
		_setText: (next: string) => {
			content = next;
		},
		languageId: init.languageId ?? 'plaintext',
		fileName: init.fileName ?? '/mock/document.txt',
		uri: Uri.file(init.fileName ?? '/mock/document.txt'),
		positionAt: (offset: number) => {
			const lines = content.split('\n');
			let remaining = Math.max(0, Math.min(offset, content.length));
			for (let line = 0; line < lines.length; line++) {
				const length = (lines[line] ?? '').length;
				if (remaining <= length) return new Position(line, remaining);
				remaining -= length + 1;
			}
			return new Position(
				lines.length - 1,
				(lines[lines.length - 1] ?? '').length,
			);
		},
		lineAt: (line: number) => {
			const lines = content.split('\n');
			return {
				text: lines[line] ?? '',
				range: new Range(
					new Position(line, 0),
					new Position(line, (lines[line] ?? '').length),
				),
			};
		},
	};
	return doc;
}

export type MockDocument = ReturnType<typeof _createDocument>;

// ------------------------------------------------------------ editors

export interface RecordedEdit {
	readonly kind: 'replace' | 'insert';
	readonly text: string;
}

export function _createEditor(document: MockDocument) {
	const edits: RecordedEdit[] = [];
	return {
		document,
		_edits: edits,
		edit: async (
			callback: (builder: {
				replace(range: Range, text: string): void;
				insert(position: Position, text: string): void;
			}) => void,
		) => {
			callback({
				replace: (_range: Range, text: string) => {
					edits.push({ kind: 'replace', text });
					document._setText(text);
				},
				insert: (_position: Position, text: string) => {
					edits.push({ kind: 'insert', text });
					document._setText(document.getText() + text);
				},
			});
			return true;
		},
	};
}

export type MockEditor = ReturnType<typeof _createEditor>;

// ------------------------------------------------------ configuration

const configStore = new Map<string, unknown>();
const configUpdates: Array<{ key: string; value: unknown; target: unknown }> =
	[];

export function _setConfig(key: string, value: unknown): void {
	configStore.set(key, value);
}

export function _getConfigUpdates(): ReadonlyArray<{
	key: string;
	value: unknown;
	target: unknown;
}> {
	return configUpdates;
}

export const ConfigurationTarget = {
	Global: 1,
	Workspace: 2,
	WorkspaceFolder: 3,
};

type ConfigListener = (event: {
	affectsConfiguration: (section: string) => boolean;
}) => void;
const configListeners: ConfigListener[] = [];

export function _fireConfigChange(section: string): void {
	for (const listener of configListeners) {
		listener({
			affectsConfiguration: (candidate: string) =>
				section === candidate || section.startsWith(`${candidate}.`),
		});
	}
}

// --------------------------------------------------------- workspace

const openedDocuments: MockDocument[] = [];

export function _openedDocuments(): readonly MockDocument[] {
	return openedDocuments;
}

let fsStatSize = 0;

export function _setFsStatSize(size: number): void {
	fsStatSize = size;
}

/** An in-memory workspace for the scan: relative path to the file's bytes. */
const workspaceFiles = new Map<string, Uint8Array>();
const WORKSPACE_ROOT = '/mock/workspace/';

export function _setWorkspaceFiles(files: Readonly<Record<string, string | Uint8Array>>): void {
	workspaceFiles.clear();
	for (const [path, content] of Object.entries(files)) {
		workspaceFiles.set(path, typeof content === 'string' ? new TextEncoder().encode(content) : content);
	}
	workspace.workspaceFolders = [{ uri: Uri.file(WORKSPACE_ROOT), name: 'workspace', index: 0 }];
}

function relativeOf(uri: Uri): string {
	return uri.fsPath.startsWith(WORKSPACE_ROOT) ? uri.fsPath.slice(WORKSPACE_ROOT.length) : uri.fsPath;
}

export const workspace = {
	workspaceFolders: undefined as WorkspaceFolder[] | undefined,
	getWorkspaceFolder: (_uri: Uri) => undefined as WorkspaceFolder | undefined,
	findFiles: async (_include: string, exclude?: string, maxResults?: number) => {
		const excluded = (exclude ?? '').replace(/^\{|\}$/g, '').split(',').filter(Boolean);
		return [...workspaceFiles.keys()]
			.filter((path) => !excluded.some((glob) => path.includes(glob.replace(/\*\*\//g, '').replace(/\/\*\*$/g, ''))))
			.slice(0, maxResults)
			.map((path) => Uri.file(`${WORKSPACE_ROOT}${path}`));
	},
	asRelativePath: (target: Uri | string, _includeFolder?: boolean) =>
		typeof target === 'string' ? target : relativeOf(target),
	fs: {
		readFile: async (uri: Uri) => {
			const bytes = workspaceFiles.get(relativeOf(uri));
			if (bytes === undefined) throw new Error(`no such file: ${relativeOf(uri)}`);
			return bytes;
		},
		writeFile: async (_uri: Uri, _content: Uint8Array) => {},
		stat: async (_uri: Uri) => ({
			type: 1,
			ctime: 0,
			mtime: 0,
			size: workspaceFiles.get(relativeOf(_uri))?.length ?? fsStatSize,
		}),
	},
	getConfiguration: (section?: string) => ({
		get: <T>(key: string, defaultValue?: T): T | undefined => {
			const full = section ? `${section}.${key}` : key;
			return configStore.has(full)
				? (configStore.get(full) as T)
				: defaultValue;
		},
		update: async (key: string, value: unknown, target?: unknown) => {
			const full = section ? `${section}.${key}` : key;
			configStore.set(full, value);
			configUpdates.push({ key: full, value, target });
		},
	}),
	onDidChangeConfiguration: (listener: ConfigListener) => {
		configListeners.push(listener);
		return {
			dispose: () => {
				const index = configListeners.indexOf(listener);
				if (index >= 0) configListeners.splice(index, 1);
			},
		};
	},
	openTextDocument: async (options?: {
		content?: string;
		language?: string;
	}) => {
		const doc = _createDocument({
			content: options?.content ?? '',
			languageId: options?.language ?? 'plaintext',
		});
		openedDocuments.push(doc);
		return doc;
	},
};

// ------------------------------------------------------------ window

export interface ShownMessage {
	readonly kind: 'info' | 'warning' | 'error';
	readonly message: string;
	readonly items: readonly unknown[];
}

const shownMessages: ShownMessage[] = [];
const shownEditors: MockEditor[] = [];
const shownDocumentOptions: Array<{ viewColumn?: number }> = [];
let activeTextEditor: MockEditor | undefined;
let quickPickResponder: ((items: unknown[]) => unknown) | undefined;
let inputBoxResponder: (() => string | undefined) | undefined;
let warningResponder: ((items: unknown[]) => unknown) | undefined;

export function _shownMessages(): readonly ShownMessage[] {
	return shownMessages;
}

export function _shownDocumentOptions(): readonly { viewColumn?: number }[] {
	return shownDocumentOptions;
}

export function _shownEditors(): readonly MockEditor[] {
	return shownEditors;
}

export function _setActiveEditor(document: MockDocument | undefined): void {
	activeTextEditor = document ? _createEditor(document) : undefined;
}

export function _activeEditor(): MockEditor | undefined {
	return activeTextEditor;
}

export function _respondToQuickPick(
	responder: ((items: unknown[]) => unknown) | undefined,
): void {
	quickPickResponder = responder;
}

/** Values an input-box validator refused, in order. */
const inputBoxRejections: Array<{ value: string; message: string }> = [];

export function _inputBoxRejections(): readonly {
	value: string;
	message: string;
}[] {
	return inputBoxRejections;
}

export function _respondToInputBox(
	responder: (() => string | undefined) | undefined,
): void {
	inputBoxResponder = responder;
}

export function _respondToWarning(
	responder: ((items: unknown[]) => unknown) | undefined,
): void {
	warningResponder = responder;
}

export const StatusBarAlignment = { Left: 1, Right: 2 };
export const ViewColumn = { Active: -1, Beside: -2, One: 1, Two: 2 };
export const ProgressLocation = { Notification: 15, Window: 10 };

const cancellationToken = {
	isCancellationRequested: false,
	onCancellationRequested: (_listener: () => void) => ({ dispose: () => {} }),
};

export const window = {
	get activeTextEditor() {
		return activeTextEditor;
	},
	showInformationMessage: async (message: string, ...items: unknown[]) => {
		shownMessages.push({ kind: 'info', message, items });
		return undefined;
	},
	showWarningMessage: async (message: string, ...items: unknown[]) => {
		shownMessages.push({ kind: 'warning', message, items });
		return warningResponder?.(items);
	},
	showErrorMessage: async (message: string, ...items: unknown[]) => {
		shownMessages.push({ kind: 'error', message, items });
		return undefined;
	},
	showQuickPick: async (items: unknown[], _options?: unknown) =>
		quickPickResponder ? quickPickResponder(items) : undefined,
	showInputBox: async (options?: unknown) => {
		const value = inputBoxResponder ? inputBoxResponder() : undefined;

		// VS Code runs validateInput against what the user types and refuses to
		// accept a value the validator rejects — the box stays open until the
		// input is valid or the user escapes. Ignoring validateInput leaves the
		// validators uncovered AND lets a test hand a command a value the real UI
		// would never deliver.
		const validate = (options as { validateInput?: (v: string) => unknown })
			?.validateInput;
		if (typeof validate === 'function' && typeof value === 'string') {
			const message = validate(value);
			if (message !== undefined && message !== null && message !== '') {
				inputBoxRejections.push({ value, message: String(message) });
				return undefined;
			}
		}
		return value;
	},
	showTextDocument: async (document: unknown, options?: unknown) => {
		const editor = _createEditor(document as MockDocument);
		shownEditors.push(editor);
		// Recorded so tests can assert placement (openResultsSideBySide sets
		// viewColumn); the editor object itself carries no such field.
		shownDocumentOptions.push(
			(options as { viewColumn?: number } | undefined) ?? {},
		);
		return editor;
	},
	withProgress: async <T>(
		_options: unknown,
		task: (
			progress: { report: (value: unknown) => void },
			token: typeof cancellationToken,
		) => Promise<T>,
	): Promise<T> => task({ report: () => {} }, cancellationToken),
	createOutputChannel: (_name: string) => {
		const linesOut: string[] = [];
		outputChannels.push(linesOut);
		return {
			appendLine: (line: string) => linesOut.push(line),
			dispose: () => {},
			_lines: linesOut,
		};
	},
	createStatusBarItem: (_alignment?: unknown, _priority?: number) => ({
		text: '',
		tooltip: '',
		command: undefined as unknown,
		visible: false,
		show(): void {
			(this as { visible: boolean }).visible = true;
		},
		hide(): void {
			(this as { visible: boolean }).visible = false;
		},
		dispose: () => {},
	}),
};

const outputChannels: string[][] = [];

export function _outputChannelLines(): readonly string[] {
	return outputChannels.flat();
}

// ---------------------------------------------------------- commands

const registeredCommands = new Map<string, (...args: unknown[]) => unknown>();

export function _registeredCommands(): ReadonlyMap<
	string,
	(...args: unknown[]) => unknown
> {
	return registeredCommands;
}

export const commands = {
	registerCommand: (id: string, handler: (...args: unknown[]) => unknown) => {
		registeredCommands.set(id, handler);
		return {
			dispose: () => {
				registeredCommands.delete(id);
			},
		};
	},
	executeCommand: async (id: string, ...args: unknown[]) => {
		const handler = registeredCommands.get(id);
		if (handler) return handler(...args);
		executedBuiltins.push({ id, args });
		return undefined;
	},
};

export const executedBuiltins: Array<{ id: string; args: unknown[] }> = [];

// --------------------------------------------------------------- env

const clipboard = { value: '' };

export const env = {
	clipboard: {
		writeText: async (text: string) => {
			clipboard.value = text;
		},
		readText: async () => clipboard.value,
	},
	openExternal: async (_uri: Uri) => true,
};

export function _clipboardText(): string {
	return clipboard.value;
}

// ------------------------------------------------- extension context

export function _createExtensionContext() {
	const globalStateStore = new Map<string, unknown>();
	return {
		subscriptions: [] as Array<{ dispose(): void }>,
		globalState: {
			get: <T>(key: string, defaultValue?: T): T | undefined =>
				globalStateStore.has(key)
					? (globalStateStore.get(key) as T)
					: defaultValue,
			update: async (key: string, value: unknown) => {
				globalStateStore.set(key, value);
			},
		},
	};
}

export type MockExtensionContext = ReturnType<typeof _createExtensionContext>;

// -------------------------------------------------------------- misc

export const FileType = {
	Unknown: 0,
	File: 1,
	Directory: 2,
	SymbolicLink: 64,
};

/** Reset all mutable mock state between tests. */
export function _resetMockState(): void {
	inputBoxRejections.length = 0;
	configStore.clear();
	configUpdates.length = 0;
	configListeners.length = 0;
	shownMessages.length = 0;
	shownEditors.length = 0;
	shownDocumentOptions.length = 0;
	openedDocuments.length = 0;
	outputChannels.length = 0;
	executedBuiltins.length = 0;
	registeredCommands.clear();
	activeTextEditor = undefined;
	quickPickResponder = undefined;
	inputBoxResponder = undefined;
	warningResponder = undefined;
	clipboard.value = '';
	fsStatSize = 0;
	workspaceFiles.clear();
	workspace.workspaceFolders = undefined;
}

export const l10n = {
	t(message: string, ...args: unknown[]): string {
		if (args.length === 1 && typeof args[0] === 'object' && args[0] !== null) {
			const named = args[0] as Record<string, unknown>;
			return message.replace(/\{(\w+)\}/g, (whole, key) =>
				key in named ? String(named[key]) : whole,
			);
		}
		return message.replace(/\{(\d+)\}/g, (whole, index) => {
			const value = args[Number(index)];
			return value === undefined ? whole : String(value);
		});
	},
};
