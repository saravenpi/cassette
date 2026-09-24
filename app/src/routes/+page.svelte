<script lang="ts">
	import { onMount } from 'svelte';

	import { send, sendNow, state as readState } from '$lib/api';
	import { intentFor, isAppKey, pitchFor, repeats } from '$lib/keys';
	import type { EngineEvent, UiState } from '$lib/types';

	const BEATS_PER_BAR = 4;

	let snapshot: UiState | null = $state(null);
	let failure: string | null = $state(null);

	const held = new Set<number>();

	function apply(event: EngineEvent) {
		void send(event).catch((error) => {
			failure = String(error);
		});
	}

	function noteOn(pitch: number) {
		if (held.has(pitch)) return;
		held.add(pitch);
		sendNow({ noteOn: { pitch, velocity: 100 } });
	}

	function noteOff(pitch: number) {
		if (!held.delete(pitch)) return;
		sendNow({ noteOff: { pitch } });
	}

	/** Select a block by clicking it, expressed as the deltas the engine speaks. */
	function selectBlock(track: number, block: number) {
		if (!snapshot) return;
		if (track !== snapshot.cursor.track) apply({ moveTrack: track - snapshot.cursor.track });
		if (block !== snapshot.cursor.block) apply({ moveBlock: block - snapshot.cursor.block });
	}

	function position(beats: number) {
		const whole = Math.max(0, Math.floor(beats));
		return { bar: Math.floor(whole / BEATS_PER_BAR) + 1, beat: (whole % BEATS_PER_BAR) + 1 };
	}

	onMount(() => {
		let alive = true;
		let frame = 0;
		let polling = false;

		async function poll() {
			polling = true;
			try {
				snapshot = await readState();
				failure = null;
			} catch (error) {
				failure = error instanceof Error ? error.message : String(error);
			}
			polling = false;
		}

		function loop() {
			if (!alive) return;
			if (!polling) void poll();
			frame = requestAnimationFrame(loop);
		}

		frame = requestAnimationFrame(loop);

		function onKeyDown(event: KeyboardEvent) {
			const renaming = snapshot?.renaming ?? false;
			const pitch = renaming ? null : pitchFor(event.code, snapshot?.octave ?? 0);

			if (pitch !== null) {
				if (!event.repeat) noteOn(pitch);
			} else {
				const intent = intentFor(event, renaming);
				if (intent && (!event.repeat || repeats(intent))) apply(intent);
			}

			if (isAppKey(event, renaming)) event.preventDefault();
		}

		function onKeyUp(event: KeyboardEvent) {
			const pitch = pitchFor(event.code, snapshot?.octave ?? 0);
			if (pitch !== null) noteOff(pitch);

			// A focused <button> runs its Space activation as the default action of
			// keyup, so cancelling only the keydown leaves it free to fire a second time.
			if (isAppKey(event, snapshot?.renaming ?? false)) event.preventDefault();
		}

		function releaseAll() {
			for (const pitch of held) noteOff(pitch);
		}

		window.addEventListener('keydown', onKeyDown);
		window.addEventListener('keyup', onKeyUp);
		window.addEventListener('blur', releaseAll);

		return () => {
			alive = false;
			cancelAnimationFrame(frame);
			window.removeEventListener('keydown', onKeyDown);
			window.removeEventListener('keyup', onKeyUp);
			window.removeEventListener('blur', releaseAll);
		};
	});
</script>

