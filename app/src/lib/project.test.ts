import {
	formatExportPercent,
	projectStem,
	projectTitle,
	resolveSaveAction,
	resolveSaveAsAction
} from './project';

interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void | Promise<void>) => void;
declare const expect: (actual: unknown) => Matchers;

describe('projectStem', () => {
	test('returns Untitled for null or empty path', () => {
		expect(projectStem(null)).toBe('Untitled');
		expect(projectStem('')).toBe('Untitled');
	});

	test('extracts stem from .cassette file', () => {
		expect(projectStem('/home/user/Music/my-song.cassette')).toBe('my-song');
		expect(projectStem('C:\\Projects\\beat.CASSETTE')).toBe('beat');
	});

	test('extracts stem from .json file', () => {
		expect(projectStem('/tmp/song.json')).toBe('song');
	});

	test('keeps filename without cassette or json extension intact', () => {
		expect(projectStem('/home/user/song')).toBe('song');
	});
});

describe('projectTitle', () => {
	test('shows stem when clean', () => {
		expect(projectTitle('/home/user/song.cassette', false)).toBe('song');
		expect(projectTitle(null, false)).toBe('Untitled');
	});

	test('appends a dot when dirty', () => {
		expect(projectTitle('/home/user/song.cassette', true)).toBe('song •');
		expect(projectTitle(null, true)).toBe('Untitled •');
	});
});

describe('resolveSaveAction', () => {
	test('invokes dialog when project path is null', async () => {
		let invoked = false;
		const stubPick = async () => {
			invoked = true;
			return '/tmp/new-song.cassette';
		};
		const action = await resolveSaveAction(null, stubPick);
		expect(invoked).toBe(true);
		expect(action).toEqual({ exportProject: { path: '/tmp/new-song.cassette' } });
	});

	test('returns null when dialog is cancelled', async () => {
		const stubPick = async () => null;
		const action = await resolveSaveAction(null, stubPick);
		expect(action).toBe(null);
	});

	test('does not invoke dialog when project path is already set', async () => {
		let invoked = false;
		const stubPick = async () => {
			invoked = true;
			return '/tmp/other.cassette';
		};
		const action = await resolveSaveAction('/existing/song.cassette', stubPick);
		expect(invoked).toBe(false);
		expect(action).toEqual({ exportProject: { path: null } });
	});
});

describe('resolveSaveAsAction', () => {
	test('always invokes dialog to pick a target path', async () => {
		let invoked = false;
		const stubPick = async () => {
			invoked = true;
			return '/tmp/save-as.cassette';
		};
		const action = await resolveSaveAsAction(stubPick);
		expect(invoked).toBe(true);
		expect(action).toEqual({ exportProject: { path: '/tmp/save-as.cassette' } });
	});

	test('returns null when save as dialog is cancelled', async () => {
		const stubPick = async () => null;
		const action = await resolveSaveAsAction(stubPick);
		expect(action).toBe(null);
	});
});

describe('formatExportPercent', () => {
	test('returns null for null or undefined', () => {
		expect(formatExportPercent(null)).toBe(null);
		expect(formatExportPercent(undefined)).toBe(null);
	});

	test('rounds progress ratio to percentage integer', () => {
		expect(formatExportPercent(0)).toBe(0);
		expect(formatExportPercent(0.456)).toBe(46);
		expect(formatExportPercent(1.0)).toBe(100);
	});
});
