/**
 * The command line, parsed as the Rust CLI parses it: strictly, in order, and
 * refusing rather than defaulting.
 */
import { parseKind, parseScript } from '../detection';
import { trim } from '../detection/text';
import type { Kind, Options } from '../types';
import { FLAGS } from './usage';

export type FailOn = 'any' | 'bidi';

export interface WalkOptions {
	readonly hidden: boolean;
	readonly respectIgnore: boolean;
}

export interface Cli {
	readonly strict: boolean;
	readonly inputs: readonly string[];
	readonly stdin: boolean;
	readonly failOn: FailOn;
	readonly detect: Options;
	readonly walk: WalkOptions;
}

export function parse(args: readonly string[]): Cli {
	const kinds: Kind[] = [];
	const scripts: number[] = [];
	const inputs: string[] = [];
	let stdin = false;
	let strict = false;
	let failOn: FailOn = 'any';
	let walk: WalkOptions = { hidden: false, respectIgnore: true };

	for (let index = 0; index < args.length; index++) {
		const arg = args[index] as string;
		if (arg.startsWith('-') && !FLAGS.includes(arg)) {
			throw new Error(`${arg} is not an option. Try --help.`);
		}
		const values = () => {
			index += 1;
			return listed(args[index], arg);
		};
		if (arg === '--stdin') stdin = true;
		if (arg === '--strict') strict = true;
		if (arg === '--hidden') walk = { ...walk, hidden: true };
		if (arg === '--no-ignore') walk = { ...walk, respectIgnore: false };
		if (arg === '--kind') kinds.push(...values().map(parseKind));
		if (arg === '--script') scripts.push(...values().map(parseScript));
		if (arg === '--fail-on') failOn = failOnOf(values().join(','));
		if (!arg.startsWith('-')) inputs.push(arg);
	}

	if (stdin && inputs.length > 0) {
		throw new Error('reading from stdin takes no file arguments');
	}
	if (!stdin && inputs.length === 0) {
		throw new Error('name a file or a directory to scan. Try --help.');
	}
	if (
		failOn === 'bidi' &&
		kinds.length > 0 &&
		!kinds.includes('bidi-control')
	) {
		throw new Error(
			'--fail-on bidi cannot be answered while --kind excludes bidi: the filter decides what the gate counts. Add bidi to --kind, or drop --fail-on.',
		);
	}
	return Object.freeze({
		strict,
		inputs,
		stdin,
		failOn,
		detect: { kinds, expectedScripts: scripts },
		walk,
	});
}

/** A comma-separated value, each part trimmed as Rust trims, none empty. */
function listed(raw: string | undefined, flag: string): string[] {
	if (raw === undefined) throw new Error(`${flag} needs a value`);
	const values = raw
		.split(',')
		.map((value) => trim(value))
		.filter((value) => value.length > 0);
	if (values.length === 0) throw new Error(`${flag} needs a value`);
	return values;
}

function failOnOf(value: string): FailOn {
	if (value === 'any' || value === 'bidi') return value;
	throw new Error(`${value} is not something to fail on; one of: any, bidi`);
}
