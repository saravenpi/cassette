<script lang="ts">
	/**
	 * The transport bar: play, stop, loop, record, the metronome with its menu
	 * (count-in and record quantize), tempo, the position readout, the view
	 * switcher and the engine's note about which backend it settled on. While a
	 * count-in runs, the readout shows the beats left as `-N` in the alert colour.
	 */

	import Icon, { addCollection } from '@iconify/svelte';
	import pixelarticons from '@iconify-json/pixelarticons/icons.json';
	import { formatCountIn, formatPosition } from '$lib/geometry';
	import { metronomeMenu, type MenuEntry } from '$lib/menu';
	import { menu } from '$lib/menu.svelte';
	import { formatExportPercent, projectStem } from '$lib/project';
	import { session } from '$lib/session.svelte';
	import { settingsStore } from '$lib/settings.svelte';

	addCollection(pixelarticons);

	interface Props {
		activeView?: 'timeline' | 'mixer';
	}

	let { activeView = $bindable('timeline') }: Props = $props();

	const snapshot = $derived(session.snapshot);
	const stem = $derived(projectStem(snapshot?.projectPath ?? null));
	const bpm = $derived(Math.round(snapshot?.bpm ?? 120));
	const position = $derived(
		formatPosition(snapshot?.transportPositionBeats ?? 0, snapshot?.beatsPerBar ?? 4)
	);
	const metronomeOn = $derived(snapshot?.metronome ?? false);
	const metronomeIcon = $derived(
		!metronomeOn
			? 'pixelarticons:speed-medium'
			: Math.floor(snapshot?.transportPositionBeats ?? 0) % 2 === 0
				? 'pixelarticons:speed-slow'
				: 'pixelarticons:speed-fast'
	);
	const countIn = $derived(formatCountIn(snapshot?.countInBeatsLeft ?? 0));
	const isMac = typeof navigator !== 'undefined' && /Macintosh|Mac OS X/.test(navigator.userAgent);
	const exportProgress = $derived(session.snapshot?.exportProgress ?? session.exportProgress);
	const exportPercent = $derived(formatExportPercent(exportProgress));

	function openMetronomeMenu(event: MouseEvent): void {
		const button = event.currentTarget;
		if (!(button instanceof HTMLElement)) return;
		const rect = button.getBoundingClientRect();
		const entries = metronomeMenu(session, snapshot?.countInBars ?? 0, snapshot?.recordQuantizeBeats ?? 0);
		menu.open(rect.left, rect.bottom + 2, entries, button);
	}

	function openSaveMenu(event: MouseEvent): void {
		const button = event.currentTarget;
		if (!(button instanceof HTMLElement)) return;
		const rect = button.getBoundingClientRect();
		const entries: MenuEntry[] = [
			{
				kind: 'item',
				label: 'Save As…',
				shortcut: 'Ctrl+Shift+S',
				run: () => session.saveProjectAs()
			}
		];
		menu.open(rect.left, rect.bottom + 2, entries, button);
	}
</script>

