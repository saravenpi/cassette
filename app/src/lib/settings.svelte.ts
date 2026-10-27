export type SettingsTab = 'audio' | 'updates' | 'general';

const STORAGE_KEY_AUTO_UPDATE = 'cassette:autoUpdate';

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

export class SettingsStore {
	isOpen = $state(false);
	activeTab = $state<SettingsTab>('audio');
	autoUpdate = $state(loadAutoUpdate());

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
}

export const settingsStore = new SettingsStore();
export const settings = settingsStore;
