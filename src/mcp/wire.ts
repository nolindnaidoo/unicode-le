/**
 * The pipe to the client, with every frame escaped after serialization, as
 * the crate's server escapes: a reply can carry text this server did not
 * write, a key path out of the document, and a frame that pasted it raw
 * would reorder whatever renders the log. A client parses `\uXXXX` back to
 * the identical string.
 *
 * The transport is the family's copy and knows nothing of this, so the
 * escaping sits on the pipe rather than in it. Backpressure passes through:
 * a frame the pipe will not take yet holds this writer's callback, and the
 * transport stops reading as soon as one is held.
 */
import { Writable } from 'node:stream';
import { escapeNonAscii } from '../utils/escape';

export function escapedWriter(pipe: NodeJS.WritableStream): Writable {
	return new Writable({
		decodeStrings: false,
		highWaterMark: 1,
		write(chunk: string | Buffer, _encoding, callback) {
			if (pipe.write(escapeNonAscii(String(chunk)))) callback();
			else pipe.once('drain', () => callback());
		},
	});
}
