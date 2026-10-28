<script lang="ts">
	import { getAudioDevices } from '$lib/api';
	import { session } from '$lib/session.svelte';
	import { settingsStore, type SettingsTab } from '$lib/settings.svelte';
	import { updaterStore } from '$lib/updater.svelte';

	const TABS: { id: SettingsTab; label: string }[] = [
		{ id: 'audio', label: 'Audio' },
		{ id: 'updates', label: 'Updates' },
		{ id: 'general', label: 'Shortcuts' }
	];

	let outputs = $state<string[]>([]);
	let inputs = $state<string[]>([]);
	let defaultOutput = $state<string | null>(null);
	let defaultInput = $state<string | null>(null);
	let loadingDevices = $state(false);
	let inputMonitoring = $state(false);

	async function refreshDevices(): Promise<void> {
		loadingDevices = true;
		try {
			const list = await getAudioDevices();
			outputs = list.outputs;
			inputs = list.inputs;
			defaultOutput = list.defaultOutput;
			defaultInput = list.defaultInput;
		} finally {
			loadingDevices = false;
		}
	}

	$effect(() => {
		if (settingsStore.isOpen) {
			void refreshDevices();
		}
	});

	function onKeyDown(event: KeyboardEvent): void {
		if (settingsStore.isOpen && event.key === 'Escape') {
			event.stopPropagation();
			settingsStore.close();
		}
	}

	function onBackdropClick(event: MouseEvent): void {
		if (event.target === event.currentTarget) {
			settingsStore.close();
		}
	}
</script>

<svelte:window onkeydown={onKeyDown} />

