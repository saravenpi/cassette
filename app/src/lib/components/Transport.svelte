<script lang="ts">
	/**
	 * The transport bar: play, stop, loop, record, the metronome with its menu
	 * (count-in and record quantize), tempo, the position readout, the play
	 * octave and the engine's note about which backend it settled on. While a
	 * count-in runs, the readout shows the beats left as `-N` in the alert colour.
	 */

	import { formatCountIn, formatPosition } from '$lib/geometry';
	import { metronomeMenu, type MenuEntry } from '$lib/menu';
	import { menu } from '$lib/menu.svelte';
	import { projectStem } from '$lib/project';
	import { session } from '$lib/session.svelte';

	const snapshot = $derived(session.snapshot);
	const stem = $derived(projectStem(snapshot?.projectPath ?? null));
	const bpm = $derived(Math.round(snapshot?.bpm ?? 120));
	const position = $derived(
		formatPosition(snapshot?.transportPositionBeats ?? 0, snapshot?.beatsPerBar ?? 4)
	);
	const countIn = $derived(formatCountIn(snapshot?.countInBeatsLeft ?? 0));
	const isMac = typeof navigator !== 'undefined' && /Macintosh|Mac OS X/.test(navigator.userAgent);

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
	<div class="group" role="group" aria-label="Transport">
		<button
			class="wide"
			aria-pressed={snapshot?.playing ?? false}
			onclick={() => session.dispatch('togglePlay')}
		>
			Play
		</button>
		<button onclick={() => session.dispatch('stop')}>Stop</button>
		<button
			aria-pressed={snapshot?.looping ?? false}
			onclick={() => session.dispatch('toggleLoop')}
		>
			Loop
		</button>
		<button
			class="rec"
			aria-label="Record notes"
			title="Record the notes you play into the selected MIDI track (F9)"
			aria-pressed={snapshot?.recording ?? false}
			onclick={() => session.dispatch('toggleRecord')}
		>
			<span class="dot" aria-hidden="true"></span>
			Rec
		</button>
	</div>

	<div class="group" role="group" aria-label="Metronome">
		<button
			aria-pressed={snapshot?.metronome ?? false}
			title="Metronome click"
			onclick={() => session.dispatch('toggleMetronome')}
		>
			Metro
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

	<div class="group" role="group" aria-label="History">
		<button
			aria-label="Undo"
			title="Undo (Ctrl+Z)"
			disabled={!(snapshot?.canUndo ?? false)}
			onclick={() => session.dispatch('undo')}
		>
			Undo
		</button>
		<button
			aria-label="Redo"
			title="Redo (Ctrl+Shift+Z)"
			disabled={!(snapshot?.canRedo ?? false)}
			onclick={() => session.dispatch('redo')}
		>
			Redo
		</button>
	</div>

	<div class="group" role="group" aria-label="Project">
		<button onclick={() => session.exportAudio()}>Export</button>
		<div class="save-pair">
			<button onclick={() => session.saveProject()}>Save</button>
			<button
				class="step"
				aria-label="Save options"
				aria-haspopup="menu"
				title="Save options"
				onclick={openSaveMenu}
			>
				&#9662;
			</button>
		</div>
		<button onclick={() => session.loadProjectFromDialog()}>Open</button>
		<span class="project-name" title={snapshot?.projectPath ?? 'Untitled'}>
			{stem}{#if snapshot?.projectDirty}<span class="dirty" aria-hidden="true">&bull;</span>{/if}
		</span>
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

	<div class="group" role="group" aria-label="Play octave">
		<button
			class="step"
			aria-label="Octave down"
			onclick={() => session.dispatch({ octaveDelta: -1 })}
		>
			&minus;
		</button>
		<span class="readout"><small class="lead">oct</small>{snapshot?.octave ?? 0}</span>
		<button class="step" aria-label="Octave up" onclick={() => session.dispatch({ octaveDelta: 1 })}>
			+
		</button>
	</div>

	{#if countIn !== null}
		<span class="position counting" aria-label="Count-in, beats left">{countIn}</span>
	{:else}
		<span class="position" aria-label="Position">{position}</span>
	{/if}

	<div class="engine">
		<span class="backend">{snapshot?.backend ?? 'connecting'}</span>
		<span class="note">{snapshot?.backendNote ?? ''}</span>
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

	.wide {
		min-width: 64px;
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

	.readout small.lead {
		margin: 0 4px 0 0;
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

	.backend {
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--text);
	}

	.note {
		max-width: 320px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
