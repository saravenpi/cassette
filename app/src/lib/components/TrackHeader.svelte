<script lang="ts">
	/**
	 * A track's strip: its colour, name and kind, its mute and solo, and the two
	 * faders next to the level meter. Slider moves are collapsed to one message per
	 * frame by the session, so a drag never floods the engine.
	 */

	import { HEADER_WIDTH, ROW_HEIGHT } from '$lib/geometry';
	import { trackMenu } from '$lib/menu';
	import { menu } from '$lib/menu.svelte';
	import { trackColor } from '$lib/palette';
	import { session } from '$lib/session.svelte';
	import type { TrackView } from '$lib/wire';

	interface Props {
		track: TrackView;
		index: number;
		height?: number;
	}

	let { track, index, height }: Props = $props();

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

	function onContextMenu(event: MouseEvent): void {
		event.preventDefault();
		menu.open(
			event.clientX,
			event.clientY,
			trackMenu(session, index, track, (muted, solo) => mix(volume, pan, muted, solo))
		);
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

<div
	class="head"
	role="group"
	aria-label="{track.name} strip"
	style="width: {HEADER_WIDTH}px; height: {height ?? ROW_HEIGHT}px; --fill: {color}"
	oncontextmenu={onContextMenu}
>
	<div class="top-row">
		<button class="pick" aria-pressed={selected} onclick={() => session.dispatch({ selectTrack: { track: index } })}>
			<span class="chip"></span>
			<span class="name">{track.name}</span>
			<span class="kind">{track.kind === 'midi' ? track.preset || 'Midi' : 'Audio'}</span>
		</button>
		<div class="buttons">
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

	<div class="mix">
		<input
			class="fader"
			type="range"
			min="0"
			max="1"
			step="0.01"
			value={volume}
			aria-label="Volume of {track.name}"
			onpointerdown={() => session.dispatch('checkpoint')}
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
			onpointerdown={() => session.dispatch('checkpoint')}
			oninput={(event) => onFader(event, 'pan')}
			onchange={releaseFaders}
			onblur={releaseFaders}
		/>
		<span class="meter"><i style="width: {level}%"></i></span>
	</div>
</div>

<style>
	.head {
		display: flex;
		flex-direction: column;
		justify-content: center;
		gap: 8px;
		flex: none;
		overflow: hidden;
		background: var(--surface);
		border-left: 1px solid var(--border-subtle);
		border-bottom: 1px solid var(--border-subtle);
		padding: 6px 10px;
	}

	.top-row {
		display: flex;
		align-items: center;
		gap: 6px;
		min-width: 0;
	}

	.buttons {
		display: flex;
		align-items: center;
		gap: 2px;
		flex: none;
	}

	.pick {
		display: flex;
		align-items: center;
		justify-content: flex-start;
		gap: 8px;
		flex: 1;
		min-width: 0;
		height: 22px;
		padding: 0 4px 0 0;
		text-align: left;
		text-transform: none;
		letter-spacing: 0.02em;
		overflow: hidden;
		background: transparent;
		color: var(--text-subtle);
	}

	.pick:hover:not(:disabled) {
		filter: none;
		color: var(--text);
	}

	.pick[aria-pressed='true'] {
		background: transparent;
		color: var(--text);
	}

	.chip {
		flex: none;
		width: 2px;
		height: 14px;
		background: var(--fill);
	}

	.name {
		font-size: 12px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.kind {
		margin-left: auto;
		flex: none;
		max-width: 64px;
		overflow: hidden;
		text-overflow: ellipsis;
		font-size: 9px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--text-subtle);
	}

	.mix {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.fader {
		flex: 1;
		width: 0;
		height: 14px;
		margin: 0;
		accent-color: var(--green);
	}

	.meter {
		flex: none;
		width: 36px;
		height: 4px;
		background: var(--surface-hover);
		overflow: hidden;
	}

	.meter i {
		display: block;
		height: 100%;
		background: linear-gradient(
			90deg,
			var(--green) 0 60%,
			var(--olive) 60% 85%,
			var(--alert) 85% 100%
		);
		background-size: 36px 100%;
	}

	.toggle {
		flex: none;
		width: 20px;
		height: 20px;
		padding: 0;
		font-size: 10px;
		letter-spacing: 0;
		color: var(--text-subtle);
	}

	.toggle.on {
		color: var(--on-green);
	}

	.toggle.on[aria-label^='Mute'] {
		background: var(--olive);
	}

	.toggle.on[aria-label^='Solo'] {
		background: var(--green);
	}
</style>
