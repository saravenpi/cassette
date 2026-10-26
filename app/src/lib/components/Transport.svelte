<script lang="ts">
	/**
	 * The transport bar: play, stop, loop, tempo, the position readout, the play
	 * octave and the engine's note about which backend it settled on.
	 */

	import { formatPosition } from '$lib/geometry';
	import { session } from '$lib/session.svelte';

	const snapshot = $derived(session.snapshot);
	const bpm = $derived(Math.round(snapshot?.bpm ?? 120));
	const position = $derived(
		formatPosition(snapshot?.transportPositionBeats ?? 0, snapshot?.beatsPerBar ?? 4)
	);
	const isMac = typeof navigator !== 'undefined' && /Macintosh|Mac OS X/.test(navigator.userAgent);

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
			aria-label="Record project to file"
			onclick={() => session.dispatch({ recordAudio: { path: null } })}
		>
			<span class="dot" aria-hidden="true"></span>
			Rec
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
		<button onclick={() => session.dispatch({ exportAudio: { path: null } })}>Export</button>
		<button onclick={() => session.dispatch({ exportProject: { path: null } })}>Save</button>
		<button onclick={() => session.loadProjectFromDialog()}>Load</button>
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

	<span class="position" aria-label="Position">{position}</span>

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
