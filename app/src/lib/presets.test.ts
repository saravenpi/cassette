import { PRESET_GROUPS, groupPresets, roleForPreset } from './presets';

interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	not: Matchers;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;

const STUBBED_SNAPSHOT_PRESETS = [
	'Lead',
	'Bass',
	'Pad',
	'Pluck',
	'Kick',
	'Snare',
	'Hat',
	'Supersaw',
	'Sub Bass',
	'Acid',
	'Keys',
	'Bell',
	'Organ',
	'Strings',
	'Brass',
	'808',
	'Tom',
	'Clap',
	'Rim',
	'Pulse Lead',
	'Reese',
	'EPiano',
	'Clav',
	'Marimba',
	'Choir'
];

describe('roleForPreset', () => {
	test('every preset in the snapshot has a mapped group', () => {
		for (const preset of STUBBED_SNAPSHOT_PRESETS) {
			const role = roleForPreset(preset);
			expect(role).not.toBe('Other');
			expect(PRESET_GROUPS[preset]).toBe(role);
		}
	});

	test('unknown presets map to Other', () => {
		expect(roleForPreset('CustomModular')).toBe('Other');
	});
});

describe('groupPresets', () => {
	test('groups presets into their roles in role order', () => {
		const groups = groupPresets(['Lead', 'Keys', 'Kick', 'Unknown']);
		expect(groups).toEqual([
			{ role: 'Synth', presets: ['Lead'] },
			{ role: 'Keys', presets: ['Keys'] },
			{ role: 'Drums', presets: ['Kick'] },
			{ role: 'Other', presets: ['Unknown'] }
		]);
	});

	test('omits groups that contain no presets', () => {
		const groups = groupPresets(['Lead', 'Brass']);
		expect(groups).toEqual([{ role: 'Synth', presets: ['Lead', 'Brass'] }]);
	});
});
