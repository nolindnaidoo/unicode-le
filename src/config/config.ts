import * as vscode from 'vscode';
import {
	type Configuration,
	KINDS,
	type KindFilter,
	type NotificationLevel,
} from '../types';

/**
 * The defaults, exported for the parity gate: `config.test.ts` asserts they
 * match every default declared in package.json, which is what stops the two
 * drifting apart.
 */
export const CONFIG_DEFAULTS = Object.freeze({
	clipboardIncludesPositions: true,
	copyToClipboardEnabled: false,
	detectionKinds: Object.freeze([] as KindFilter[]),
	detectionScripts: Object.freeze([] as string[]),
	notificationsLevel: 'silent' as const,
	openResultsSideBySide: true,
	safetyEnabled: true,
	safetyFileSizeWarnBytes: 1_000_000,
	showPositions: true,
	statusBarEnabled: true,
	telemetryEnabled: false,
	workspaceScanExcludes: Object.freeze([
		'**/node_modules/**',
		'**/.git/**',
		'**/dist/**',
		'**/build/**',
		'**/target/**',
		'**/*.min.js',
	]),
	workspaceScanMaxFiles: 5000,
	workspaceScanPatterns: Object.freeze(['**/*']),
});

export function readConfig(): Configuration {
	const config = vscode.workspace.getConfiguration('unicode-le');
	return Object.freeze({
		clipboardIncludesPositions: readBoolean(
			config,
			'clipboardIncludesPositions',
			CONFIG_DEFAULTS.clipboardIncludesPositions,
		),
		copyToClipboardEnabled: readBoolean(
			config,
			'copyToClipboardEnabled',
			CONFIG_DEFAULTS.copyToClipboardEnabled,
		),
		detectionKinds: Object.freeze(
			readStrings(
				config,
				'detection.kinds',
				CONFIG_DEFAULTS.detectionKinds,
			).filter(isKindFilter),
		),
		detectionScripts: Object.freeze(
			readStrings(
				config,
				'detection.scripts',
				CONFIG_DEFAULTS.detectionScripts,
			),
		),
		notificationsLevel: readNotificationLevel(config),
		openResultsSideBySide: readBoolean(
			config,
			'openResultsSideBySide',
			CONFIG_DEFAULTS.openResultsSideBySide,
		),
		safetyEnabled: readBoolean(
			config,
			'safety.enabled',
			CONFIG_DEFAULTS.safetyEnabled,
		),
		safetyFileSizeWarnBytes: readNumber(
			config,
			'safety.fileSizeWarnBytes',
			CONFIG_DEFAULTS.safetyFileSizeWarnBytes,
			1000,
		),
		showPositions: readBoolean(
			config,
			'showPositions',
			CONFIG_DEFAULTS.showPositions,
		),
		statusBarEnabled: readBoolean(
			config,
			'statusBar.enabled',
			CONFIG_DEFAULTS.statusBarEnabled,
		),
		telemetryEnabled: readBoolean(
			config,
			'telemetryEnabled',
			CONFIG_DEFAULTS.telemetryEnabled,
		),
		workspaceScanExcludes: Object.freeze(
			readStrings(
				config,
				'workspace.scanExcludes',
				CONFIG_DEFAULTS.workspaceScanExcludes,
			),
		),
		workspaceScanMaxFiles: readNumber(
			config,
			'workspace.scanMaxFiles',
			CONFIG_DEFAULTS.workspaceScanMaxFiles,
			1,
		),
		workspaceScanPatterns: Object.freeze(
			readStrings(
				config,
				'workspace.scanPatterns',
				CONFIG_DEFAULTS.workspaceScanPatterns,
			),
		),
	});
}

function readBoolean(
	config: vscode.WorkspaceConfiguration,
	key: string,
	defaultValue: boolean,
): boolean {
	const value = config.get(key, defaultValue);
	return typeof value === 'boolean' ? value : defaultValue;
}

function readNumber(
	config: vscode.WorkspaceConfiguration,
	key: string,
	defaultValue: number,
	minValue: number,
): number {
	const value = Number(config.get(key, defaultValue));
	if (!Number.isFinite(value)) return defaultValue;
	return Math.max(minValue, value);
}

function readStrings(
	config: vscode.WorkspaceConfiguration,
	key: string,
	defaultValue: readonly string[],
): readonly string[] {
	const value = config.get<unknown>(key, defaultValue);
	return Array.isArray(value)
		? value.filter((item): item is string => typeof item === 'string')
		: defaultValue;
}

function isKindFilter(value: string): value is KindFilter {
	return KINDS.some(([, short]) => short === value);
}

export function isValidNotificationLevel(v: unknown): v is NotificationLevel {
	return v === 'all' || v === 'important' || v === 'silent';
}

function readNotificationLevel(
	config: vscode.WorkspaceConfiguration,
): NotificationLevel {
	const raw = config.get<string>(
		'notificationsLevel',
		CONFIG_DEFAULTS.notificationsLevel,
	);
	return isValidNotificationLevel(raw)
		? raw
		: CONFIG_DEFAULTS.notificationsLevel;
}