<header data-tauri-drag-region class:mac={isMac}>
	<div class="group" role="group" aria-label="Project">
		<span class="project-name" title={snapshot?.projectPath ?? 'Untitled'}>
			{stem}{#if snapshot?.projectDirty}<span class="dirty" aria-hidden="true">&bull;</span>{/if}
		</span>
		<div class="save-pair">
			<button
				class="icon-btn"
				class:loading={session.saving}
				aria-label="Save"
				title="Save project (Ctrl+S)"
				disabled={session.saving}
				onclick={() => session.saveProject()}
			>
				{#if session.saving}
					<span class="spinner" aria-hidden="true"></span>
				{:else}
					<Icon icon="pixelarticons:save-sharp" width="16" height="16" />
				{/if}
			</button>
			<button
				class="step"
				aria-label="Save options"
				aria-haspopup="menu"
				title="Save options"
				disabled={session.saving}
				onclick={openSaveMenu}
			>
				&#9662;
			</button>
		</div>
		<button
			class="export-btn icon-btn"
			class:loading={session.exporting}
			aria-label="Export"
			title="Export audio"
			disabled={session.exporting}
			onclick={() => session.exportAudio()}
		>
			{#if session.exporting}
				<span class="spinner" aria-hidden="true"></span>
				{#if exportPercent !== null}{exportPercent}%{/if}
			{:else}
				<Icon icon="pixelarticons:upload-sharp" width="16" height="16" />
			{/if}
			{#if session.exporting && exportPercent !== null}
				<span class="progress-bar" style="width: {exportPercent}%"></span>
			{/if}
		</button>
		<button
			class="icon-btn"
			aria-label="Open"
			title="Open project (Ctrl+O)"
			onclick={() => session.loadProjectFromDialog()}
		>
			<Icon icon="pixelarticons:folder-sharp" width="16" height="16" />
		</button>
	</div>

	<div class="group" role="group" aria-label="Transport">
		<button
			class="icon-btn"
			aria-label="Play"
			title="Play (Space)"
			aria-pressed={snapshot?.playing ?? false}
			disabled={session.exporting}
			onclick={() => session.dispatch('togglePlay')}
		>
			<Icon icon="pixelarticons:play" width="16" height="16" />
		</button>
		<button
			class="icon-btn"
			aria-label="Stop"
			title="Stop"
			disabled={session.exporting}
			onclick={() => session.dispatch('stop')}
		>
			<Icon icon="pixelarticons:stop" width="16" height="16" />
		</button>
		<button
			class="icon-btn"
			aria-label="Loop"
			title="Loop"
			aria-pressed={snapshot?.looping ?? false}
			disabled={session.exporting}
			onclick={() => session.dispatch('toggleLoop')}
		>
			<Icon icon="pixelarticons:repeat-sharp" width="16" height="16" />
		</button>
		<button
			class="rec icon-btn"
			aria-label="Record notes"
			title="Record the notes you play into the selected MIDI track (F9)"
			aria-pressed={snapshot?.recording ?? false}
			disabled={session.exporting}
			onclick={() => session.dispatch('toggleRecord')}
		>
			<span class="dot" aria-hidden="true"></span>
		</button>
	</div>

	<div class="group" role="group" aria-label="Metronome">
		<button
			aria-pressed={snapshot?.metronome ?? false}
			aria-label="Metronome"
			class="icon-btn"
			title="Metronome click"
			onclick={() => session.dispatch('toggleMetronome')}
		>
			<Icon icon={metronomeIcon} width="16" height="16" />
		</button>
		<button
			class="step"
			aria-label="Count-in and record quantize"
			aria-haspopup="menu"
			title="Count-in and record quantize"
			onclick={openMetronomeMenu}
		>
			&#9662;
		</button>
	</div>

	<div class="group" role="group" aria-label="Tempo">
		<button class="step" aria-label="Slower" onclick={() => session.dispatch({ bpmDelta: -1 })}>
			&minus;
		</button>
		<span class="readout">{bpm}<small>bpm</small></span>
		<button class="step" aria-label="Faster" onclick={() => session.dispatch({ bpmDelta: 1 })}>
			+
		</button>
	</div>

	<div class="group" role="group" aria-label="History">
		<button
			class="icon-btn"
			aria-label="Undo"
			title="Undo (Ctrl+Z)"
			disabled={!(snapshot?.canUndo ?? false)}
			onclick={() => session.dispatch('undo')}
		>
			<Icon icon="pixelarticons:undo-sharp" width="16" height="16" />
		</button>
		<button
			class="icon-btn"
			aria-label="Redo"
			title="Redo (Ctrl+Shift+Z)"
			disabled={!(snapshot?.canRedo ?? false)}
			onclick={() => session.dispatch('redo')}
		>
			<Icon icon="pixelarticons:redo-sharp" width="16" height="16" />
		</button>
	</div>

	{#if countIn !== null}
		<span class="position counting" aria-label="Count-in, beats left">{countIn}</span>
	{:else}
		<span class="position" aria-label="Position">{position}</span>
	{/if}

	<div class="engine">
		<span class="note">{snapshot?.backendNote ?? ''}</span>
	</div>

	<div class="group view-group" role="group" aria-label="View">
		<button
			class="view-btn icon-btn"
			class:active={activeView === 'timeline'}
			aria-label="Timeline"
			aria-pressed={activeView === 'timeline'}
			title="Timeline view (Tab)"
			onclick={() => (activeView = 'timeline')}
		>
			<Icon icon="pixelarticons:align-end-vertical-sharp" width="16" height="16" />
		</button>
		<button
			class="view-btn icon-btn"
			class:active={activeView === 'mixer'}
			aria-label="Mixer"
			aria-pressed={activeView === 'mixer'}
			title="Mixer view (Tab)"
			onclick={() => (activeView = 'mixer')}
		>
			<Icon icon="pixelarticons:sliders-vertical" width="16" height="16" />
		</button>
	</div>

	<div class="group settings-group" role="group" aria-label="Settings">
		<button
			class="icon-btn settings-btn"
			aria-label="Settings"
			aria-pressed={settingsStore.isOpen}
			title="Settings"
			onclick={() => settingsStore.toggle()}
		>
			<Icon icon="pixelarticons:settings-cog" width="16" height="16" />
		</button>
	</div>
</header>

<style>
	button[aria-pressed='true'] {
		background: var(--green);
		color: var(--on-green);
	}

	header {
		display: flex;
		align-items: center;
		flex: none;
		height: 48px;
		padding: 0 16px;
		background: var(--bg);
		border-bottom: 1px solid var(--border-subtle);
		user-select: none;
	}

	header.mac {
		padding-left: 84px;
	}

	.group {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 0 14px;
		border-left: 1px solid var(--border-subtle);
	}

	.group:first-child {
		padding-left: 0;
		border-left: 0;
	}

	.settings-group {
		padding-left: 8px;
		padding-right: 0;
		border-left: 0;
	}

	.icon-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-width: 28px;
		height: 24px;
		padding: 0 6px;
		gap: 4px;
	}

	.export-btn {
		position: relative;
		overflow: hidden;
	}

	.progress-bar {
		position: absolute;
		bottom: 0;
		left: 0;
		height: 2px;
		background: var(--green);
		transition: width 0.1s linear;
	}

	.spinner {
		display: inline-block;
		width: 8px;
		height: 8px;
		margin-right: 4px;
		border: 1.5px solid var(--text-subtle);
		border-top-color: var(--green);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
		vertical-align: middle;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.save-pair {
		display: flex;
		align-items: center;
	}

	.project-name {
		display: flex;
		align-items: center;
		gap: 4px;
		margin-left: 6px;
		font-size: 11px;
		letter-spacing: 0.04em;
		color: var(--text);
		max-width: 140px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.dirty {
		color: var(--green);
		font-size: 14px;
	}

	.step {
		width: 26px;
		padding: 0;
		background: transparent;
		color: var(--text-subtle);
	}

	.step:hover:not(:disabled) {
		color: var(--text);
		background: var(--surface-hover);
	}

	.rec {
		background: transparent;
		color: var(--text-subtle);
	}

	.rec:hover:not(:disabled) {
		color: var(--alert);
		background: var(--alert-soft);
	}

	.rec[aria-pressed='true'] {
		color: var(--alert);
		background: var(--alert-soft);
	}

	.dot {
		width: 6px;
		height: 6px;
		background: var(--alert);
	}

	.readout {
		min-width: 44px;
		text-align: center;
		font-size: 14px;
		font-variant-numeric: tabular-nums;
	}

	.readout small {
		margin-left: 4px;
		font-size: 10px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--text-subtle);
	}

	.position {
		padding: 0 14px;
		border-left: 1px solid var(--border-subtle);
		font-size: 18px;
		font-variant-numeric: tabular-nums;
		color: var(--green);
		min-width: 64px;
	}

	.position.counting {
		color: var(--alert);
	}

	.engine {
		margin-left: auto;
		display: flex;
		align-items: baseline;
		gap: 10px;
		min-width: 0;
		font-size: 11px;
		color: var(--text-subtle);
	}

	.note {
		max-width: 320px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.view-btn.active {
		background: var(--green);
		color: var(--on-green);
	}
</style>
