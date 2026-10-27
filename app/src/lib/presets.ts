/**
 * Built-in instrument preset roles and groupings.
 */

export type PresetRole = 'Synth' | 'Keys' | 'Bass' | 'Pad' | 'Drums' | 'Other';

export const PRESET_ROLES: readonly PresetRole[] = ['Synth', 'Keys', 'Bass', 'Pad', 'Drums', 'Other'];

export const PRESET_GROUPS: Readonly<Record<string, PresetRole>> = {
	Lead: 'Synth',
	Pluck: 'Synth',
	Supersaw: 'Synth',
	Acid: 'Synth',
	Keys: 'Keys',
	Bell: 'Keys',
	Organ: 'Keys',
	Bass: 'Bass',
	'Sub Bass': 'Bass',
	'808': 'Bass',
	Pad: 'Pad',
	Strings: 'Pad',
	Brass: 'Pad',
	Kick: 'Drums',
	Snare: 'Drums',
	Hat: 'Drums',
	Tom: 'Drums',
	Clap: 'Drums',
	Rim: 'Drums'
};

/** Look up which role a preset belongs to, or 'Other' when not in the catalog. */
export function roleForPreset(name: string): PresetRole {
	return PRESET_GROUPS[name] ?? 'Other';
}

export interface PresetGroup {
	role: PresetRole;
	presets: string[];
}

/** Group a list of preset names by their roles, omitting empty groups. */
export function groupPresets(presets: readonly string[]): PresetGroup[] {
	const map = new Map<PresetRole, string[]>();
	for (const role of PRESET_ROLES) {
		map.set(role, []);
	}
	for (const preset of presets) {
		const role = roleForPreset(preset);
		map.get(role)?.push(preset);
	}
	return PRESET_ROLES.map((role) => ({ role, presets: map.get(role) ?? [] })).filter(
		(group) => group.presets.length > 0
	);
}
