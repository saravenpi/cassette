<script lang="ts">
	/**
	 * The piano roll: the open MIDI block's notes on a pitch-by-beat grid, with a
	 * keyboard down the left that auditions a pitch while it is held. A click on
	 * empty grid adds a note, a drag moves it or, from its right edge, resizes it,
	 * the wheel over a note sets its velocity and Delete removes the selected one.
	 * Edits go through the session's per-frame queues, and the edited note is drawn
	 * from local state until the snapshot catches up.
	 */

	import { onDestroy, onMount } from 'svelte';

	import { beatsToPx, zoomBy } from '$lib/geometry';
	import { trackColor } from '$lib/palette';
	import {
		DEFAULT_NOTE_BEATS,
		DEFAULT_VELOCITY,
		KEYS_WIDTH,
		MIDDLE_C,
		NOTE_GRID,
		PITCH_COUNT,
		PITCH_MAX,
		ROLL_ZOOM_DEFAULT,
		ROW_PX,
		cellAt,
		clampLength,
		clampMove,
		floorToGrid,
		hitNote,
		isBlackKey,
		noteName,
		pitchToY,
		snapToGrid,
		velocityOpacity,
		velocityStep
	} from '$lib/pianoRoll';
	import { session } from '$lib/session.svelte';
	import type { NoteView } from '$lib/wire';

	const SETTLE_TIMEOUT_MS = 250;
	const SETTLE_EPSILON = 0.001;
	const PITCHES = Array.from({ length: PITCH_COUNT }, (_, row) => PITCH_MAX - row);

	interface NoteDrag {
		kind: 'move' | 'resize';
		note: number;
		pitch: number;
		startBeats: number;
		lengthBeats: number;
		grab: number;
		released: boolean;
		releasedAt: number;
	}

	interface PendingVelocity {
		note: number;
		velocity: number;
		sentAt: number;
	}

	let scroller = $state<HTMLDivElement | null>(null);
	let grid = $state<HTMLDivElement | null>(null);
	let zoom = $state(ROLL_ZOOM_DEFAULT);
	let selected = $state<number | null>(null);
	let drag = $state<NoteDrag | null>(null);
	let pending = $state<PendingVelocity | null>(null);
	let hover = $state<'add' | 'move' | 'resize'>('add');
	let auditioning = $state<number | null>(null);

	const roll = $derived(session.roll);
	const track = $derived(roll === null ? null : (session.snapshot?.tracks[roll.track] ?? null));
	const block = $derived(roll === null || track === null ? null : (track.blocks[roll.block] ?? null));
	const blockBeats = $derived(block?.lengthBeats ?? 0);
	const beatsPerBar = $derived(session.snapshot?.beatsPerBar ?? 4);
	const gridWidth = $derived(Math.max(1, beatsToPx(blockBeats, zoom)));
	const gridHeight = PITCH_COUNT * ROW_PX;
	const beats = $derived(Array.from({ length: Math.floor(blockBeats) + 1 }, (_, beat) => beat));
	const fill = $derived(trackColor(track?.colorIndex ?? 0));

	const notes = $derived.by((): NoteView[] =>
		(block?.notes ?? []).map((note, index) => {
			if (drag !== null && drag.note === index) {
				return { ...note, pitch: drag.pitch, startBeats: drag.startBeats, lengthBeats: drag.lengthBeats };
			}
			if (pending !== null && pending.note === index) return { ...note, velocity: pending.velocity };
			return note;
		})
	);

	const current = $derived(selected === null ? null : (notes[selected] ?? null));

	onMount(() => {
		if (scroller === null) return;
		scroller.scrollTop = pitchToY(MIDDLE_C) + ROW_PX / 2 - scroller.clientHeight / 2;
	});

	onDestroy(() => release());

	$effect(() => {
		const live = block?.notes ?? [];
		const now = performance.now();
		if (drag !== null && drag.released) {
			const note = live[drag.note];
			const confirmed =
				note !== undefined &&
				(drag.kind === 'move'
					? note.pitch === drag.pitch && Math.abs(note.startBeats - drag.startBeats) < SETTLE_EPSILON
					: Math.abs(note.lengthBeats - drag.lengthBeats) < SETTLE_EPSILON);
			if (confirmed || now - drag.releasedAt > SETTLE_TIMEOUT_MS) drag = null;
		}
		if (pending !== null) {
			const confirmed = live[pending.note]?.velocity === pending.velocity;
			if (confirmed || now - pending.sentAt > SETTLE_TIMEOUT_MS) pending = null;
		}
	});

	function pointIn(event: MouseEvent): { x: number; y: number } | null {
		if (grid === null) return null;
		const rect = grid.getBoundingClientRect();
		return { x: event.clientX - rect.left, y: event.clientY - rect.top };
	}

	function add(x: number, y: number): void {
		if (roll === null || block === null) return;
		const cell = cellAt(x, y, zoom);
		const startBeats = Math.min(floorToGrid(cell.beats), Math.max(0, blockBeats - NOTE_GRID));
		const lengthBeats = clampLength(startBeats, DEFAULT_NOTE_BEATS, blockBeats);
		const index = block.notes.length;
		const { track: trackIndex, block: blockIndex } = roll;
		void session
			.request({
				addNote: {
					track: trackIndex,
					block: blockIndex,
					pitch: cell.pitch,
					startBeats,
					lengthBeats,
					velocity: DEFAULT_VELOCITY
				}
			})
			.then((accepted) => {
				if (accepted) selected = index;
			});
	}

	function onPointerDown(event: PointerEvent): void {
		if (event.button !== 0 || grid === null) return;
		const point = pointIn(event);
		if (point === null) return;
		event.preventDefault();
		grid.focus();
		const hit = hitNote(notes, point.x, point.y, zoom);
		if (hit === null) {
			add(point.x, point.y);
			return;
		}
		const note = notes[hit.note];
		selected = hit.note;
		grid.setPointerCapture(event.pointerId);
		session.dispatch('checkpoint');
		drag = {
			kind: hit.edge ? 'resize' : 'move',
			note: hit.note,
			pitch: note.pitch,
			startBeats: note.startBeats,
			lengthBeats: note.lengthBeats,
			grab: cellAt(point.x, point.y, zoom).beats - note.startBeats,
			released: false,
			releasedAt: 0
		};
	}

	function onPointerMove(event: PointerEvent): void {
		const point = pointIn(event);
		if (point === null) return;
		if (drag === null || drag.released) {
			const hit = hitNote(notes, point.x, point.y, zoom);
			hover = hit === null ? 'add' : hit.edge ? 'resize' : 'move';
			return;
		}
		const cell = cellAt(point.x, point.y, zoom);
		if (drag.kind === 'move') {
			const start = snapToGrid(cell.beats - drag.grab, event.altKey);
			const next = clampMove(cell.pitch, start, drag.lengthBeats, blockBeats);
			if (next.pitch === drag.pitch && next.startBeats === drag.startBeats) return;
			drag.pitch = next.pitch;
			drag.startBeats = next.startBeats;
			session.queueNoteMove(drag.note, next.pitch, next.startBeats);
		} else {
			const end = snapToGrid(cell.beats, event.altKey);
			const lengthBeats = clampLength(drag.startBeats, end - drag.startBeats, blockBeats);
			if (lengthBeats === drag.lengthBeats) return;
			drag.lengthBeats = lengthBeats;
			session.queueNoteResize(drag.note, lengthBeats);
		}
	}

	function onPointerUp(): void {
		if (drag === null || drag.released) return;
		drag.released = true;
		drag.releasedAt = performance.now();
	}

	function onWheel(event: WheelEvent): void {
		const point = pointIn(event);
		if (point === null) return;
		const hit = hitNote(notes, point.x, point.y, zoom);
		const delta = event.shiftKey && event.deltaY === 0 ? event.deltaX : event.deltaY;
		if (hit === null || delta === 0) return;
		event.preventDefault();
		const velocity = velocityStep(notes[hit.note].velocity, delta, event.shiftKey);
		selected = hit.note;
		pending = { note: hit.note, velocity, sentAt: performance.now() };
		session.queueNoteVelocity(hit.note, velocity);
	}

	function onKeyDown(event: KeyboardEvent): void {
		if (session.renaming || (event.key !== 'Delete' && event.key !== 'Backspace')) return;
		event.preventDefault();
		event.stopPropagation();
		if (roll === null || selected === null || block?.notes[selected] === undefined) return;
		session.dispatch({ deleteNote: { track: roll.track, block: roll.block, note: selected } });
		selected = null;
	}

	function audition(event: PointerEvent, pitch: number): void {
		if (event.button !== 0) return;
		release();
		auditioning = pitch;
		session.sendNow({ noteOn: { pitch, velocity: DEFAULT_VELOCITY } });
	}

	function release(): void {
		if (auditioning === null) return;
		session.sendNow({ noteOff: { pitch: auditioning } });
		auditioning = null;
	}
