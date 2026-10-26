import { AUDIO_EXTS, dropStarts } from './audio';

interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;

describe('dropStarts', () => {
	test('offsets each file one step after the previous', () => {
		expect(dropStarts(8, 3, 4)).toEqual([8, 12, 16]);
	});

	test('gives one file the start untouched', () => {
		expect(dropStarts(0, 1)).toEqual([0]);
	});
});

describe('AUDIO_EXTS', () => {
	test('matches audio, case-insensitively, and not projects', () => {
		expect(AUDIO_EXTS.test('/a/Kick.WAV')).toBe(true);
		expect(AUDIO_EXTS.test('/a/song.json')).toBe(false);
	});
});
