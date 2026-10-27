import { addableKinds, dropIndex, indicatorX, moveTarget, paramKey, specFor } from './chain';
import type { EffectKind, EffectSpecView, EffectView } from './wire';

// bun test injects these as globals; @types/bun is not a dependency, so svelte-check
// needs the shapes declared here to see them.
interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	toBeNull(): void;
	not: Matchers;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;

const KINDS: EffectKind[] = ['eq', 'filter', 'saturator', 'compressor', 'delay', 'reverb'];
const CATALOG: EffectSpecView[] = KINDS.map((kind) => ({ kind, name: kind, takesSidechain: false, params: [] }));

function effect(kind: EffectKind): EffectView {
	return { id: 0, kind, name: kind, enabled: true, params: [], sidechain: null };
}

const SPANS = [
	{ left: 100, width: 100 },
	{ left: 206, width: 50 },
	{ left: 262, width: 100 }
];

describe('addableKinds', () => {
	test('offers every kind to an empty chain, in catalog order', () => {
		expect(addableKinds(CATALOG, []).map((spec) => spec.kind)).toEqual(KINDS);
	});

	test('leaves out the kinds the chain holds', () => {
		const left = addableKinds(CATALOG, [effect('delay'), effect('eq')]).map((spec) => spec.kind);
		expect(left).toEqual(['filter', 'saturator', 'compressor', 'reverb']);
	});

	test('offers nothing once every kind is on the track', () => {
		expect(addableKinds(CATALOG, KINDS.map(effect))).toEqual([]);
	});
});

describe('dropIndex', () => {
	test('counts the cards whose middle is left of the pointer', () => {
		expect(dropIndex(0, SPANS)).toBe(0);
		expect(dropIndex(149, SPANS)).toBe(0);
		expect(dropIndex(151, SPANS)).toBe(1);
		expect(dropIndex(240, SPANS)).toBe(2);
		expect(dropIndex(320, SPANS)).toBe(3);
		expect(dropIndex(900, SPANS)).toBe(3);
	});

	test('drops into slot zero with no cards', () => {
		expect(dropIndex(500, [])).toBe(0);
	});
});

describe('moveTarget', () => {
	test('lands one lower when dropped right of the lifted card', () => {
		expect(moveTarget(0, 3)).toBe(2);
		expect(moveTarget(0, 2)).toBe(1);
	});

	test('lands at the slot when dropped left of it', () => {
		expect(moveTarget(2, 0)).toBe(0);
		expect(moveTarget(2, 1)).toBe(1);
	});

	test('is null when the card would not move', () => {
		expect(moveTarget(1, 1)).toBeNull();
		expect(moveTarget(1, 2)).toBeNull();
	});
});

describe('indicatorX', () => {
	test('sits in the gap before a card, or after the last one', () => {
		expect(indicatorX(0, SPANS, 6)).toBe(97);
		expect(indicatorX(1, SPANS, 6)).toBe(203);
		expect(indicatorX(3, SPANS, 6)).toBe(365);
	});

	test('is null with no cards', () => {
		expect(indicatorX(0, [], 6)).toBeNull();
	});
});

describe('paramKey and specFor', () => {
	test('gives every parameter of every effect of every track its own key', () => {
		const keys = new Set<number>();
		for (let track = 0; track < 4; track += 1) {
			for (let index = 0; index < 6; index += 1) {
				for (let param = 0; param < 5; param += 1) keys.add(paramKey(track, index, param));
			}
		}
		expect(keys.size).toBe(4 * 6 * 5);
	});

	test('finds a kind in the catalog, or null', () => {
		expect(specFor(CATALOG, 'reverb')?.kind).toBe('reverb');
		expect(specFor([], 'reverb')).toBeNull();
	});
});