<div class="app">
	<header>
		<span class="wordmark">zboub69</span>

		<div class="transport">
			<button class="play" class:pressed={snapshot?.playing} onclick={() => apply('togglePlay')}>
				{snapshot?.playing ? 'Stop' : 'Play'}
			</button>
			<button class:pressed={snapshot?.looping} onclick={() => apply('toggleLoop')}>Loop</button>
		</div>

		<div class="clock">
			<span class="bpm">{Math.round(snapshot?.bpm ?? 120)}<small>bpm</small></span>
			{#if snapshot}
				{@const at = position(snapshot.transportPositionBeats)}
				<span class="bar">{at.bar}.{at.beat}</span>
			{/if}
			<span class="octave">oct {snapshot?.octave ?? 0}</span>
		</div>

		<div class="engine">
			<span>{snapshot?.backend ?? '…'}</span>
			<span class="dim">{snapshot?.sampleRate ?? 0} Hz</span>
		</div>
	</header>

	{#if failure}
		<p class="failure">{failure}</p>
	{/if}

	<main>
		<div class="tracks-head">
			<span class="col-index">#</span>
			<span class="col-name">track</span>
			<span class="col-blocks">blocks</span>
			<span class="col-meter">level</span>
			<span class="col-mix">mix</span>
		</div>

		{#each snapshot?.tracks ?? [] as track, index (index)}
			<div class="track" class:selected={index === snapshot?.cursor.track}>
				<span class="col-index">{index + 1}</span>

				<span class="col-name">
					<strong>{track.name}</strong>
					<em class="kind">{track.kind}</em>
					<span class="dim">{track.preset === 'custom' ? track.waveform : track.preset}</span>
				</span>

				<span class="col-blocks">
					{#each track.blocks as label, block (block)}
						<button
							class="block"
							class:cursor={index === snapshot?.cursor.track &&
								block === snapshot?.cursor.block}
							onclick={() => selectBlock(index, block)}
						>
							{label}
						</button>
					{/each}
					<button
						class="block add"
						class:cursor={index === snapshot?.cursor.track && track.addSlot}
						onclick={() => selectBlock(index, track.blocks.length)}
						title="select the + slot, then press N to add a block"
					>
						+
					</button>
				</span>

				<span class="col-meter">
					<span class="meter"><i style="width: {Math.round(track.level * 100)}%"></i></span>
				</span>

				<span class="col-mix">
					<span class="dim">vol {track.volume.toFixed(2)}</span>
					<span class="dim">pan {track.pan >= 0 ? '+' : ''}{track.pan.toFixed(2)}</span>
					<button class="tiny" class:pressed={track.muted} onclick={() => apply('toggleMute')}>M</button>
					<button class="tiny" class:pressed={track.solo} onclick={() => apply('toggleSolo')}>S</button>
				</span>
			</div>
		{/each}
	</main>

	<footer>
		<span class="status" class:renaming={snapshot?.renaming}>{snapshot?.status ?? 'connecting…'}</span>
		<span class="hints">
			<kbd>space</kbd> play
			<kbd>&uarr;&darr;</kbd> track
			<kbd>&larr;&rarr;</kbd> block
			<kbd>N</kbd> add
			<kbd>R</kbd> rename
			<kbd>A</kbd>&ndash;<kbd>K</kbd> notes
		</span>
	</footer>
</div>

<style>
	.app {
		display: flex;
		flex-direction: column;
		height: 100vh;
	}

	header {
		display: flex;
		align-items: center;
		gap: 24px;
		padding: 10px 16px;
		background: var(--panel);
		border-bottom: 1px solid var(--line);
	}

	.wordmark {
		font-weight: 700;
		letter-spacing: -0.02em;
	}

	.transport,
	.clock,
	.engine {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.engine {
		margin-left: auto;
		font-size: 12px;
	}

	.play {
		min-width: 62px;
	}

	.pressed {
		border-color: var(--head);
		color: var(--head);
	}

	.clock .bpm {
		font-variant-numeric: tabular-nums;
		font-size: 18px;
	}

	.clock .bpm small {
		font-size: 10px;
		color: var(--dim);
		margin-left: 4px;
	}

	.clock .bar {
		font-variant-numeric: tabular-nums;
		color: var(--accent);
	}

	.dim {
		color: var(--dim);
		font-size: 12px;
	}

	.failure {
		margin: 0;
		padding: 8px 16px;
		background: color-mix(in srgb, var(--warn) 18%, transparent);
		color: var(--warn);
		font-family: ui-monospace, monospace;
		font-size: 12px;
	}

	main {
		flex: 1;
		overflow-y: auto;
		padding: 12px 16px;
	}

	.tracks-head,
	.track {
		display: grid;
		grid-template-columns: 24px minmax(150px, 1fr) minmax(240px, 3fr) 110px 240px;
		align-items: center;
		gap: 12px;
	}

	.tracks-head {
		padding: 0 8px 6px;
		color: var(--dim);
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}

	.track {
		padding: 8px;
		border: 1px solid transparent;
		border-radius: var(--radius);
	}

	.track + .track {
		margin-top: 2px;
	}

	.track.selected {
		background: var(--panel);
		border-color: var(--line);
	}

	.col-index {
		color: var(--dim);
		font-variant-numeric: tabular-nums;
	}

	.col-name {
		display: flex;
		align-items: baseline;
		gap: 8px;
		overflow: hidden;
	}

	.kind {
		font-style: normal;
		font-size: 10px;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--accent);
	}

	.col-blocks {
		display: flex;
		gap: 4px;
		flex-wrap: wrap;
	}

	.block {
		font-size: 12px;
		padding: 3px 8px;
	}

	.block.add {
		color: var(--dim);
	}

	.block.cursor {
		border-color: var(--head);
		color: var(--head);
	}

	.meter {
		display: block;
		height: 6px;
		border-radius: 3px;
		background: var(--panel-2);
		overflow: hidden;
	}

	.meter i {
		display: block;
		height: 100%;
		background: linear-gradient(90deg, var(--head), var(--accent));
		transition: width 60ms linear;
	}

	.col-mix {
		display: flex;
		align-items: center;
		gap: 8px;
		font-variant-numeric: tabular-nums;
	}

	.tiny {
		padding: 1px 7px;
		font-size: 11px;
	}

	footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		padding: 8px 16px;
		background: var(--panel);
		border-top: 1px solid var(--line);
		font-size: 12px;
	}

	.status {
		color: var(--dim);
	}

	.status.renaming {
		color: var(--head);
	}

	.hints {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--dim);
	}
</style>