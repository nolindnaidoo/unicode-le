import { EventEmitter } from 'node:events';
import { describe, expect, it } from 'vitest';
import { escapedWriter } from './wire';

/** A pipe that records what it was given and answers whether it has room. */
function pipe(room: boolean) {
	const written: string[] = [];
	const target = Object.assign(new EventEmitter(), {
		write: (chunk: string) => {
			written.push(chunk);
			return target.room;
		},
		room,
	});
	return { target, written };
}

describe('escapedWriter', () => {
	it('escapes every non-ASCII code unit of a frame, as the crate writes it', async () => {
		const { target, written } = pipe(true);
		const writer = escapedWriter(target as never);
		await new Promise<void>((resolve) => {
			writer.write('{"path":"café/\u{1D41A}"}\n', () => resolve());
		});
		expect(written).toEqual(['{"path":"caf\\u00E9/\\uD835\\uDC1A"}\n']);
	});

	it('holds a frame the pipe will not take yet, and tells the transport the pipe is full', async () => {
		const { target, written } = pipe(false);
		const writer = escapedWriter(target as never);
		expect(writer.write('{"id":1}\n')).toBe(false);
		expect(written).toEqual(['{"id":1}\n']);
		writer.write('{"id":2}\n');
		expect(written).toHaveLength(1);

		const drained = new Promise<void>((resolve) => {
			writer.once('drain', () => resolve());
		});
		target.room = true;
		target.emit('drain');
		await drained;
		expect(written).toEqual(['{"id":1}\n', '{"id":2}\n']);
	});
});
