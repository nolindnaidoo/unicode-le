/**
 * The terminal surface, as the Rust CLI's `cli.rs` runs it.
 *
 * stdout is protocol: one JSON document whenever something is reading it, and
 * nothing when it is a terminal. Every message is escaped before it is written,
 * so no character this tool exists to find reaches a terminal.
 */
import { escapeNonAscii } from '../utils/escape';
import { type Cli, parse } from './args';
import { rustIoMessage } from './ioError';
import {
	describe,
	describeRefusal,
	exitCode,
	type FileReport,
	type Report,
	report,
	scanBytes,
	scanFile,
} from './scan';
import { USAGE } from './usage';
import { collect } from './walk';

export interface RunIo {
	readonly stdout: (text: string) => void;
	readonly stderr: (text: string) => void;
	readonly stdoutIsTerminal: boolean;
	readonly readStdin: () => Promise<Uint8Array>;
	readonly version: string;
}

export async function run(args: readonly string[], io: RunIo): Promise<number> {
	const first = args[0];
	if (first === '--help' || first === '-h') {
		io.stdout(`${USAGE}\n`);
		return 0;
	}
	if (first === '--version' || first === '-V') {
		io.stdout(`unicode-le ${io.version}\n`);
		return 0;
	}
	try {
		return await execute(parse(args), io);
	} catch (error) {
		const message = error instanceof Error ? error.message : String(error);
		io.stderr(`unicode-le: ${escapeNonAscii(message)}\n`);
		return 2;
	}
}

async function execute(cli: Cli, io: RunIo): Promise<number> {
	let files: FileReport[];
	if (cli.stdin) {
		let bytes: Uint8Array;
		try {
			bytes = await io.readStdin();
		} catch (error) {
			throw new Error(`could not read stdin: ${rustIoMessage(error)}`);
		}
		files = [scanBytes(bytes, '<stdin>', cli.detect)];
	} else {
		files = collect(cli.inputs, cli.walk).map((file) =>
			scanFile(file, cli.detect),
		);
	}
	const run = report(files);
	if (!io.stdoutIsTerminal)
		io.stdout(`${escapeNonAscii(JSON.stringify(run))}\n`);
	io.stderr(summary(run));
	return exitCode(run, cli.failOn, cli.strict);
}

export function summary(run: Report): string {
	const lines: string[] = [];
	for (const file of run.files) {
		for (const refusal of file.refusals)
			lines.push(describeRefusal(file.file, refusal));
		for (const finding of file.findings) lines.push(describe(file, finding));
	}
	const refused =
		run.summary.refusals === 0
			? ''
			: `, ${run.summary.refusals} refused (${run.summary.unexamined} not read at all)`;
	lines.push(
		`${plural(run.summary.findings, 'finding', 'findings')} in ${plural(run.summary.files, 'file', 'files')}${refused}`,
	);
	const scriptContext = run.files.some((file) =>
		file.refusals.some(
			(refusal) => refusal.reason === 'intentional_script_context',
		),
	);
	if (scriptContext) {
		lines.push(
			'name the expected script with --script to have those files judged too',
		);
	}
	return `${lines.join('\n')}\n`;
}

function plural(count: number, one: string, many: string): string {
	return `${count} ${count === 1 ? one : many}`;
}