{#if settingsStore.isOpen}
	<div
		class="overlay"
		role="dialog"
		aria-modal="true"
		aria-label="Settings"
		tabindex="-1"
		onclick={onBackdropClick}
		onkeydown={onKeyDown}
	>
		<div class="modal">
			<div class="modal-body">
				<nav class="sidebar" aria-label="Settings categories">
					{#each TABS as tab (tab.id)}
						<button
							type="button"
							class="tab-btn"
							class:active={settingsStore.activeTab === tab.id}
							onclick={() => (settingsStore.activeTab = tab.id)}
						>
							{tab.label}
						</button>
					{/each}
					<button
						type="button"
						class="close-btn"
						aria-label="Close settings"
						onclick={() => settingsStore.close()}
					>
						Close [Esc]
					</button>
				</nav>

				<main class="content">
					{#if settingsStore.activeTab === 'audio'}
						<section class="section">
							<div class="section-head">
								<h3 class="section-title">Audio Devices</h3>
								<button
									type="button"
									class="secondary-btn"
									disabled={loadingDevices}
									onclick={() => void refreshDevices()}
								>
									{loadingDevices ? 'Scanning...' : 'Refresh'}
								</button>
							</div>

							<div class="device-grid">
								<label class="device-card">
									<span class="device-label">Output Device</span>
									<span class="desc">Playback destination</span>
									<select
										class="select"
										aria-label="Audio output device"
										value={settingsStore.outputDevice}
										onchange={(e) => settingsStore.setOutputDevice(e.currentTarget.value)}
									>
										<option value="default">
											System Default{defaultOutput ? ` (${defaultOutput})` : ''}
										</option>
										{#each outputs as dev (dev)}
											<option value={dev}>{dev}</option>
										{/each}
									</select>
								</label>

								<label class="device-card">
									<span class="device-label">Input Device</span>
									<span class="desc">Audio track recording source</span>
									<select
										class="select"
										aria-label="Audio input device"
										value={settingsStore.inputDevice}
										onchange={(e) => settingsStore.setInputDevice(e.currentTarget.value)}
									>
										<option value="default">
											System Default{defaultInput ? ` (${defaultInput})` : ''}
										</option>
										{#each inputs as dev (dev)}
											<option value={dev}>{dev}</option>
										{/each}
									</select>
								</label>
							</div>

							<div class="row">
								<div class="label-group">
									<span class="label">Driver & Status</span>
									<span class="desc">{session.snapshot?.backendNote || 'Active audio engine'}</span>
								</div>
								<div class="value-group">
									<span class="status-square"></span>
									<span class="mono-value">{session.snapshot?.backend ?? 'cpal'}</span>
								</div>
							</div>

							<div class="row">
								<div class="label-group">
									<span class="label">Sample Rate</span>
									<span class="desc">Engine processing rate</span>
								</div>
								<div class="value-group">
									<span class="mono-value">{session.snapshot?.sampleRate ?? 44100} Hz</span>
								</div>
							</div>

							<div class="row">
								<div class="label-group">
									<span class="label">Input Monitoring</span>
									<span class="desc">Pass input signal through armed audio tracks</span>
								</div>
								<div class="value-group">
									<button
										type="button"
										class="toggle-btn"
										class:active={inputMonitoring}
										aria-pressed={inputMonitoring}
										onclick={() => (inputMonitoring = !inputMonitoring)}
									>
										{inputMonitoring ? 'ON' : 'OFF'}
									</button>
								</div>
							</div>
						</section>
					{:else if settingsStore.activeTab === 'updates'}
						<section class="section">
							<h3 class="section-title">Software Updates</h3>

							<div class="row">
								<div class="label-group">
									<span class="label">Installed Version</span>
									<span class="desc">Current release build</span>
								</div>
								<div class="value-group">
									<span class="mono-value">v{updaterStore.currentVersion}</span>
								</div>
							</div>

							<div class="row">
								<div class="label-group">
									<span class="label">Automatic Updates</span>
									<span class="desc">Check for new releases on startup</span>
								</div>
								<div class="value-group">
									<button
										type="button"
										class="toggle-btn"
										class:active={settingsStore.autoUpdate}
										aria-pressed={settingsStore.autoUpdate}
										onclick={() => settingsStore.setAutoUpdate(!settingsStore.autoUpdate)}
									>
										{settingsStore.autoUpdate ? 'ON' : 'OFF'}
									</button>
								</div>
							</div>

							<div class="update-action-box">
								<div class="status-message">
									{#if updaterStore.state === 'checking'}
										<span class="badge checking">Checking for updates...</span>
									{:else if updaterStore.state === 'available'}
										<span class="badge available">
											Version v{updaterStore.availableVersion} is available
										</span>
									{:else if updaterStore.state === 'downloading'}
										<div class="download-box">
											<div class="progress-bar-bg">
												<div
													class="progress-bar-fill"
													style="width: {updaterStore.downloadProgress}%"
												></div>
											</div>
											<span class="progress-text">
												Downloading... {updaterStore.downloadProgress}%
											</span>
										</div>
									{:else if updaterStore.state === 'ready'}
										<span class="badge ready">Update downloaded and ready to install</span>
									{:else if updaterStore.state === 'error'}
										<span class="badge error">
											{updaterStore.errorMessage || 'Update check failed'}
										</span>
									{:else if updaterStore.state === 'upToDate'}
										<span class="badge uptodate">Cassette is up to date</span>
									{:else}
										<span class="subtle-text">Ready to check for new versions</span>
									{/if}
								</div>

								<div class="button-row">
									{#if updaterStore.state === 'available'}
										<button
											type="button"
											class="primary-btn"
											onclick={() => updaterStore.installNow()}
										>
											Download & Install
										</button>
									{:else if updaterStore.state === 'ready'}
										<button
											type="button"
											class="primary-btn"
											onclick={() => updaterStore.restartNow()}
										>
											Install and Restart
										</button>
									{:else}
										<button
											type="button"
											class="secondary-btn"
											disabled={updaterStore.state === 'checking' ||
												updaterStore.state === 'downloading'}
											onclick={() => updaterStore.checkNow(false)}
										>
											{updaterStore.state === 'checking' ? 'Checking...' : 'Check for updates'}
										</button>
									{/if}
								</div>
							</div>

							<div class="row release-notes-row">
								<div class="label-group">
									<span class="label">Release Notes</span>
									<span class="desc">View changelog and release history</span>
								</div>
								<div class="value-group">
									<a
										class="link"
										href="https://github.com/Saravenpi/cassette/releases"
										target="_blank"
										rel="noreferrer"
									>
										github.com/Saravenpi/cassette/releases
									</a>
								</div>
							</div>
						</section>
					{:else if settingsStore.activeTab === 'general'}
						<section class="section">
							<h3 class="section-title">Keyboard Shortcuts</h3>

							<div class="table-wrapper">
								<table class="shortcuts-table">
									<thead>
										<tr>
											<th>Shortcut</th>
											<th>Action</th>
										</tr>
									</thead>
									<tbody>
										<tr>
											<td><kbd>Space</kbd></td>
											<td>Play / Stop transport</td>
										</tr>
										<tr>
											<td><kbd>F9</kbd></td>
											<td>Record into armed/selected audio or MIDI track</td>
										</tr>
										<tr>
											<td><kbd>P</kbd></td>
											<td>Open clip view</td>
										</tr>
										<tr>
											<td><kbd>L</kbd></td>
											<td>Toggle loop mode</td>
										</tr>
										<tr>
											<td><kbd>Tab</kbd></td>
											<td>Toggle Timeline / Mixer view</td>
										</tr>
										<tr>
											<td><kbd>Shift</kbd> + <kbd>Tab</kbd></td>
											<td>Switch Clip / Devices detail view</td>
										</tr>
										<tr>
											<td><kbd>Ctrl</kbd> + <kbd>U</kbd></td>
											<td>Quantize notes to active grid</td>
										</tr>
										<tr>
											<td><kbd>Ctrl</kbd> + <kbd>T</kbd></td>
											<td>Add Audio Track (Shift for MIDI)</td>
										</tr>
										<tr>
											<td><kbd>Ctrl</kbd> + <kbd>D</kbd></td>
											<td>Duplicate selected block</td>
										</tr>
										<tr>
											<td><kbd>Ctrl</kbd> + <kbd>E</kbd></td>
											<td>Split block at playhead</td>
										</tr>
										<tr>
											<td><kbd>B</kbd></td>
											<td>Split block at pointer</td>
										</tr>
										<tr>
											<td><kbd>Enter</kbd> / <kbd>N</kbd></td>
											<td>Add MIDI block at playhead</td>
										</tr>
										<tr>
											<td><kbd>Delete</kbd></td>
											<td>Delete selected block or notes</td>
										</tr>
										<tr>
											<td><kbd>M</kbd> / <kbd>O</kbd></td>
											<td>Toggle track mute / solo</td>
										</tr>
										<tr>
											<td><kbd>Ctrl</kbd> + <kbd>Z</kbd></td>
											<td>Undo (Shift to redo)</td>
										</tr>
										<tr>
											<td><kbd>Ctrl</kbd> + <kbd>S</kbd></td>
											<td>Save project (Shift for Save As)</td>
										</tr>
										<tr>
											<td><kbd>Esc</kbd></td>
											<td>Close Clip / Modal</td>
										</tr>
									</tbody>
								</table>
							</div>

							<div class="row about-row">
								<div class="label-group">
									<span class="label">About Cassette</span>
									<span class="desc">Version {updaterStore.currentVersion} • MIT License</span>
								</div>
								<div class="value-group">
									<a
										class="link"
										href="https://github.com/Saravenpi/cassette"
										target="_blank"
										rel="noreferrer"
									>
										github.com/Saravenpi/cassette
									</a>
								</div>
							</div>
						</section>
					{/if}
				</main>
			</div>
		</div>
	</div>
{/if}

<style>
	.overlay {
		position: fixed;
		inset: 0;
		z-index: 1000;
		background: rgba(0, 0, 0, 0.8);
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 24px;
	}

	.modal {
		width: 680px;
		max-width: 95vw;
		height: 480px;
		max-height: 85vh;
		background: var(--bg);
		border: 1px solid var(--border);
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.modal-body {
		display: flex;
		flex: 1;
		min-height: 0;
	}

	.sidebar {
		width: 148px;
		flex: none;
		background: var(--surface);
		border-right: 1px solid var(--border-subtle);
		display: flex;
		flex-direction: column;
	}

	.tab-btn {
		width: 100%;
		text-align: left;
		padding: 12px 14px;
		font-size: 11px;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		background: transparent;
		border: none;
		border-bottom: 1px solid var(--border-subtle);
		border-left: 2px solid transparent;
		color: var(--text-subtle);
		cursor: pointer;
	}

	.tab-btn:hover {
		color: var(--text);
		background: var(--surface-hover);
	}

	.tab-btn.active {
		color: var(--text);
		background: var(--bg);
		border-left-color: var(--green);
	}

	.close-btn {
		margin-top: auto;
		width: 100%;
		text-align: left;
		padding: 12px 14px;
		font-size: 10px;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		background: transparent;
		border: none;
		border-top: 1px solid var(--border-subtle);
		color: var(--text-subtle);
		cursor: pointer;
	}

	.close-btn:hover {
		color: var(--text);
		background: var(--surface-hover);
	}

	.content {
		flex: 1;
		min-width: 0;
		min-height: 0;
		overflow-y: auto;
		padding: 20px 24px;
	}

	.section {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	.section-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.section-title {
		margin: 0;
		font-size: 11px;
		font-weight: normal;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--text-subtle);
	}

	.device-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 12px;
	}

	.device-card {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 12px;
		background: var(--surface);
		border: 1px solid var(--border-subtle);
	}

	.device-label {
		font-size: 11px;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--text);
	}

	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 0;
		border-bottom: 1px solid var(--border-subtle);
	}

	.label-group {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.label {
		font-size: 12px;
		color: var(--text);
	}

	.desc {
		font-size: 11px;
		color: var(--text-subtle);
	}

	.value-group {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.status-square {
		width: 7px;
		height: 7px;
		background: var(--green);
	}

	.mono-value {
		font-family: inherit;
		font-size: 12px;
		color: var(--text);
	}

	.select {
		width: 100%;
		height: 28px;
		background: var(--bg);
		border: 1px solid var(--border);
		color: var(--text);
		font-size: 11px;
		padding: 0 8px;
		outline: none;
	}

	.select:focus-visible {
		border-color: var(--green);
	}

	.toggle-btn {
		min-width: 48px;
		height: 24px;
		padding: 0 10px;
		font-size: 10px;
		letter-spacing: 0.12em;
		background: var(--surface);
		border: 1px solid var(--border);
		color: var(--text-subtle);
		cursor: pointer;
	}

	.toggle-btn.active {
		background: var(--green);
		border-color: var(--green);
		color: var(--on-green);
	}

	.update-action-box {
		padding: 14px;
		background: var(--surface);
		border: 1px solid var(--border-subtle);
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.status-message {
		font-size: 12px;
	}

	.badge {
		display: inline-block;
		font-size: 11px;
		padding: 3px 8px;
	}

	.badge.checking {
		background: var(--surface-hover);
		color: var(--text-subtle);
	}

	.badge.available {
		background: var(--olive-soft);
		color: var(--olive);
	}

	.badge.ready,
	.badge.uptodate {
		background: var(--green-soft);
		color: var(--green);
	}

	.badge.error {
		background: var(--alert-soft);
		color: var(--alert);
	}

	.subtle-text {
		color: var(--text-subtle);
	}

	.button-row {
		display: flex;
		gap: 8px;
	}

	.primary-btn {
		background: var(--green);
		color: var(--on-green);
		font-size: 11px;
		padding: 6px 14px;
		border: 1px solid var(--green);
		cursor: pointer;
		text-transform: uppercase;
		letter-spacing: 0.08em;
	}

	.secondary-btn {
		background: var(--surface);
		color: var(--text);
		border: 1px solid var(--border);
		font-size: 10px;
		padding: 5px 12px;
		cursor: pointer;
		text-transform: uppercase;
		letter-spacing: 0.08em;
	}

	.secondary-btn:hover:not(:disabled) {
		border-color: var(--green);
		color: var(--text);
	}

	.secondary-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.download-box {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.progress-bar-bg {
		width: 100%;
		height: 6px;
		background: var(--bg);
		border: 1px solid var(--border-subtle);
		overflow: hidden;
	}

	.progress-bar-fill {
		height: 100%;
		background: var(--green);
	}

	.progress-text {
		font-size: 11px;
		color: var(--text-subtle);
	}

	.link {
		color: var(--green);
		font-size: 11px;
		text-decoration: none;
	}

	.link:hover {
		text-decoration: underline;
	}

	.table-wrapper {
		max-height: 290px;
		overflow-y: auto;
		border: 1px solid var(--border-subtle);
	}

	.shortcuts-table {
		width: 100%;
		border-collapse: collapse;
		font-size: 11px;
	}

	.shortcuts-table th,
	.shortcuts-table td {
		padding: 6px 10px;
		text-align: left;
		border-bottom: 1px solid var(--border-subtle);
	}

	.shortcuts-table th {
		background: var(--surface);
		color: var(--text-subtle);
		font-weight: normal;
		text-transform: uppercase;
		letter-spacing: 0.08em;
	}

	.shortcuts-table td {
		color: var(--text);
	}

	kbd {
		background: var(--surface);
		border: 1px solid var(--border);
		padding: 1px 5px;
		font-size: 10px;
		font-family: inherit;
		color: var(--silver);
	}

	.about-row {
		border-top: 1px solid var(--border-subtle);
		border-bottom: none;
		margin-top: 4px;
	}
</style>
