import { BACKOFF_MAX_MS, BACKOFF_MIN_MS, KeyedQueue, Latest, nextBackoff } from './queue';

interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	toHaveLength(expected: number): void;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;

describe('KeyedQueue', () => {
	test('keeps one entry per key, the latest', () => {
		const queue = new KeyedQueue<string>();
		queue.set(0, 'a');
		queue.set(1, 'b');
		queue.set(0, 'c');
		expect(queue.drain()).toEqual(['c', 'b']);
	});

	test('is empty after a drain', () => {
		const queue = new KeyedQueue<string>();
		queue.set(2, 'x');
		queue.drain();
		expect(queue.drain()).toHaveLength(0);
	});
});

describe('Latest', () => {
	test('only the newest ticket is current', () => {
		const latest = new Latest();
		const first = latest.begin();
		const second = latest.begin();
		expect(latest.isCurrent(first)).toBe(false);
		expect(latest.isCurrent(second)).toBe(true);
	});
});

describe('nextBackoff', () => {
	test('starts at the minimum, doubles and caps', () => {
		expect(nextBackoff(0)).toBe(BACKOFF_MIN_MS);
		expect(nextBackoff(BACKOFF_MIN_MS)).toBe(BACKOFF_MIN_MS * 2);
		expect(nextBackoff(BACKOFF_MAX_MS)).toBe(BACKOFF_MAX_MS);
	});
});
