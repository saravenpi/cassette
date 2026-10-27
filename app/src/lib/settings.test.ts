export {};

interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => Matchers;
declare const beforeEach: (fn: () => void) => void;

if (typeof (globalThis as Record<string, unknown>).$state === 'undefined') {
	(globalThis as Record<string, unknown>).$state = <T>(value: T) => value;
}

const { SettingsStore } = await import('./settings.svelte');
const { UpdaterStore } = await import('./updater.svelte');

beforeEach(() => {
	globalThis.localStorage?.clear?.();
});

describe('SettingsStore', () => {
	test('initializes with default state', () => {
		const store = new SettingsStore();
		expect(store.isOpen).toBe(false);
		expect(store.open).toBe(false);
		expect(store.activeTab).toBe('audio');
		expect(store.autoUpdate).toBe(true);
	});

	test('opens and selects specified tab', () => {
		const store = new SettingsStore();
		store.show('updates');
		expect(store.isOpen).toBe(true);
		expect(store.open).toBe(true);
		expect(store.activeTab).toBe('updates');
	});

	test('closes settings', () => {
		const store = new SettingsStore();
		store.show();
		expect(store.isOpen).toBe(true);
		store.close();
		expect(store.isOpen).toBe(false);
		expect(store.open).toBe(false);
	});

	test('toggles open state', () => {
		const store = new SettingsStore();
		store.toggle();
		expect(store.isOpen).toBe(true);
		store.toggle();
		expect(store.isOpen).toBe(false);
	});

	test('updates auto-update setting', () => {
		const store = new SettingsStore();
		store.setAutoUpdate(false);
		expect(store.autoUpdate).toBe(false);
		store.setAutoUpdate(true);
		expect(store.autoUpdate).toBe(true);
	});
});

describe('UpdaterStore', () => {
	test('initializes with idle state and current version', () => {
		const store = new UpdaterStore();
		expect(store.state).toBe('idle');
		expect(store.currentVersion).toBe('0.16.3');
		expect(store.availableVersion).toBe(null);
		expect(store.downloadProgress).toBe(0);
		expect(store.errorMessage).toBe(null);
	});

	test('checkNow falls back to upToDate outside Tauri', async () => {
		const store = new UpdaterStore();
		await store.checkNow(false);
		expect(store.state).toBe('upToDate');
	});

	test('auto checkNow stays idle outside Tauri', async () => {
		const store = new UpdaterStore();
		await store.checkNow(true);
		expect(store.state).toBe('idle');
	});

	test('installNow falls back to ready outside Tauri', async () => {
		const store = new UpdaterStore();
		await store.installNow();
		expect(store.state).toBe('ready');
		expect(store.downloadProgress).toBe(100);
	});
});
