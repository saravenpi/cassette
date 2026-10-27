<script lang="ts">
	import { session } from '$lib/session.svelte';
	import { settingsStore, type SettingsTab } from '$lib/settings.svelte';
	import { updaterStore } from '$lib/updater.svelte';

	const TABS: { id: SettingsTab; label: string }[] = [
		{ id: 'audio', label: 'Audio' },
		{ id: 'updates', label: 'Updates' },
		{ id: 'general', label: 'General' }
	];

	let inputMonitoring = $state(false);

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
		aria-labelledby="settings-title"
		tabindex="-1"
		onclick={onBackdropClick}
		onkeydown={onKeyDown}
	>
		<div class="modal">
			<header class="modal-header">
				<h2 id="settings-title" class="title">Settings</h2>
				<button
					class="close-btn"
					aria-label="Close settings"
					onclick={() => settingsStore.close()}
				>
					✕
				</button>
			</header>

			<div class="modal-body">
				<nav class="sidebar" aria-label="Settings categories">
					{#each TABS as tab (tab.id)}
						<button
							class="tab-btn"
							class:active={settingsStore.activeTab === tab.id}
							onclick={() => (settingsStore.activeTab = tab.id)}
						>
							{tab.label}
						</button>
					{/each}
				</nav>

				<main class="content">
					{#if settingsStore.activeTab === 'audio'}
						<section class="section">
							<h3 class="section-title">Audio Engine</h3>

							<div class="row">
								<div class="label-group">
									<span class="label">Backend</span>
									<span class="desc">Active audio device driver</span>
								</div>
								<div class="value-group">
									<span class="status-dot"></span>
									<span class="mono-value">{session.snapshot?.backend ?? 'cpal'}</span>
								</div>
							</div>

							<div class="row">
								<div class="label-group">
									<span class="label">Output Device</span>
									<span class="desc">System audio destination</span>
								</div>
								<div class="value-group">
									<select class="select" aria-label="Audio output device">
										<option value="default" selected>
											{session.snapshot?.backendNote || 'System Default Device'}
										</option>
									</select>
								</div>
							</div>

							<div class="row">
								<div class="label-group">
									<span class="label">Sample Rate</span>
									<span class="desc">Audio engine processing rate</span>
								</div>
								<div class="value-group">
									<span class="mono-value">{session.snapshot?.sampleRate ?? 44100} Hz</span>
								</div>
							</div>

							<div class="row">
								<div class="label-group">
									<span class="label">Latency & Buffer</span>
									<span class="desc">I/O buffer period</span>
								</div>
								<div class="value-group">
									<span class="mono-value">256 samples (~5.8 ms)</span>
								</div>
							</div>

							<div class="row">
								<div class="label-group">
									<span class="label">Input Monitoring</span>
									<span class="desc">Pass active input signal through track channels</span>
								</div>
								<div class="value-group">
									<label class="switch">
										<input
											type="checkbox"
											checked={inputMonitoring}
											onchange={(e) => (inputMonitoring = e.currentTarget.checked)}
										/>
										<span class="slider"></span>
									</label>
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
									<label class="switch">
										<input
											type="checkbox"
											checked={settingsStore.autoUpdate}
											onchange={(e) => settingsStore.setAutoUpdate(e.currentTarget.checked)}
										/>
										<span class="slider"></span>
									</label>
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
											class="primary-btn"
											onclick={() => updaterStore.installNow()}
										>
											Download & Install
										</button>
									{:else if updaterStore.state === 'ready'}
										<button
											class="primary-btn"
											onclick={() => updaterStore.restartNow()}
										>
											Install and Restart
										</button>
									{:else}
										<button
											class="secondary-btn"
											disabled={updaterStore.state === 'checking' || updaterStore.state === 'downloading'}
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
										github.com/Saravenpi/cassette/releases ↗
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
											<td><kbd>P</kbd></td>
											<td>Open Piano Roll</td>
										</tr>
										<tr>
											<td><kbd>L</kbd></td>
											<td>Toggle loop mode</td>
										</tr>
										<tr>
											<td><kbd>Tab</kbd></td>
											<td>Toggle Mixer / Devices detail view</td>
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
											<td>Add block to active track</td>
										</tr>
										<tr>
											<td><kbd>Delete</kbd></td>
											<td>Delete selected block</td>
										</tr>
										<tr>
											<td><kbd>M</kbd></td>
											<td>Toggle track mute</td>
										</tr>
										<tr>
											<td><kbd>O</kbd></td>
											<td>Toggle track solo</td>
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
											<td><kbd>Z</kbd> / <kbd>X</kbd></td>
											<td>Octave transpose down / up</td>
										</tr>
										<tr>
											<td><kbd>-</kbd> / <kbd>=</kbd></td>
											<td>Master volume adjust</td>
										</tr>
										<tr>
											<td><kbd>[</kbd> / <kbd>]</kbd></td>
											<td>Track pan adjust</td>
										</tr>
										<tr>
											<td><kbd>,</kbd> / <kbd>.</kbd></td>
											<td>BPM tempo adjust</td>
										</tr>
										<tr>
											<td><kbd>F9</kbd></td>
											<td>Toggle recording</td>
										</tr>
										<tr>
											<td><kbd>Esc</kbd></td>
											<td>Close Piano Roll / Modal</td>
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
		background: rgba(0, 0, 0, 0.72);
		backdrop-filter: blur(4px);
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 24px;
	}

	.modal {
		width: 680px;
		max-width: 95vw;
		height: 520px;
		max-height: 85vh;
		background: #0e0e0e;
		border: 1px solid var(--border);
		box-shadow: 0 16px 40px rgba(0, 0, 0, 0.6);
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.modal-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 18px;
		border-bottom: 1px solid var(--border-subtle);
		background: var(--bg);
	}

	.title {
		margin: 0;
		font-size: 13px;
		font-weight: 600;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--text);
	}

	.close-btn {
		width: 28px;
		height: 28px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: transparent;
		border: none;
		color: var(--text-subtle);
		font-size: 14px;
		cursor: pointer;
		padding: 0;
	}

	.close-btn:hover {
		color: var(--text);
	}

	.modal-body {
		display: flex;
		flex: 1;
		min-height: 0;
	}

	.sidebar {
		width: 160px;
		flex: none;
		background: #0a0a0a;
		border-right: 1px solid var(--border-subtle);
		display: flex;
		flex-direction: column;
		padding: 8px 0;
	}

	.tab-btn {
		width: 100%;
		text-align: left;
		padding: 10px 16px;
		font-size: 12px;
		background: transparent;
		border: none;
		border-left: 2px solid transparent;
		color: var(--text-subtle);
		cursor: pointer;
		transition: color 0.1s, background 0.1s;
	}

	.tab-btn:hover {
		color: var(--text);
		background: rgba(255, 255, 255, 0.03);
	}

	.tab-btn.active {
		color: var(--text);
		background: rgba(255, 255, 255, 0.05);
		border-left-color: var(--green);
		font-weight: 500;
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
		gap: 16px;
	}

	.section-title {
		margin: 0 0 6px 0;
		font-size: 12px;
		font-weight: 600;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--text-subtle);
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
		font-weight: 500;
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

	.status-dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--green);
	}

	.mono-value {
		font-family: inherit;
		font-size: 12px;
		color: var(--text);
	}

	.select {
		background: #141414;
		border: 1px solid var(--border);
		color: var(--text);
		font-size: 12px;
		padding: 5px 8px;
		outline: none;
	}

	.switch {
		position: relative;
		display: inline-block;
		width: 34px;
		height: 18px;
	}

	.switch input {
		opacity: 0;
		width: 0;
		height: 0;
	}

	.slider {
		position: absolute;
		cursor: pointer;
		inset: 0;
		background-color: #222;
		border: 1px solid var(--border);
		border-radius: 18px;
		transition: 0.2s;
	}

	.slider:before {
		position: absolute;
		content: "";
		height: 12px;
		width: 12px;
		left: 2px;
		bottom: 2px;
		background-color: var(--text-subtle);
		border-radius: 50%;
		transition: 0.2s;
	}

	input:checked + .slider {
		background-color: var(--green-soft);
		border-color: var(--green);
	}

	input:checked + .slider:before {
		transform: translateX(16px);
		background-color: var(--green);
	}

	.update-action-box {
		padding: 14px;
		background: #121212;
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
		border-radius: 2px;
	}

	.badge.checking {
		background: rgba(255, 255, 255, 0.08);
		color: var(--text-subtle);
	}

	.badge.available {
		background: var(--olive-soft);
		color: var(--olive);
	}

	.badge.ready {
		background: var(--green-soft);
		color: var(--green);
	}

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
		color: #000;
		font-size: 11px;
		font-weight: 600;
		padding: 6px 14px;
		border: none;
		cursor: pointer;
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}

	.primary-btn:hover {
		filter: brightness(1.1);
	}

	.secondary-btn {
		background: #1a1a1a;
		color: var(--text);
		border: 1px solid var(--border);
		font-size: 11px;
		padding: 6px 14px;
		cursor: pointer;
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}

	.secondary-btn:hover:not(:disabled) {
		border-color: var(--text-subtle);
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
		background: #222;
		overflow: hidden;
	}

	.progress-bar-fill {
		height: 100%;
		background: var(--green);
		transition: width 0.2s ease-out;
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
		max-height: 280px;
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
		background: #141414;
		color: var(--text-subtle);
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}

	.shortcuts-table td {
		color: var(--text);
	}

	kbd {
		background: #202020;
		border: 1px solid var(--border);
		border-radius: 2px;
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
