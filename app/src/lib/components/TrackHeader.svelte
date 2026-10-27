<script lang="ts">
	/**
	 * A track's strip: its colour, name and kind, its mute and solo, and the two
	 * faders next to the level meter. Slider moves are collapsed to one message per
	 * frame by the session, so a drag never floods the engine.
	 */

	import { HEADER_WIDTH, ROW_HEIGHT, rangePercent } from '$lib/geometry';
	import { trackMenu } from '$lib/menu';
	import { menu } from '$lib/menu.svelte';
	import { trackColor } from '$lib/palette';
	import { session } from '$lib/session.svelte';
	import { trackSelection } from '$lib/trackSelection.svelte';
	import type { TrackView } from '$lib/wire';

	interface Props {
		track: TrackView;
		index: number;
		height?: number;
		dragging?: boolean;
		ongrab?: (event: PointerEvent) => void;
	}

	let { track, index, height, dragging = false, ongrab }: Props = $props();

	let draggedVolume = $state<number | null>(null);
	let draggedPan = $state<number | null>(null);
	let renameInput = $state<HTMLInputElement | null>(null);
	let renaming = $state(false);
	let renameText = $state('');

	const DEFAULT_VOLUME = 0.8;
	const DEFAULT_PAN = 0.0;

	const color = $derived(trackColor(track.colorIndex));
	const selected = $derived(session.snapshot?.cursor.track === index);
	const isSelected = $derived(selected || trackSelection.isSelected(index));
	const level = $derived(Math.round(Math.min(1, Math.max(0, track.level)) * 100));
	const volume = $derived(draggedVolume ?? track.volume);
	const pan = $derived(draggedPan ?? track.pan);

	$effect(() => {
		if (draggedVolume !== null && Math.abs(track.volume - draggedVolume) < 0.001) {
			draggedVolume = null;
		}
		if (draggedPan !== null && Math.abs(track.pan - draggedPan) < 0.001) {
			draggedPan = null;
		}
	});

	$effect(() => {
		if (renaming && renameInput) {
			renameInput.focus();
			renameInput.select();
		}
	});

	function startRename(): void {
		renameText = track.name;
		renaming = true;
	}

	function commitRename(): void {
		if (!renaming) return;
		renaming = false;
		const trimmed = renameText.trim();
		if (trimmed && trimmed !== track.name) {
			session.renameTrack(index, trimmed);
		}
	}

	function cancelRename(): void {
		renaming = false;
		renameText = track.name;
	}

	function onRenameKeyDown(event: KeyboardEvent): void {
		event.stopPropagation();
		if (event.key === 'Enter') {
			event.preventDefault();
			commitRename();
		} else if (event.key === 'Escape') {
			event.preventDefault();
			cancelRename();
		}
	}

	/** A fader follows the pointer while it is down, then hands back to the snapshot. */
	function releaseFaders(): void {
		draggedVolume = null;
		draggedPan = null;
	}

	function mix(nextVolume: number, nextPan: number, muted: boolean, solo: boolean): void {
		session.queueMix(index, nextVolume, nextPan, muted, solo);
	}

	function resetFader(which: 'volume' | 'pan'): void {
		session.dispatch('checkpoint');
		if (which === 'volume') {
			draggedVolume = DEFAULT_VOLUME;
			mix(DEFAULT_VOLUME, pan, track.muted, track.solo);
		} else {
			draggedPan = DEFAULT_PAN;
			mix(volume, DEFAULT_PAN, track.muted, track.solo);
		}
	}

	function onPick(event: MouseEvent): void {
		trackSelection.selectTrack(index, {
			shift: event.shiftKey,
			toggle: event.metaKey || event.ctrlKey,
			totalTracks: session.snapshot?.tracks.length ?? 1
		});
		session.selectTrack(index);
	}

	function toggleMute(): void {
		const nextMuted = !track.muted;
		if (isSelected && trackSelection.selectedTracks.size > 1) {
			for (const t of trackSelection.selectedTracks) {
				const item = session.snapshot?.tracks[t];
				if (item) {
					session.queueMix(t, item.volume, item.pan, nextMuted, item.solo);
				}
			}
		} else {
			mix(volume, pan, nextMuted, track.solo);
		}
	}

	function toggleSolo(): void {
		const nextSolo = !track.solo;
		if (isSelected && trackSelection.selectedTracks.size > 1) {
			for (const t of trackSelection.selectedTracks) {
				const item = session.snapshot?.tracks[t];
				if (item) {
					session.queueMix(t, item.volume, item.pan, item.muted, nextSolo);
				}
			}
		} else {
			mix(volume, pan, track.muted, nextSolo);
		}
	}

	function onContextMenu(event: MouseEvent): void {
		event.preventDefault();
		const totalTracks = session.snapshot?.tracks.length ?? 1;
		const inSelection = isSelected;
		const entries = trackMenu(
			session,
			index,
			track,
			totalTracks,
			(muted, solo) => {
				if (muted !== track.muted) {
					toggleMute();
				} else if (solo !== track.solo) {
					toggleSolo();
				} else {
					mix(volume, pan, muted, solo);
				}
			},
			startRename
		).map((entry) => {
			if (entry.kind === 'colors' && inSelection) {
				return {
					...entry,
					pick: (colorIndex: number) => {
						for (const t of trackSelection.selectedTracks) {
							void session.request({ setTrackColor: { track: t, colorIndex } });
						}
					}
				};
			}
			return entry;
		});
		menu.open(event.clientX, event.clientY, entries);
	}

	function onFader(event: Event, which: 'volume' | 'pan'): void {
		const input = event.currentTarget;
		if (!(input instanceof HTMLInputElement)) return;
		const value = Number(input.value);
		if (which === 'volume') draggedVolume = value;
		else draggedPan = value;
		mix(volume, pan, track.muted, track.solo);
	}

	function onFaderKeyDown(event: KeyboardEvent, which: 'volume' | 'pan'): void {
		if (
			event.key !== 'ArrowUp' &&
			event.key !== 'ArrowRight' &&
			event.key !== 'ArrowDown' &&
			event.key !== 'ArrowLeft'
		) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		if (!event.repeat) session.dispatch('checkpoint');
		const dir = event.key === 'ArrowUp' || event.key === 'ArrowRight' ? 1 : -1;
		const step = which === 'volume' ? 0.01 : 0.02;
		const cur = which === 'volume' ? volume : pan;
		const min = which === 'volume' ? 0 : -1;
		const max = 1;
		const next = Math.max(min, Math.min(max, Number((cur + dir * step).toFixed(2))));
		if (which === 'volume') draggedVolume = next;
		else draggedPan = next;
		mix(which === 'volume' ? next : volume, which === 'pan' ? next : pan, track.muted, track.solo);
	}
