<script lang="ts">
	/**
	 * A track's strip: its colour, name and kind, its mute and solo, and the two
	 * faders next to the level meter. Slider moves are collapsed to one message per
	 * frame by the session, so a drag never floods the engine.
	 */

	import { HEADER_WIDTH, ROW_HEIGHT } from '$lib/geometry';
	import { trackColor } from '$lib/palette';
	import { session } from '$lib/session.svelte';
	import type { TrackView } from '$lib/wire';

	interface Props {
		track: TrackView;
		index: number;
	}

	let { track, index }: Props = $props();

	let draggedVolume = $state<number | null>(null);
	let draggedPan = $state<number | null>(null);

	const color = $derived(trackColor(track.colorIndex));
	const selected = $derived(session.snapshot?.cursor.track === index);
	const level = $derived(Math.round(Math.min(1, Math.max(0, track.level)) * 100));
	const volume = $derived(draggedVolume ?? track.volume);
	const pan = $derived(draggedPan ?? track.pan);

	/** A fader follows the pointer while it is down, then hands back to the snapshot. */
	function releaseFaders(): void {
		draggedVolume = null;
		draggedPan = null;
	}

	function mix(nextVolume: number, nextPan: number, muted: boolean, solo: boolean): void {
		session.queueMix(index, nextVolume, nextPan, muted, solo);
	}

	function onFader(event: Event, which: 'volume' | 'pan'): void {
		const input = event.currentTarget;
		if (!(input instanceof HTMLInputElement)) return;
		const value = Number(input.value);
		if (which === 'volume') draggedVolume = value;
		else draggedPan = value;
		mix(volume, pan, track.muted, track.solo);
	}
</script>

<div class="head" style="width: {HEADER_WIDTH}px; height: {ROW_HEIGHT}px; --fill: {color}">
	<button class="pick" aria-pressed={selected} onclick={() => session.dispatch({ selectTrack: { track: index } })}>
		<span class="chip"></span>
		<span class="name">{track.name}</span>
		<span class="kind">{track.kind === 'midi' ? 'MID' : 'AUD'}</span>
	</button>

	<div class="mix">
		<input
			class="fader"
			type="range"
			min="0"
			max="1"
			step="0.01"
			value={volume}
			aria-label="Volume of {track.name}"
			oninput={(event) => onFader(event, 'volume')}
			onchange={releaseFaders}
			onblur={releaseFaders}
		/>
		<input
			class="fader"
			type="range"
			min="-1"
			max="1"
			step="0.02"
			value={pan}
			aria-label="Pan of {track.name}"
			oninput={(event) => onFader(event, 'pan')}
			onchange={releaseFaders}
			onblur={releaseFaders}
		/>
		<span class="meter"><i style="width: {level}%"></i></span>
		<button
			class="toggle"
			class:on={track.muted}
			aria-pressed={track.muted}
			aria-label="Mute {track.name}"
			onclick={() => mix(volume, pan, !track.muted, track.solo)}
		>
			M
		</button>
		<button
			class="toggle"
			class:on={track.solo}
			aria-pressed={track.solo}
			aria-label="Solo {track.name}"
			onclick={() => mix(volume, pan, track.muted, !track.solo)}
		>
			S
		</button>
	</div>
</div>

<style>
	.head {
		display: grid;
		grid-auto-rows: 1fr;
		flex: none;
		overflow: hidden;
		background: var(--panel);
		border-right: 1px solid var(--line);
		border-bottom: 1px solid var(--line);
	}

	.pick {
		display: flex;
		align-items: center;
		gap: 5px;
		width: 100%;
		height: 100%;
		padding: 0 6px;
		text-align: left;
		overflow: hidden;
		background: transparent;
		border: 0;
		border-radius: 0;
	}

	.pick:hover {
		background: var(--panel-2);
	}

	.chip {
		flex: none;
		width: 6px;
		height: 8px;
		border-radius: 1px;
		background: var(--fill);
	}

	.name {
		font-size: 11px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.kind {
		margin-left: auto;
		flex: none;
		font-size: 8px;
		letter-spacing: 0.06em;
		color: var(--dim);
	}

	.mix {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 0 6px;
	}

	.fader {
		flex: 1;
		width: 0;
		height: 12px;
		margin: 0;
		accent-color: var(--dim);
	}

	.meter {
		flex: none;
		width: 40px;
		height: 4px;
		border-radius: 2px;
		background: var(--panel-2);
		overflow: hidden;
	}

	.meter i {
		display: block;
		height: 100%;
		background: var(--fill);
	}

	.toggle {
		flex: none;
		height: 11px;
		line-height: 1;
		padding: 0 4px;
		font-size: 8px;
		border-radius: 2px;
	}

	.toggle.on {
		border-color: var(--head);
		color: var(--head);
	}
</style>
