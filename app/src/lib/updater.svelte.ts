import { isTauri } from '@tauri-apps/api/core';
import { check, type DownloadEvent, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import pkg from '../../package.json';

export type UpdaterState =
	| 'idle'
	| 'checking'
	| 'available'
	| 'downloading'
	| 'ready'
	| 'error'
	| 'upToDate';

export class UpdaterStore {
	state = $state<UpdaterState>('idle');
	currentVersion = $state<string>(pkg.version);
	availableVersion = $state<string | null>(null);
	downloadProgress = $state<number>(0);
	errorMessage = $state<string | null>(null);

	private updateHandle: Update | null = null;
	private downloadedBytes = 0;
	private totalBytes = 0;

	checkNow = async (auto = false): Promise<void> => {
		if (this.state === 'checking' || this.state === 'downloading') return;
		this.errorMessage = null;
		if (!auto) {
			this.state = 'checking';
		}

		if (!isTauri()) {
			if (!auto) {
				this.state = 'upToDate';
			}
			return;
		}

		try {
			const update = await check();
			if (update) {
				this.updateHandle = update;
				this.currentVersion = update.currentVersion || this.currentVersion;
				this.availableVersion = update.version;
				this.state = 'available';
			} else {
				this.updateHandle = null;
				this.availableVersion = null;
				this.state = 'upToDate';
			}
		} catch (err) {
			const msg = err instanceof Error ? err.message : String(err);
			if (!auto) {
				this.errorMessage = msg;
				this.state = 'error';
			} else {
				this.state = 'idle';
			}
		}
	};

	installNow = async (): Promise<void> => {
		if (this.state === 'downloading' || this.state === 'ready') return;

		if (!isTauri()) {
			this.state = 'downloading';
			this.downloadProgress = 100;
			this.state = 'ready';
			return;
		}

		if (!this.updateHandle) {
			this.errorMessage = 'No update available to install';
			this.state = 'error';
			return;
		}

		this.state = 'downloading';
		this.downloadProgress = 0;
		this.downloadedBytes = 0;
		this.totalBytes = 0;

		try {
			await this.updateHandle.downloadAndInstall((event: DownloadEvent) => {
				if (event.event === 'Started') {
					this.totalBytes = event.data.contentLength ?? 0;
					this.downloadedBytes = 0;
					this.downloadProgress = 0;
				} else if (event.event === 'Progress') {
					this.downloadedBytes += event.data.chunkLength;
					if (this.totalBytes > 0) {
						this.downloadProgress = Math.min(
							99,
							Math.round((this.downloadedBytes / this.totalBytes) * 100)
						);
					}
				} else if (event.event === 'Finished') {
					this.downloadProgress = 100;
				}
			});
			this.downloadProgress = 100;
			this.state = 'ready';
		} catch (err) {
			this.errorMessage = err instanceof Error ? err.message : String(err);
			this.state = 'error';
		}
	};

	restartNow = async (): Promise<void> => {
		if (!isTauri()) {
			if (typeof window !== 'undefined') {
				window.location.reload();
			}
			return;
		}
		try {
			await relaunch();
		} catch (err) {
			this.errorMessage = err instanceof Error ? err.message : String(err);
			this.state = 'error';
		}
	};
}

export const updaterStore = new UpdaterStore();
export const updater = updaterStore;
