<script lang="ts">
	/**
	 * The piano roll: notes of the open MIDI block on a pitch-by-beat grid, with a
	 * keyboard down the left that auditions pitches while held. Double-click on
	 * empty grid adds a note, dragging on empty grid draws a marquee selection, and
	 * dragging a note moves the whole selection. Edges resize from start or end,
	 * wheel sets velocity, and Delete removes selection. Draw mode toggles with B.
	 */

	import { onDestroy, onMount } from 'svelte';

	import { beatsToPx } from '$lib/geometry';
	import {
		addSelection,
		arrowNudge,
		arrowResize,
		arrowTranspose,
		clampGroupMove,
		clampGroupResizeLeft,
		clampGroupResizeRight,
		copySelection,
		duplicateSelection,
		getModuleClipboard,
		marqueeSelect,
		pasteClipboard,
		rangeSelection,
		remapAfterDelete,
		setModuleClipboard,
		toggleNote
	} from '$lib/noteSelection';
	import { trackColor } from '$lib/palette';
	import {
		DEFAULT_NOTE_BEATS,
		DEFAULT_VELOCITY,
		KEYS_WIDTH,
		MIDDLE_C,
		MIN_NOTE_BEATS,
		NOTE_GRID,
		PITCH_COUNT,
		PITCH_MAX,
		ROLL_ZOOM_DEFAULT,
		ROW_PX,
		cellAt,
		clampLength,
		floorToGrid,
		hitNote,
		isBlackKey,
		noteName,
		pitchToY,
		rollZoomBy,
		rollZoomByWheel,
		snapToGrid,
		velocityOpacity,
		velocityStep
	} from '$lib/pianoRoll';
	import { session } from '$lib/session.svelte';
	import type { NoteEdit, NoteView } from '$lib/wire';

	const SETTLE_TIMEOUT_MS = 250;
	const SETTLE_EPSILON = 0.001;
	const PITCHES = Array.from({ length: PITCH_COUNT }, (_, row) => PITCH_MAX - row);

	interface ActiveDrag {
		kind: 'move' | 'resize-start' | 'resize-end';
		anchorIndex: number;
		anchorStartBeats: number;
		anchorPitch: number;
		grabOffsetBeats: number;
		initialNotes: Map<number, NoteView>;
		currentEdits: Map<number, { pitch: number; startBeats: number; lengthBeats: number }>;
		released: boolean;
		releasedAt: number;
	}

	interface MarqueeState {
		startX: number;
		startY: number;
		currentX: number;
		currentY: number;
		startBeat: number;
		startPitch: number;
		additive: boolean;
		initialSelection: Set<number>;
		hasMoved: boolean;
	}

	interface DrawState {
		pitch: number;
		startBeats: number;
		lengthBeats: number;
	}

	interface PendingVelocity {
		velocity: number;
		sentAt: number;
	}

	let scroller = $state<HTMLDivElement | null>(null);
	let grid = $state<HTMLDivElement | null>(null);
	let zoom = $state(ROLL_ZOOM_DEFAULT);
	let selection = $state<Set<number>>(new Set());
	let drawMode = $state(false);
	let drag = $state<ActiveDrag | null>(null);
	let marquee = $state<MarqueeState | null>(null);
	let drawDrag = $state<DrawState | null>(null);
	let pendingVelocities = $state<Map<number, PendingVelocity>>(new Map());
	let hover = $state<'add' | 'draw' | 'move' | 'resize-start' | 'resize-end'>('add');
	let auditioning = $state<number | null>(null);
	let lastClickBeats = $state<number | null>(null);

	let coalescedEdits: NoteEdit[] | null = null;
	let animFrameId = 0;
	let lastVelocityWheelAt = 0;

	const roll = $derived(session.roll);
	const track = $derived(
		roll === null ? null : (session.snapshot?.tracks[roll.track] ?? null)
	);
	const noteFill = $derived(track !== null ? trackColor(track.colorIndex) : 'var(--green)');
	const block = $derived(
		roll === null || track === null ? null : (track.blocks[roll.block] ?? null)
	);
	const blockBeats = $derived(block?.lengthBeats ?? 0);
	const beatsPerBar = $derived(session.snapshot?.beatsPerBar ?? 4);
	const gridWidth = $derived(Math.max(1, beatsToPx(blockBeats, zoom)));
	const gridHeight = PITCH_COUNT * ROW_PX;
	const beats = $derived(Array.from({ length: Math.floor(blockBeats) + 1 }, (_, beat) => beat));

	const notes = $derived.by((): NoteView[] => {
		const base = block?.notes ?? [];
		return base.map((note, index) => {
			if (drag !== null) {
				const override = drag.currentEdits.get(index);
				if (override !== undefined) {
					return {
						...note,
						pitch: override.pitch,
						startBeats: override.startBeats,
						lengthBeats: override.lengthBeats
					};
				}
			}
			const pendingVel = pendingVelocities.get(index);
			if (pendingVel !== undefined) {
				return { ...note, velocity: pendingVel.velocity };
			}
			return note;
		});
	});

	const current = $derived(
		selection.size === 1 ? (notes[Array.from(selection)[0]] ?? null) : null
	);

	onMount(() => {
		if (scroller === null) return;
		scroller.scrollTop = pitchToY(MIDDLE_C) + ROW_PX / 2 - scroller.clientHeight / 2;
	});

	onDestroy(() => {
		release();
		if (animFrameId !== 0) cancelAnimationFrame(animFrameId);
	});

	$effect(() => {
		const live = block?.notes ?? [];
		const now = performance.now();
		if (drag !== null && drag.released) {
			let allSettled = true;
			for (const [index, edit] of drag.currentEdits) {
				const n = live[index];
				if (
					n === undefined ||
					n.pitch !== edit.pitch ||
					Math.abs(n.startBeats - edit.startBeats) >= SETTLE_EPSILON ||
					Math.abs(n.lengthBeats - edit.lengthBeats) >= SETTLE_EPSILON
				) {
					allSettled = false;
					break;
				}
			}
			if (allSettled || now - drag.releasedAt > SETTLE_TIMEOUT_MS) {
				drag = null;
			}
		}
		if (pendingVelocities.size > 0) {
			let changed = false;
			const next = new Map(pendingVelocities);
			for (const [index, { velocity, sentAt }] of pendingVelocities) {
				const n = live[index];
				const settled = n !== undefined && n.velocity === velocity;
				if (settled || now - sentAt > SETTLE_TIMEOUT_MS) {
					next.delete(index);
					changed = true;
				}
			}
			if (changed) pendingVelocities = next;
		}
	});

	$effect(() => {
		const live = block?.notes ?? [];
		let invalid = false;
		for (const idx of selection) {
			if (idx >= live.length) {
				invalid = true;
				break;
			}
		}
		if (invalid) {
			const filtered = new Set<number>();
			for (const idx of selection) {
				if (idx < live.length) filtered.add(idx);
			}
			selection = filtered;
		}
	});

	function queueDragEdits(trackIndex: number, blockIndex: number, edits: NoteEdit[]): void {
		coalescedEdits = edits;
		if (animFrameId === 0) {
			animFrameId = requestAnimationFrame(() => {
				animFrameId = 0;
				if (coalescedEdits !== null) {
					session.dispatch({
						editNotes: { track: trackIndex, block: blockIndex, edits: coalescedEdits }
					});
					coalescedEdits = null;
				}
			});
		}
	}

	function flushDragEdits(trackIndex: number, blockIndex: number): void {
		if (animFrameId !== 0) {
			cancelAnimationFrame(animFrameId);
			animFrameId = 0;
		}
		if (coalescedEdits !== null) {
			session.dispatch({
				editNotes: { track: trackIndex, block: blockIndex, edits: coalescedEdits }
			});
			coalescedEdits = null;
		}
	}

	function applyZoom(nextZoom: number, anchorX?: number): void {
		if (scroller === null) {
			zoom = nextZoom;
			return;
		}
		const oldZoom = zoom;
		const focusX = anchorX ?? Math.max(0, scroller.clientWidth / 2 - KEYS_WIDTH);
		zoom = nextZoom;
		if (focusX > 0) {
			scroller.scrollLeft = Math.max(
				0,
				scroller.scrollLeft + focusX * (nextZoom / oldZoom - 1)
			);
		}
	}

	function pointIn(event: MouseEvent): { x: number; y: number } | null {
		if (grid === null) return null;
		const rect = grid.getBoundingClientRect();
		return { x: event.clientX - rect.left, y: event.clientY - rect.top };
	}

	function onPointerDown(event: PointerEvent): void {
		if (event.button !== 0 || grid === null) return;
		const point = pointIn(event);
		if (point === null) return;
		event.preventDefault();
		grid.focus();

		const hit = hitNote(notes, point.x, point.y, zoom);

		if (drawMode) {
			if (hit !== null) {
				if (roll !== null) {
					session.dispatch({
						deleteNote: { track: roll.track, block: roll.block, note: hit.note }
					});
					selection = remapAfterDelete(selection, [hit.note]);
				}
				return;
			}
			const cell = cellAt(point.x, point.y, zoom);
			const snappedStart = floorToGrid(cell.beats);
			const startBeats = Math.min(snappedStart, Math.max(0, blockBeats - NOTE_GRID));
			const lengthBeats = clampLength(startBeats, NOTE_GRID, blockBeats);
			grid.setPointerCapture(event.pointerId);
			drawDrag = { pitch: cell.pitch, startBeats, lengthBeats };
			return;
		}

		if (hit === null) {
			grid.setPointerCapture(event.pointerId);
			const cell = cellAt(point.x, point.y, zoom);
			lastClickBeats = floorToGrid(cell.beats);
			marquee = {
				startX: point.x,
				startY: point.y,
				currentX: point.x,
				currentY: point.y,
				startBeat: cell.beats,
				startPitch: cell.pitch,
				additive: event.shiftKey,
				initialSelection: new Set(selection),
				hasMoved: false
			};
			return;
		}

		if (event.shiftKey) {
			selection = toggleNote(selection, hit.note);
		} else if (!selection.has(hit.note)) {
			selection = new Set([hit.note]);
		}

		if (!selection.has(hit.note)) return;

		grid.setPointerCapture(event.pointerId);
		session.dispatch('checkpoint');

		const anchor = notes[hit.note];
		const cell = cellAt(point.x, point.y, zoom);
		const initialNotes = new Map<number, NoteView>();
		const currentEdits = new Map<
			number,
			{ pitch: number; startBeats: number; lengthBeats: number }
		>();

		for (const index of selection) {
			const n = notes[index];
			if (n === undefined) continue;
			initialNotes.set(index, { ...n });
			currentEdits.set(index, {
				pitch: n.pitch,
				startBeats: n.startBeats,
				lengthBeats: n.lengthBeats
			});
		}

		let kind: 'move' | 'resize-start' | 'resize-end' = 'move';
		if (hit.edge === 'start') kind = 'resize-start';
		else if (hit.edge === 'end') kind = 'resize-end';

		drag = {
			kind,
			anchorIndex: hit.note,
			anchorStartBeats: anchor.startBeats,
			anchorPitch: anchor.pitch,
			grabOffsetBeats: cell.beats - anchor.startBeats,
			initialNotes,
			currentEdits,
			released: false,
			releasedAt: 0
		};
	}

	function onPointerMove(event: PointerEvent): void {
		const point = pointIn(event);
		if (point === null) return;

		if (drawDrag !== null) {
			const cell = cellAt(point.x, point.y, zoom);
			const endBeats = snapToGrid(cell.beats, event.altKey);
			if (endBeats > drawDrag.startBeats) {
				drawDrag.lengthBeats = clampLength(
					drawDrag.startBeats,
					endBeats - drawDrag.startBeats,
					blockBeats
				);
			}
			return;
		}

		if (marquee !== null) {
			marquee.currentX = point.x;
			marquee.currentY = point.y;
			if (
				Math.abs(point.x - marquee.startX) > 3 ||
				Math.abs(point.y - marquee.startY) > 3
			) {
				marquee.hasMoved = true;
			}
			const cell = cellAt(point.x, point.y, zoom);
			const hits = marqueeSelect(notes, {
				startBeat: marquee.startBeat,
				endBeat: cell.beats,
				startPitch: marquee.startPitch,
				endPitch: cell.pitch
			});
			if (marquee.additive) {
				selection = addSelection(marquee.initialSelection, hits);
			} else {
				selection = hits;
			}
			return;
		}

		if (drag !== null && !drag.released) {
			if (roll === null) return;
			const cell = cellAt(point.x, point.y, zoom);

			if (drag.kind === 'move') {
				const targetStart = snapToGrid(cell.beats - drag.grabOffsetBeats, event.altKey);
				const deltaBeats = targetStart - drag.anchorStartBeats;
				const deltaPitch = cell.pitch - drag.anchorPitch;
				const clamped = clampGroupMove(
					notes,
					drag.initialNotes.keys(),
					deltaPitch,
					deltaBeats,
					blockBeats
				);
				const edits: NoteEdit[] = [];
				const nextEdits = new Map<
					number,
					{ pitch: number; startBeats: number; lengthBeats: number }
				>();
				for (const [idx, init] of drag.initialNotes) {
					const p = init.pitch + clamped.deltaPitch;
					const s = init.startBeats + clamped.deltaBeats;
					const l = init.lengthBeats;
					nextEdits.set(idx, { pitch: p, startBeats: s, lengthBeats: l });
					edits.push({
						note: idx,
						pitch: p,
						startBeats: s,
						lengthBeats: l,
						velocity: init.velocity
					});
				}
				drag.currentEdits = nextEdits;
				queueDragEdits(roll.track, roll.block, edits);
			} else if (drag.kind === 'resize-start') {
				const targetStart = snapToGrid(cell.beats, event.altKey);
				const deltaBeats = targetStart - drag.anchorStartBeats;
				const clampedDelta = clampGroupResizeLeft(
					notes,
					drag.initialNotes.keys(),
					deltaBeats
				);
				const edits: NoteEdit[] = [];
				const nextEdits = new Map<
					number,
					{ pitch: number; startBeats: number; lengthBeats: number }
				>();
				for (const [idx, init] of drag.initialNotes) {
					const s = init.startBeats + clampedDelta;
					const l = init.lengthBeats - clampedDelta;
					nextEdits.set(idx, { pitch: init.pitch, startBeats: s, lengthBeats: l });
					edits.push({
						note: idx,
						pitch: init.pitch,
						startBeats: s,
						lengthBeats: l,
						velocity: init.velocity
					});
				}
				drag.currentEdits = nextEdits;
				queueDragEdits(roll.track, roll.block, edits);
			} else if (drag.kind === 'resize-end') {
				const targetEnd = snapToGrid(cell.beats, event.altKey);
				const anchorInit = drag.initialNotes.get(drag.anchorIndex);
				const anchorEnd =
					anchorInit !== undefined
						? anchorInit.startBeats + anchorInit.lengthBeats
						: targetEnd;
				const deltaBeats = targetEnd - anchorEnd;
				const clampedDelta = clampGroupResizeRight(
					notes,
					drag.initialNotes.keys(),
					deltaBeats,
					blockBeats
				);
				const edits: NoteEdit[] = [];
				const nextEdits = new Map<
					number,
					{ pitch: number; startBeats: number; lengthBeats: number }
				>();
				for (const [idx, init] of drag.initialNotes) {
					const s = init.startBeats;
					const l = init.lengthBeats + clampedDelta;
					nextEdits.set(idx, { pitch: init.pitch, startBeats: s, lengthBeats: l });
					edits.push({
						note: idx,
						pitch: init.pitch,
						startBeats: s,
						lengthBeats: l,
						velocity: init.velocity
					});
				}
				drag.currentEdits = nextEdits;
				queueDragEdits(roll.track, roll.block, edits);
			}
			return;
		}

		if (drawMode) {
			hover = 'draw';
			return;
		}

		const hit = hitNote(notes, point.x, point.y, zoom);
		if (hit === null) hover = 'add';
		else if (hit.edge === 'start') hover = 'resize-start';
		else if (hit.edge === 'end') hover = 'resize-end';
		else hover = 'move';
	}

	function onPointerUp(): void {
		if (drawDrag !== null) {
			if (roll !== null) {
				const { pitch, startBeats, lengthBeats } = drawDrag;
				const index = block?.notes.length ?? 0;
				void session
					.request({
						addNote: {
							track: roll.track,
							block: roll.block,
							pitch,
							startBeats,
							lengthBeats,
							velocity: DEFAULT_VELOCITY
						}
					})
					.then((accepted) => {
						if (accepted) selection = new Set([index]);
					});
			}
			drawDrag = null;
			return;
		}

		if (marquee !== null) {
			if (!marquee.hasMoved && !marquee.additive) {
				selection = new Set();
			}
			marquee = null;
			return;
		}

		if (drag !== null && !drag.released) {
			if (roll !== null) flushDragEdits(roll.track, roll.block);
			drag.released = true;
			drag.releasedAt = performance.now();
		}
	}

	function onDblClick(event: MouseEvent): void {
		if (drawMode || event.button !== 0 || roll === null || block === null) return;
		const point = pointIn(event);
		if (point === null) return;
		const hit = hitNote(notes, point.x, point.y, zoom);
		if (hit !== null) return;
		const cell = cellAt(point.x, point.y, zoom);
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
				if (accepted) selection = new Set([index]);
			});
	}

	function onWheel(event: WheelEvent): void {
		const point = pointIn(event);
		if (point === null || roll === null) return;
		const isZoom = event.ctrlKey || event.metaKey;
		if (isZoom) {
			const delta = event.deltaY !== 0 ? event.deltaY : event.deltaX;
			if (delta === 0) return;
			event.preventDefault();
			const nextZoom = rollZoomByWheel(zoom, delta);
			applyZoom(nextZoom, point.x);
			return;
		}
		const isVelocity = event.altKey;
		const hit = isVelocity ? hitNote(notes, point.x, point.y, zoom) : null;
		if (hit !== null) {
			const delta = event.shiftKey && event.deltaY === 0 ? event.deltaX : event.deltaY;
			if (delta === 0) return;
			event.preventDefault();
			const targetIndices = selection.has(hit.note) ? Array.from(selection) : [hit.note];
			const edits: NoteEdit[] = [];
			const nextPending = new Map(pendingVelocities);
			const now = performance.now();
			if (now - lastVelocityWheelAt > 300) {
				session.dispatch('checkpoint');
			}
			lastVelocityWheelAt = now;
			for (const idx of targetIndices) {
				const n = notes[idx];
				if (n === undefined) continue;
				const v = velocityStep(n.velocity, delta, event.shiftKey);
				edits.push({
					note: idx,
					pitch: n.pitch,
					startBeats: n.startBeats,
					lengthBeats: n.lengthBeats,
					velocity: v
				});
				nextPending.set(idx, { velocity: v, sentAt: now });
			}
			pendingVelocities = nextPending;
			session.dispatch({ editNotes: { track: roll.track, block: roll.block, edits } });
		}
	}

	function onKeyDown(event: KeyboardEvent): void {
		if (session.renaming) return;
		if (event.key === ' ' || event.code === 'Space') {
			event.preventDefault();
			session.dispatch('togglePlay');
			return;
		}

		if (event.ctrlKey || event.metaKey) {
			if (event.key === 'z' || event.key === 'Z') return;
			if (event.key === 'y' || event.key === 'Y') return;
			if (event.key === 'u' || event.key === 'U') return;

			if (event.key === 'a' || event.key === 'A') {
				event.preventDefault();
				event.stopPropagation();
				selection = new Set(notes.map((_, i) => i));
				return;
			}

			if (event.key === 'c' || event.key === 'C') {
				event.preventDefault();
				event.stopPropagation();
				const clip = copySelection(notes, selection);
				if (clip !== null) setModuleClipboard(clip);
				return;
			}

			if (event.key === 'x' || event.key === 'X') {
				event.preventDefault();
				event.stopPropagation();
				if (roll !== null && selection.size > 0) {
					const clip = copySelection(notes, selection);
					if (clip !== null) setModuleClipboard(clip);
					const toDelete = Array.from(selection).sort((a, b) => a - b);
					session.dispatch({
						deleteNotes: { track: roll.track, block: roll.block, notes: toDelete }
					});
					selection = new Set();
				}
				return;
			}

			if (event.key === 'v' || event.key === 'V') {
				event.preventDefault();
				event.stopPropagation();
				const clip = getModuleClipboard();
				if (roll !== null && clip !== null) {
					const pasted = pasteClipboard(clip, lastClickBeats, blockBeats);
					if (pasted.length > 0) {
						const oldLength = block?.notes.length ?? 0;
						void session
							.request({
								addNotes: { track: roll.track, block: roll.block, notes: pasted }
							})
							.then((accepted) => {
								if (accepted) selection = rangeSelection(oldLength, pasted.length);
							});
					}
				}
				return;
			}

			if (event.key === 'd' || event.key === 'D') {
				event.preventDefault();
				event.stopPropagation();
				if (roll !== null && selection.size > 0) {
					const duped = duplicateSelection(notes, selection, blockBeats);
					if (duped.length > 0) {
						const oldLength = block?.notes.length ?? 0;
						void session
							.request({
								addNotes: { track: roll.track, block: roll.block, notes: duped }
							})
							.then((accepted) => {
								if (accepted) selection = rangeSelection(oldLength, duped.length);
							});
					}
				}
				return;
			}
		}

		if (
			(event.key === 'b' || event.key === 'B') &&
			!event.ctrlKey &&
			!event.metaKey &&
			!event.altKey
		) {
			event.preventDefault();
			event.stopPropagation();
			drawMode = !drawMode;
			return;
		}

		if (event.key === 'Delete' || event.key === 'Backspace') {
			event.preventDefault();
			event.stopPropagation();
			if (roll !== null && selection.size > 0) {
				const toDelete = Array.from(selection).sort((a, b) => a - b);
				session.dispatch({
					deleteNotes: { track: roll.track, block: roll.block, notes: toDelete }
				});
				selection = new Set();
			}
			return;
		}

		if (event.key === 'Escape') {
			event.preventDefault();
			event.stopPropagation();
			if (selection.size > 0) {
				selection = new Set();
			} else {
				session.closeRoll();
			}
			return;
		}

		if (
			event.key === '+' ||
			event.key === '=' ||
			event.code === 'NumpadAdd' ||
			(event.shiftKey && event.code === 'Equal')
		) {
			event.preventDefault();
			event.stopPropagation();
			applyZoom(rollZoomBy(zoom, 1));
			return;
		}

		if (
			event.key === '-' ||
			event.key === '_' ||
			event.code === 'NumpadSubtract' ||
			(event.shiftKey && event.code === 'Minus')
		) {
			event.preventDefault();
			event.stopPropagation();
			applyZoom(rollZoomBy(zoom, -1));
			return;
		}

		if (event.key === 'ArrowUp') {
			event.preventDefault();
			event.stopPropagation();
			if (roll !== null && selection.size > 0) {
				if (!event.repeat) session.dispatch('checkpoint');
				const edits = arrowTranspose(notes, selection, event.shiftKey ? 12 : 1);
				session.dispatch({ editNotes: { track: roll.track, block: roll.block, edits } });
			}
			return;
		}

		if (event.key === 'ArrowDown') {
			event.preventDefault();
			event.stopPropagation();
			if (roll !== null && selection.size > 0) {
				if (!event.repeat) session.dispatch('checkpoint');
				const edits = arrowTranspose(notes, selection, event.shiftKey ? -12 : -1);
				session.dispatch({ editNotes: { track: roll.track, block: roll.block, edits } });
			}
			return;
		}

		if (event.key === 'ArrowLeft') {
			event.preventDefault();
			event.stopPropagation();
			if (roll !== null && selection.size > 0) {
				if (!event.repeat) session.dispatch('checkpoint');
				const step = event.altKey ? MIN_NOTE_BEATS : NOTE_GRID;
				const edits = event.shiftKey
					? arrowResize(notes, selection, -step, blockBeats)
					: arrowNudge(notes, selection, -step, blockBeats);
				session.dispatch({ editNotes: { track: roll.track, block: roll.block, edits } });
			}
			return;
		}

		if (event.key === 'ArrowRight') {
			event.preventDefault();
			event.stopPropagation();
			if (roll !== null && selection.size > 0) {
				if (!event.repeat) session.dispatch('checkpoint');
				const step = event.altKey ? MIN_NOTE_BEATS : NOTE_GRID;
				const edits = event.shiftKey
					? arrowResize(notes, selection, step, blockBeats)
					: arrowNudge(notes, selection, step, blockBeats);
				session.dispatch({ editNotes: { track: roll.track, block: roll.block, edits } });
			}
			return;
		}
	}

	function onQuantize(): void {
		if (roll === null) return;
		const selectedList = selection.size > 0 ? Array.from(selection).sort((a, b) => a - b) : [];
		session.dispatch({
			quantizeBlock: {
				track: roll.track,
				block: roll.block,
				gridBeats: NOTE_GRID,
				notes: selectedList
			}
		});
		grid?.focus();
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

<section class="roll" aria-label="Piano roll" style="--note-color: {noteFill}">
	<header class="bar">
		<span class="info" aria-live="polite">
			{#if selection.size > 1}
				{selection.size} selected
			{:else if current !== null}
				{noteName(current.pitch)} at beat {current.startBeats + 1},
				length {current.lengthBeats}, velocity {current.velocity}
			{:else}
				{notes.length} {notes.length === 1 ? 'note' : 'notes'}
			{/if}
		</span>
		<button
			class="icon pencil"
			class:active={drawMode}
			title="Draw mode (B)"
			aria-label="Toggle draw mode"
			aria-pressed={drawMode}
			onclick={() => {
				drawMode = !drawMode;
				grid?.focus();
			}}
		>
			<svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
				<path
					d="M12.146.146a.5.5 0 0 1 .708 0l3 3a.5.5 0 0 1 0 .708l-10 10
						a.5.5 0 0 1-.168.11l-5 2a.5.5 0 0 1-.65-.65l2-5
						a.5.5 0 0 1 .11-.168l10-10zM11.207 2.5 13.5 4.793 14.793 3.5
						12.5 1.207 11.207 2.5zm1.586 3L10.5 3.207 4 9.707V10h.5
						a.5.5 0 0 1 .5.5v.5h.5a.5.5 0 0 1 .5.5v.5h.293l6.5-6.5zm-9.761 5.175
						-.806 2.016 2.016-.806L4.05 11.7l-.87-.87-.18.175z"
				/>
			</svg>
		</button>
		<button
			class="quantize"
			title="Snap note starts to the sixteenth-note grid (Ctrl+U)"
			onclick={onQuantize}
		>
			Quantize
		</button>
		<button
			class="icon"
			aria-label="Zoom the piano roll out"
			onclick={() => applyZoom(rollZoomBy(zoom, -1))}
		>
			&minus;
		</button>
		<button
			class="icon"
			aria-label="Zoom the piano roll in"
			onclick={() => applyZoom(rollZoomBy(zoom, 1))}
		>
			+
		</button>
		<button class="icon" aria-label="Close piano roll" onclick={() => session.closeRoll()}>
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
						class:down={auditioning === pitch || session.heldPitches.has(pitch)}
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
				class:marquee={marquee !== null}
				class:draw={drawMode}
				role="grid"
				aria-rowcount={PITCH_COUNT}
				aria-label="Notes of {block?.name ?? 'the block'}:
					drag empty grid to marquee select, double-click to add,
					drag note to move, drag edges to resize, wheel for velocity,
					Delete to remove, B to toggle draw mode"
				tabindex="0"
				bind:this={grid}
				style="width: {gridWidth}px; height: {gridHeight}px"
				onpointerdown={onPointerDown}
				onpointermove={onPointerMove}
				onpointerup={onPointerUp}
				onpointercancel={onPointerUp}
				ondblclick={onDblClick}
				onwheel={onWheel}
				onkeydown={onKeyDown}
			>
				<svg class="lines" width={gridWidth} height={gridHeight} aria-hidden="true">
					{#each PITCHES as pitch (pitch)}
						{#if isBlackKey(pitch)}
							<rect
								class="black-row"
								x="0"
								y={pitchToY(pitch)}
								width={gridWidth}
								height={ROW_PX}
							/>
						{/if}
						{#if session.heldPitches.has(pitch)}
							<rect
								class="held-row"
								x="0"
								y={pitchToY(pitch)}
								width={gridWidth}
								height={ROW_PX}
							/>
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
						class:selected={selection.has(index)}
						aria-hidden="true"
						style="left: {beatsToPx(note.startBeats, zoom)}px;
							top: {pitchToY(note.pitch)}px;
							width: {beatsToPx(note.lengthBeats, zoom)}px;
							height: {ROW_PX}px;
							opacity: {velocityOpacity(note.velocity)}"
					></div>
				{/each}

				{#if drawDrag !== null}
					<div
						class="note preview"
						aria-hidden="true"
						style="left: {beatsToPx(drawDrag.startBeats, zoom)}px;
							top: {pitchToY(drawDrag.pitch)}px;
							width: {beatsToPx(drawDrag.lengthBeats, zoom)}px;
							height: {ROW_PX}px;
							opacity: {velocityOpacity(DEFAULT_VELOCITY)}"
					></div>
				{/if}

				{#if marquee !== null && marquee.hasMoved}
					{@const left = Math.min(marquee.startX, marquee.currentX)}
					{@const top = Math.min(marquee.startY, marquee.currentY)}
					{@const width = Math.abs(marquee.currentX - marquee.startX)}
					{@const height = Math.abs(marquee.currentY - marquee.startY)}
					<div
						class="marquee-box"
						style="left: {left}px; top: {top}px; width: {width}px; height: {height}px;"
						aria-hidden="true"
					></div>
				{/if}
			</div>
		</div>
	</div>
</section>

<style>
	.roll {
		flex: 1;
		min-height: 0;
		display: flex;
		flex-direction: column;
		background: var(--bg);
	}

	.bar {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 32px;
		flex: none;
		padding: 0 8px 0 12px;
		border-bottom: 1px solid var(--border-subtle);
		background: var(--surface);
	}

	.info {
		margin-right: auto;
		font-size: 11px;
		color: var(--dim);
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.quantize {
		height: 24px;
		background: transparent;
		border: 1px solid var(--border);
		color: var(--text-subtle);
	}

	.quantize:hover:not(:disabled) {
		filter: none;
		border-color: var(--green);
		color: var(--text);
	}

	.icon {
		width: 26px;
		height: 26px;
		padding: 0;
		background: transparent;
		color: var(--text-subtle);
		font-size: 14px;
		letter-spacing: 0;
	}

	.icon:hover:not(:disabled) {
		filter: none;
		background: var(--surface-hover);
		color: var(--text);
	}

	.icon.pencil.active {
		background: var(--green);
		color: var(--on-green);
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
		background: var(--surface);
		border-right: 1px solid var(--border);
	}

	.key {
		flex: none;
		justify-content: flex-end;
		padding: 0 6px 0 0;
		border: 0;
		border-bottom: 1px solid var(--border-subtle);
		background: var(--surface-hover);
		color: var(--text-subtle);
		font-size: 9px;
		line-height: 1;
		letter-spacing: 0.04em;
		text-transform: none;
		transition: none;
	}

	.key.black {
		background: var(--surface);
	}

	.key:hover:not(:disabled) {
		filter: none;
		background: var(--green-soft);
		color: var(--text);
	}

	.key.down {
		background: var(--green);
		color: var(--on-green);
	}

	.grid {
		position: relative;
		flex: none;
		user-select: none;
		touch-action: none;
	}

	.grid.add {
		cursor: default;
	}

	.grid.draw {
		cursor: crosshair;
	}

	.grid.move {
		cursor: grab;
	}

	.grid.resize-start,
	.grid.resize-end {
		cursor: ew-resize;
	}

	.grid.dragging.move {
		cursor: grabbing;
	}

	.grid.dragging.resize-start,
	.grid.dragging.resize-end {
		cursor: ew-resize;
	}

	.grid.marquee {
		cursor: crosshair;
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
		fill: var(--surface);
		fill-opacity: 0.6;
	}

	.held-row {
		fill: var(--green);
		fill-opacity: 0.28;
	}

	.lines line {
		stroke: var(--grid-minor);
		stroke-width: 1;
	}

	.lines line.bar {
		stroke: var(--grid-major);
	}

	.lines line.octave {
		stroke: var(--border);
	}

	.note {
		position: absolute;
		background: var(--note-color, var(--green));
		box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--note-color, var(--green)) 55%, var(--bg));
		pointer-events: none;
	}

	.note.selected {
		background: color-mix(in srgb, var(--note-color, var(--green)) 85%, var(--text));
		box-shadow: inset 0 0 0 1px var(--bg);
		outline: 1px solid var(--text);
	}

	.note.preview {
		background: var(--note-color, var(--green));
		opacity: 0.6;
		outline: 1px dashed var(--text);
	}

	.marquee-box {
		position: absolute;
		background: color-mix(in srgb, var(--note-color, var(--green)) 18%, transparent);
		border: 1px solid var(--note-color, var(--green));
		pointer-events: none;
		z-index: 3;
	}
</style>
