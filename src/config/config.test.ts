import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { _resetMockState, _setConfig } from '../__mocks__/vscode';
import { KINDS } from '../types';
import {
	CONFIG_DEFAULTS,
	isValidNotificationLevel,
	readConfig,
} from './config';

/**
 * CONFIG_DEFAULTS must stay identical to the defaults declared in package.json
 * contributes.configuration; a sibling shipped with the two silently
 * disagreeing.
 */
describe('config defaults parity with package.json', () => {
	const manifest = JSON.parse(
		readFileSync(join(__dirname, '..', '..', 'package.json'), 'utf8'),
	) as {
		contributes: {
			configuration: {
				properties: Record<
					string,
					{ default: unknown; items?: { enum?: string[] } }
				>;
			};
		};
	};
	const props = manifest.contributes.configuration.properties;

	const KEY_MAP: Record<string, keyof typeof CONFIG_DEFAULTS> = {
		'unicode-le.clipboardIncludesPositions': 'clipboardIncludesPositions',
		'unicode-le.copyToClipboardEnabled': 'copyToClipboardEnabled',
		'unicode-le.detection.kinds': 'detectionKinds',
		'unicode-le.detection.scripts': 'detectionScripts',
		'unicode-le.notificationsLevel': 'notificationsLevel',
		'unicode-le.openResultsSideBySide': 'openResultsSideBySide',
		'unicode-le.safety.enabled': 'safetyEnabled',
		'unicode-le.safety.fileSizeWarnBytes': 'safetyFileSizeWarnBytes',
		'unicode-le.showPositions': 'showPositions',
		'unicode-le.statusBar.enabled': 'statusBarEnabled',
		'unicode-le.telemetryEnabled': 'telemetryEnabled',
		'unicode-le.workspace.scanExcludes': 'workspaceScanExcludes',
		'unicode-le.workspace.scanMaxFiles': 'workspaceScanMaxFiles',
		'unicode-le.workspace.scanPatterns': 'workspaceScanPatterns',
	};

	it('covers every declared setting', () => {
		expect(Object.keys(props).sort()).toEqual(Object.keys(KEY_MAP).sort());
	});

	for (const [manifestKey, defaultsKey] of Object.entries(KEY_MAP)) {
		it(`${manifestKey} default matches`, () => {
			expect(CONFIG_DEFAULTS[defaultsKey]).toEqual(props[manifestKey]?.default);
		});
	}

	it('offers exactly the kinds the detector filters on', () => {
		expect(props['unicode-le.detection.kinds']?.items?.enum).toEqual(
			KINDS.map(([, short]) => short),
		);
	});
});

describe('readConfig', () => {
	afterEach(() => _resetMockState());

	it('drops a kind the detector does not know rather than failing every command', () => {
		_setConfig('unicode-le.detection.kinds', ['bidi', 'homoglyph', 3]);
		expect(readConfig().detectionKinds).toEqual(['bidi']);
	});

	it('falls back to the default for a value of the wrong type', () => {
		_setConfig('unicode-le.workspace.scanPatterns', 'src/**');
		_setConfig('unicode-le.workspace.scanMaxFiles', 'many');
		expect(readConfig().workspaceScanPatterns).toEqual(
			CONFIG_DEFAULTS.workspaceScanPatterns,
		);
		expect(readConfig().workspaceScanMaxFiles).toBe(
			CONFIG_DEFAULTS.workspaceScanMaxFiles,
		);
	});
});

describe('isValidNotificationLevel', () => {
	it('accepts the three declared levels and nothing else', () => {
		for (const level of ['all', 'important', 'silent'])
			expect(isValidNotificationLevel(level)).toBe(true);
		expect(isValidNotificationLevel('verbose')).toBe(false);
		expect(isValidNotificationLevel(null)).toBe(false);
	});
});
