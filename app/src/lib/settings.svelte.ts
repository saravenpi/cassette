import { sendNow } from './api';

export type SettingsTab = 'audio' | 'updates' | 'general';

const STORAGE_KEY_AUTO_UPDATE = 'cassette:autoUpdate';
const STORAGE_KEY_OUTPUT_DEVICE = 'cassette:outputDevice';
const STORAGE_KEY_INPUT_DEVICE = 'cassette:inputDevice';

function loadAutoUpdate(): boolean {
	try {
		const val = globalThis.localStorage?.getItem(STORAGE_KEY_AUTO_UPDATE);
		return val === 'false' ? false : true;
	} catch {
		return true;
	}
}

function saveAutoUpdate(val: boolean): void {
	try {
		globalThis.localStorage?.setItem(STORAGE_KEY_AUTO_UPDATE, String(val));
	} catch {
		return;
	}
}

function loadString(key: string, fallback: string): string {
	try {
		return globalThis.localStorage?.getItem(key) || fallback;
	} catch {
		return fallback;
	}
}

function saveString(key: string, val: string): void {
	try {
		globalThis.localStorage?.setItem(key, val);
	} catch {
		return;
	}
}

export class SettingsStore {
	isOpen = $state(false);
	activeTab = $state<SettingsTab>('audio');
	autoUpdate = $state(loadAutoUpdate());
	outputDevice = $state(loadString(STORAGE_KEY_OUTPUT_DEVICE, 'default'));
	inputDevice = $state(loadString(STORAGE_KEY_INPUT_DEVICE, 'default'));

	get open(): boolean {
		return this.isOpen;
	}

	set open(val: boolean) {
		this.isOpen = val;
	}

	show = (tab?: SettingsTab | string): void => {
		if (tab === 'audio' || tab === 'updates' || tab === 'general') {
			this.activeTab = tab as SettingsTab;
		}
		this.isOpen = true;
	};

	close = (): void => {
		this.isOpen = false;
	};

	toggle = (): void => {
		this.isOpen = !this.isOpen;
	};

	setOpen = (val: boolean): void => {
		this.isOpen = val;
	};

	setAutoUpdate = (enabled: boolean): void => {
		this.autoUpdate = enabled;
		saveAutoUpdate(enabled);
	};

	setOutputDevice = (device: string): void => {
		this.outputDevice = device;
		saveString(STORAGE_KEY_OUTPUT_DEVICE, device);
		sendNow({ setOutputDevice: { device: device === 'default' ? null : device } });
	};

	setInputDevice = (device: string): void => {
		this.inputDevice = device;
		saveString(STORAGE_KEY_INPUT_DEVICE, device);
	};
}

export const settingsStore = new SettingsStore();
export const settings = settingsStore;
