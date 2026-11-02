import { ARP_RATES, ARP_STYLES, DEFAULT_ARP, targetVelocity, withArp } from './arp';

interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;

describe('withArp', () => {
	test('applies a patch and keeps the rest', () => {
		const next = withArp(DEFAULT_ARP, { enabled: true, style: 'converge', rate: '1/8T' });
		expect(next).toEqual({ ...DEFAULT_ARP, enabled: true, style: 'converge', rate: '1/8T' });
	});

	test('holds every number inside the engine range', () => {
		const next = withArp(DEFAULT_ARP, {
			gate: 9,
			distance: -40.6,
			steps: 12,
			swing: -1,
			velocityDecay: 99,
			velocityTarget: 300
		});
		expect(next.gate).toBe(2);
		expect(next.distance).toBe(-24);
		expect(next.steps).toBe(8);
		expect(next.swing).toBe(0);
		expect(next.velocityDecay).toBe(16);
		expect(next.velocityTarget).toBe(127);
	});

	test('falls back to defaults for numbers that are not finite', () => {
		const next = withArp(DEFAULT_ARP, { gate: Number.NaN, steps: Number.POSITIVE_INFINITY });
		expect(next.gate).toBe(DEFAULT_ARP.gate);
		expect(next.steps).toBe(DEFAULT_ARP.steps);
	});
});

describe('menus', () => {
	test('list every style and rate the engine knows once', () => {
		expect(new Set(ARP_STYLES.map((style) => style.value)).size).toBe(8);
		expect(new Set(ARP_RATES).size).toBe(9);
	});
});

test('targetVelocity maps a dial share to a whole velocity', () => {
	expect(targetVelocity(0)).toBe(0);
	expect(targetVelocity(0.5)).toBe(64);
	expect(targetVelocity(1)).toBe(127);
	expect(targetVelocity(2)).toBe(127);
});
