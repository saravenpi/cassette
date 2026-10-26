import { runDropQueue } from './dropQueue';

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void | Promise<void>) => void;
declare const expect: (actual: unknown) => { toEqual(expected: unknown): void };

function harness(lengths: Record<string, number | null>, sendOk: Record<string, boolean> = {}) {
	let clock = 0;
	const sent: [string, number][] = [];
	const failed: string[] = [];
	let current: string | null = null;
	const queue = {
		first: 8,
		now: () => clock,
		sleep: async (ms: number) => {
			clock += ms;
		},
		timeoutMs: 100,
		pollMs: 10,
		mark: () => {},
		fail: (path: string) => failed.push(path),
		send: async (path: string, start: number) => {
			sent.push([path, start]);
			current = path;
			return sendOk[path] ?? true;
		},
		arrived: (start: number) => {
			const length = current === null ? null : lengths[current];
			return length === null || length === undefined ? null : start + length;
		}
	};
	return { queue, sent, failed };
}

describe('runDropQueue', () => {
	test('places each file at the previous block end', async () => {
		const { queue, sent, failed } = harness({ a: 3, b: 5, c: 1 });
		await runDropQueue({ ...queue, paths: ['a', 'b', 'c'] });
		expect(sent).toEqual([['a', 8], ['b', 11], ['c', 16]]);
		expect(failed).toEqual([]);
	});

	test('a block that never appears times out and the queue continues', async () => {
		const { queue, sent, failed } = harness({ a: null, b: 2 });
		await runDropQueue({ ...queue, paths: ['a', 'b'] });
		expect(sent).toEqual([['a', 8], ['b', 8]]);
		expect(failed).toEqual(['a']);
	});

	test('a failed send is reported and the queue continues', async () => {
		const { queue, sent, failed } = harness({ a: 4, b: 2 }, { a: false });
		await runDropQueue({ ...queue, paths: ['a', 'b'] });
		expect(sent).toEqual([['a', 8], ['b', 8]]);
		expect(failed).toEqual(['a']);
	});
});
