export const DROP_TIMEOUT_MS = 10000;
export const DROP_POLL_MS = 50;

export interface DropQueue {
	paths: string[];
	first: number;
	send: (path: string, start: number) => Promise<boolean>;
	arrived: (start: number) => number | null;
	mark: () => void;
	fail: (path: string) => void;
	timeoutMs?: number;
	pollMs?: number;
	sleep?: (ms: number) => Promise<void>;
	now?: () => number;
}

const wait = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

/**
 * Imports files one at a time. Each file starts where the previous block ends,
 * once `arrived` reports that block's end beat. A failed send or a block that
 * does not show up within the timeout is reported through `fail`, and the
 * queue moves on to the next file at the same start.
 */
export async function runDropQueue(queue: DropQueue): Promise<void> {
	const timeout = queue.timeoutMs ?? DROP_TIMEOUT_MS;
	const poll = queue.pollMs ?? DROP_POLL_MS;
	const sleep = queue.sleep ?? wait;
	const now = queue.now ?? Date.now;
	let start = queue.first;
	for (const path of queue.paths) {
		queue.mark();
		if (!(await queue.send(path, start))) {
			queue.fail(path);
			continue;
		}
		const deadline = now() + timeout;
		let end = queue.arrived(start);
		while (end === null && now() < deadline) {
			await sleep(poll);
			end = queue.arrived(start);
		}
		if (end === null) queue.fail(path);
		else start = end;
	}
}
