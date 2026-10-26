import { AUDIO_EXTS } from './audio';

interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;

describe('AUDIO_EXTS', () => {
	test('matches audio, case-insensitively, and not projects', () => {
		expect(AUDIO_EXTS.test('/a/Kick.WAV')).toBe(true);
		expect(AUDIO_EXTS.test('/a/song.json')).toBe(false);
	});
});