</script>

<section class="roll" aria-label="Piano roll">
	<header class="bar">
		<span class="title">{track?.name ?? ''} / {block?.name ?? ''}</span>
		<span class="info" aria-live="polite">
			{#if current !== null}
				{noteName(current.pitch)} at beat {current.startBeats + 1}, length {current.lengthBeats},
				velocity {current.velocity}
			{:else}
				{notes.length} {notes.length === 1 ? 'note' : 'notes'}
			{/if}
		</span>
		<button class="small" aria-label="Zoom the piano roll out" onclick={() => (zoom = zoomBy(zoom, -1))}>
			&minus;
		</button>
		<button class="small" aria-label="Zoom the piano roll in" onclick={() => (zoom = zoomBy(zoom, 1))}>
			+
		</button>
		<button class="small" aria-label="Close piano roll" onclick={() => session.closeRoll()}>
			&times;
		</button>
	</header>

	<div class="scroller" bind:this={scroller}>
		<div class="canvas" style="width: {KEYS_WIDTH + gridWidth}px; height: {gridHeight}px">
			<div class="keys" role="group" aria-label="Audition keys" style="width: {KEYS_WIDTH}px">
				{#each PITCHES as pitch (pitch)}
					<button
						type="button"
						class="key"
						class:black={isBlackKey(pitch)}
						class:down={auditioning === pitch}
						tabindex="-1"
						aria-label="Play {noteName(pitch)}"
						style="height: {ROW_PX}px"
						onpointerdown={(event) => audition(event, pitch)}
						onpointerup={release}
						onpointerleave={release}
						onpointercancel={release}
					>
						{#if pitch % 12 === 0}{noteName(pitch)}{/if}
					</button>
				{/each}
			</div>

			<div
				class="grid {hover}"
				class:dragging={drag !== null && !drag.released}
				role="grid"
				aria-rowcount={PITCH_COUNT}
				aria-label="Notes of {block?.name ?? 'the block'}: click to add, drag to move, drag the right edge to resize, wheel for velocity, Delete to remove"
				tabindex="0"
				bind:this={grid}
				style="width: {gridWidth}px; height: {gridHeight}px; --fill: {fill}"
				onpointerdown={onPointerDown}
				onpointermove={onPointerMove}
				onpointerup={onPointerUp}
				onpointercancel={onPointerUp}
				onwheel={onWheel}
				onkeydown={onKeyDown}
			>
				<svg class="lines" width={gridWidth} height={gridHeight} aria-hidden="true">
					{#each PITCHES as pitch (pitch)}
						{#if isBlackKey(pitch)}
							<rect class="black-row" x="0" y={pitchToY(pitch)} width={gridWidth} height={ROW_PX} />
						{/if}
						{#if pitch % 12 === 0}
							<line
								class="octave"
								x1="0"
								x2={gridWidth}
								y1={pitchToY(pitch) + ROW_PX - 0.5}
								y2={pitchToY(pitch) + ROW_PX - 0.5}
							/>
						{/if}
					{/each}
					{#each beats as beat (beat)}
						<line
							class:bar={beat % beatsPerBar === 0}
							x1={beatsToPx(beat, zoom) + 0.5}
							x2={beatsToPx(beat, zoom) + 0.5}
							y1="0"
							y2={gridHeight}
						/>
					{/each}
				</svg>

				{#each notes as note, index (index)}
					<div
						class="note"
						class:selected={selected === index}
						aria-hidden="true"
						style="left: {beatsToPx(note.startBeats, zoom)}px; top: {pitchToY(note.pitch)}px; width: {beatsToPx(note.lengthBeats, zoom)}px; height: {ROW_PX}px; opacity: {velocityOpacity(note.velocity)}"
					></div>
				{/each}
			</div>
		</div>
	</div>
</section>

<style>
	.roll {
		flex: none;
		height: 280px;
		display: flex;
		flex-direction: column;
		border-top: 1px solid var(--line);
		background: var(--panel);
	}

	.bar {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 4px 8px;
		border-bottom: 1px solid var(--line);
		font-size: 11px;
	}

	.title {
		color: var(--text);
		white-space: nowrap;
	}

	.info {
		margin-right: auto;
		padding-left: 8px;
		color: var(--dim);
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.small {
		padding: 0 6px;
		font-size: 11px;
		line-height: 16px;
		height: 18px;
	}

	.scroller {
		flex: 1;
		min-height: 0;
		overflow: auto;
		background: var(--bg);
	}

	.canvas {
		display: flex;
		position: relative;
	}

	.keys {
		position: sticky;
		left: 0;
		z-index: 2;
		flex: none;
		display: flex;
		flex-direction: column;
		background: var(--panel);
		border-right: 1px solid var(--line);
	}

	.key {
		flex: none;
		padding: 0 4px 0 0;
		border: 0;
		border-bottom: 1px solid var(--line);
		background: color-mix(in srgb, var(--text) 82%, var(--panel));
		color: var(--bg);
		font-size: 8px;
		line-height: 1;
		text-align: right;
		cursor: pointer;
	}

	.key.black {
		background: var(--panel-2);
		color: var(--dim);
	}

	.key:hover {
		border-color: var(--line);
		background: color-mix(in srgb, var(--accent) 40%, var(--panel));
	}

	.key.down {
		background: var(--accent);
	}

	.grid {
		position: relative;
		flex: none;
		user-select: none;
		touch-action: none;
	}

	.grid.add {
		cursor: crosshair;
	}

	.grid.move {
		cursor: grab;
	}

	.grid.resize {
		cursor: ew-resize;
	}

	.grid.dragging.move {
		cursor: grabbing;
	}

	.grid:focus-visible {
		outline-offset: -2px;
	}

	.lines {
		position: absolute;
		inset: 0;
		pointer-events: none;
	}

	.black-row {
		fill: var(--panel);
	}

	.lines line {
		stroke: var(--line);
		stroke-width: 1;
	}

	.lines line.bar {
		stroke: var(--dim);
		stroke-opacity: 0.5;
	}

	.lines line.octave {
		stroke: var(--dim);
		stroke-opacity: 0.35;
	}

	.note {
		position: absolute;
		background: var(--fill);
		border: 1px solid color-mix(in srgb, var(--fill) 60%, var(--bg));
		pointer-events: none;
	}

	.note.selected {
		border-color: var(--text);
		box-shadow: inset 0 0 0 1px var(--text);
	}
</style>
