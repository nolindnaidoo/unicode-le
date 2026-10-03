import { parseKind, parseScript } from '../detection';
import type { Configuration, Options } from '../types';

/**
 * The detection options a configuration asks for, or the reason it cannot be
 * honoured. A script name nothing recognises is refused by name rather than
 * dropped: dropping it would leave the homoglyph check refusing files the user
 * believes they declared.
 */
export function optionsFrom(config: Configuration): Options {
	return {
		kinds: config.detectionKinds.map(parseKind),
		expectedScripts: config.detectionScripts.map(parseScript),
	};
}