</script>

<div
	class="head"
	class:dragging
	class:selected={isSelected}
	role="group"
	aria-label="{track.name} strip"
	data-track-header={index}
	style="width: {HEADER_WIDTH}px; height: {height ?? ROW_HEIGHT}px; --fill: {color}"
	oncontextmenu={onContextMenu}
>
	<div class="top-row">
		{#if !renaming}
			<button
				type="button"
				class="drag-handle"
				aria-label="Drag to reorder {track.name}"
				title="Drag to reorder"
				onpointerdown={(event) => {
					if (event.button === 0 && ongrab) ongrab(event);
				}}
			>
				<svg width="6" height="12" viewBox="0 0 6 12" aria-hidden="true">
					<circle cx="1.5" cy="2" r="1" fill="currentColor" />
					<circle cx="4.5" cy="2" r="1" fill="currentColor" />
					<circle cx="1.5" cy="6" r="1" fill="currentColor" />
					<circle cx="4.5" cy="6" r="1" fill="currentColor" />
					<circle cx="1.5" cy="10" r="1" fill="currentColor" />
					<circle cx="4.5" cy="10" r="1" fill="currentColor" />
				</svg>
			</button>
		{/if}
		{#if renaming}
			<input
				bind:this={renameInput}
				class="rename-input"
				bind:value={renameText}
				aria-label="Rename {track.name}"
				onkeydown={onRenameKeyDown}
				onblur={commitRename}
			/>
		{:else}
			<button
				class="pick"
				aria-pressed={isSelected}
				onclick={(event) => onPick(event)}
				ondblclick={(event) => {
					event.stopPropagation();
					startRename();
				}}
				onpointerdown={(event) => {
					if (event.button === 0 && ongrab) ongrab(event);
				}}
			>
				<span class="chip"></span>
				<span class="name">{track.name}</span>
				<span class="kind">{track.kind === 'midi' ? track.preset || 'Midi' : 'Audio'}</span>
			</button>
		{/if}
		<div class="buttons">
			<button
				class="toggle"
				class:on={track.muted}
				aria-pressed={track.muted}
				aria-label="Mute {track.name}"
				onclick={toggleMute}
			>
				M
			</button>
			<button
				class="toggle"
				class:on={track.solo}
				aria-pressed={track.solo}
				aria-label="Solo {track.name}"
				onclick={toggleSolo}
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
			style="--filled: {rangePercent(volume, 0, 1)}"
			aria-label="Volume of {track.name}"
			onpointerdown={(event) => {
				if (event.detail === 2) {
					resetFader('volume');
				} else {
					session.dispatch('checkpoint');
				}
			}}
			ondblclick={() => resetFader('volume')}
			oninput={(event) => onFader(event, 'volume')}
			onkeydown={(event) => onFaderKeyDown(event, 'volume')}
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
			style="--filled: {rangePercent(pan, -1, 1)}"
			aria-label="Pan of {track.name}"
			onpointerdown={(event) => {
				if (event.detail === 2) {
					resetFader('pan');
				} else {
					session.dispatch('checkpoint');
				}
			}}
			ondblclick={() => resetFader('pan')}
			oninput={(event) => onFader(event, 'pan')}
			onkeydown={(event) => onFaderKeyDown(event, 'pan')}
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

	.head.selected {
		background: var(--surface-hover);
		border-left-color: var(--fill);
	}

	.head.dragging {
		opacity: 0.5;
	}

	.drag-handle {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 10px;
		height: 18px;
		padding: 0;
		background: transparent;
		border: 0;
		color: var(--text-subtle);
		cursor: grab;
		flex: none;
	}

	.drag-handle:hover {
		color: var(--text);
	}

	.drag-handle:active {
		cursor: grabbing;
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

	.rename-input {
		flex: 1;
		min-width: 0;
		height: 22px;
		margin: 0;
		padding: 0 4px;
		font: inherit;
		font-size: 12px;
		color: var(--text);
		background: var(--bg);
		border: 1px solid var(--green);
	}

	.rename-input:focus {
		outline: 1px solid var(--green);
	}
</style>
