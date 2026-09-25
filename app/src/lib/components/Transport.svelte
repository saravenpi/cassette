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
</script>

<header>
	<span class="wordmark">zboub69</span>

	<div class="group" role="group" aria-label="Transport">
		<button
			class="wide"
			class:on={snapshot?.playing}
			aria-pressed={snapshot?.playing ?? false}
			onclick={() => session.dispatch('togglePlay')}
		>
			Play
		</button>
		<button onclick={() => session.dispatch('stop')}>Stop</button>
		<button
			class:on={snapshot?.looping}
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
			&bull; Rec
		</button>
	</div>

	<div class="group" role="group" aria-label="Export">
		<button onclick={() => session.dispatch({ exportAudio: { path: null } })}>Export WAV</button>
		<button onclick={() => session.dispatch({ exportProject: { path: null } })}>Save</button>
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
		<span class="readout octave">oct {snapshot?.octave ?? 0}</span>
		<button class="step" aria-label="Octave up" onclick={() => session.dispatch({ octaveDelta: 1 })}>
			+
		</button>
	</div>

	<span class="position">{position}</span>

	<div class="engine">
		<span class="backend">{snapshot?.backend ?? 'connecting'}</span>
		<span class="note">{snapshot?.backendNote ?? ''}</span>
	</div>
</header>

<style>
	header {
		display: flex;
		align-items: center;
		gap: 18px;
		padding: 6px 12px;
		background: var(--panel);
		border-bottom: 1px solid var(--line);
	}

	.wordmark {
		font-weight: 700;
		letter-spacing: -0.02em;
	}

	.group {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.wide {
		min-width: 56px;
	}

	.step {
		padding: 2px 8px;
	}

	.on {
		border-color: var(--head);
		color: var(--head);
	}

	.rec {
		color: var(--accent);
		font-weight: 600;
	}

	.rec:hover {
		border-color: var(--accent);
	}

	.readout {
		font-variant-numeric: tabular-nums;
		font-size: 16px;
		padding: 0 2px;
	}

	.readout small {
		font-size: 9px;
		color: var(--dim);
		margin-left: 3px;
	}

	.octave {
		font-size: 12px;
		color: var(--dim);
	}

	.position {
		font-variant-numeric: tabular-nums;
		color: var(--accent);
		min-width: 42px;
	}

	.engine {
		margin-left: auto;
		display: flex;
		align-items: baseline;
		gap: 8px;
		min-width: 0;
		font-size: 11px;
		color: var(--dim);
	}

	.backend {
		color: var(--text);
	}

	.note {
		max-width: 320px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
