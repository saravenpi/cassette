/**
 * The one place that owns interface state. It polls the engine once per animation
 * frame, holds the snapshot every component reads, funnels intent back through one
 * queue so a drag or a fader sends at most one message per frame, hands the
 * block drags to the drag controller, and remembers which block the piano roll
 * has open. Opening a block and picking a track also steer the detail pane.
 */

import {
	pickExportWav,
	pickProject,
	pickSaveProject,
	send as sendEvent,
	sendNow as sendWithoutWaiting,
	state
} from './api';
import { paramKey } from './chain';
import { detail } from './detail.svelte';
import { DragController } from './drag.svelte';
import { DEFAULT_LENGTH_BEATS, ZOOM_DEFAULT, clampZoom, gridStep, snapBeats, zoomBy } from './geometry';
import { attachKeyboard } from './keyboard';
import { NoteCache } from './noteCache';
import { NOTE_GRID, locateBlock } from './pianoRoll';
import { resolveSaveAction, resolveSaveAsAction } from './project';
import { KeyedQueue, nextBackoff } from './queue';
import { selection } from './selection.svelte';
import { menu } from './menu.svelte';
import { TRACK_PALETTE } from './palette';
import { trackSelection, type SelectTrackOptions } from './trackSelection.svelte';
import type { EffectKind, EngineEvent, TrackKind, UiState } from './wire';

/** A track's voice, as the effects pane edits it. */
export interface Voice {
	gain: number;
	octave: number;
	attack: number;
	decay: number;
	sustain: number;
	release: number;
}

/**
 * The MIDI block the piano roll edits, with what identifies it across re-sorts.
 * `serial` changes only when a block is opened, so the pane can tell a fresh
 * open from the same block moving. `seated` turns true once a snapshot shows
 * the cursor on the block's track: the keyboard and the audition keys play the
 * cursor's track, so the roll closes when the cursor leaves it afterwards.
 */
export interface OpenBlock {
	serial: number;
	seated: boolean;
	track: number;
	block: number;
	name: string;
	startBeats: number;
}

/**
 * Where the pointer sits on the timeline: the beat the hover line marks, the
 * lane, and the block under the pointer if there is one.
 */
export interface Hover {
	beats: number;
	track: number;
	block: number | null;
}

function message(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

export interface EffectClipboard {
	kind: EffectKind;
	params: number[];
	enabled: boolean;
	sidechain: number | null;
}

class Session {
	snapshot = $state<UiState | null>(null);
	error = $state<string | null>(null);
	zoom = $state(ZOOM_DEFAULT);
	scrollRequest = $state(0);
	roll = $state<OpenBlock | null>(null);
	hover = $state<Hover | null>(null);
	exporting = $state(false);
	exportProgress = $state<number | null>(null);
	saving = $state(false);
	selectedEffectIndex = $state<number | null>(null);
	effectClipboard = $state<EffectClipboard | null>(null);
	activeView = $state<'timeline' | 'mixer'>('timeline');
	selectedTrack = $state(0);
	drag: DragController;

	private errorFrom: 'poll' | 'send' | null = null;
	private exportActive = false;
	private frame = 0;
	private running = false;
	private polling = false;
	private mix = new KeyedQueue<EngineEvent>();
	private voice = new KeyedQueue<EngineEvent>();
	private noteDrag = new KeyedQueue<EngineEvent>();
	private noteVelocity = new KeyedQueue<EngineEvent>();
	private effectParam = new KeyedQueue<EngineEvent>();
	private instrumentTone = new KeyedQueue<EngineEvent>();
	private opened = 0;
	private notes = new NoteCache();
	private backoff = 0;
	private resumeAt = 0;
	private lastCreatedBlock: { track: number; startBeats: number; at: number } | null = null;

	constructor() {
		this.drag = new DragController({
			snapshot: () => this.snapshot,
			zoom: () => this.zoom,
			send: (event) => this.send(event)
		});
	}

	get renaming(): boolean {
		return this.snapshot?.renaming ?? false;
	}

	get octave(): number {
		return this.snapshot?.octave ?? 0;
	}

	/** Begin polling the engine once per animation frame. */
	start(): void {
		if (this.running) return;
		this.running = true;
		const tick = () => {
			this.frame = requestAnimationFrame(tick);
			if (this.polling || performance.now() < this.resumeAt) return;
			this.polling = true;
			void this.step().finally(() => {
				this.polling = false;
			});
		};
		this.frame = requestAnimationFrame(tick);
	}

	/** Stop polling. */
	stop(): void {
		this.running = false;
		cancelAnimationFrame(this.frame);
		this.frame = 0;
	}

	/** Wire the keymap to the window. Returns the teardown. */
	attachKeyboard(): () => void {
		return attachKeyboard(this);
	}

	/** Apply one intent. */
	dispatch(event: EngineEvent): void {
		if (this.exporting && (event === 'togglePlay' || event === 'toggleRecord')) return;
		if (typeof event === 'object' && 'selectTrack' in event) {
			this.selectedTrack = event.selectTrack.track;
		}
		if (typeof event === 'object' && 'reorderTrack' in event) {
			trackSelection.reorderTrack(event.reorderTrack.from, event.reorderTrack.to);
		}
		void this.send(event);
	}

	/** Apply one intent and resolve to whether the engine accepted it. */
	request(event: EngineEvent): Promise<boolean> {
		if (typeof event === 'object' && 'deleteTrack' in event) {
			const cursorTrack = this.snapshot?.cursor.track;
			if (
				trackSelection.selectedTracks.size > 1 &&
				((cursorTrack !== undefined && trackSelection.selectedTracks.has(cursorTrack)) ||
					trackSelection.selectedTracks.has(event.deleteTrack.track))
			) {
				this.deleteSelectedTracks();
				return Promise.resolve(true);
			}
		}
		if (typeof event === 'object' && 'duplicateTrack' in event) {
			if (
				trackSelection.selectedTracks.size > 1 &&
				trackSelection.selectedTracks.has(event.duplicateTrack.track)
			) {
				this.duplicateSelectedTracks();
				return Promise.resolve(true);
			}
		}
		if (typeof event === 'object' && 'reorderTrack' in event) {
			trackSelection.reorderTrack(event.reorderTrack.from, event.reorderTrack.to);
		}
		return this.send(event);
	}

	/** Show a failure in the status bar until the next successful send. */
	report(text: string): void {
		this.fail(text, 'send');
	}

	/** Apply one intent without waiting, for note-on and note-off. */
	sendNow(event: EngineEvent): void {
		sendWithoutWaiting(event);
	}

	/** Send a mixer strip change, collapsed to one message per frame. */
	queueMix(index: number, volume: number, pan: number, muted: boolean, solo: boolean): void {
		this.mix.set(index, { setTrackMix: { track: index, volume, pan, muted, solo } });
	}

	/** Send a voice change, collapsed to one message per frame. */
	queueVoice(index: number, voice: Voice): void {
		this.voice.set(index, { setInstrument: { track: index, ...voice } });
	}

	/** Send an effect parameter change, collapsed to one message per frame per parameter. */
	queueEffectParam(track: number, index: number, param: number, value: number): void {
		this.effectParam.set(paramKey(track, index, param), {
			setEffectParam: { track, index, param, value }
		});
	}

	/** Add an effect to a track's chain at `index`, or at the end when it is null. */
	addEffect(track: number, kind: EffectKind, index: number | null = null): void {
		this.dispatch({ addEffect: { track, kind, index } });
	}

	/** Select a track from its header, and show its devices if the detail pane is open. */
	selectTrack(track: number, options?: SelectTrackOptions): void {
		this.selectedTrack = track;
		if (track !== 999) {
			if (this.snapshot?.cursor.track !== track) {
				this.selectedEffectIndex = null;
			}
			if (options) {
				trackSelection.selectTrack(track, {
					...options,
					totalTracks: this.snapshot?.tracks.length ?? 1
				});
			} else if (!trackSelection.isSelected(track)) {
				trackSelection.selectTrack(track, {
					totalTracks: this.snapshot?.tracks.length ?? 1
				});
			}
		}
		this.dispatch({ selectTrack: { track } });
		detail.followTrack();
	}

	selectNextTrack(): void {
		const total = this.snapshot?.tracks.length ?? 0;
		if (total === 0) return;
		if (this.selectedTrack === 999) return;
		if (this.selectedTrack >= total - 1) {
			this.selectTrack(999);
		} else {
			this.selectTrack(this.selectedTrack + 1);
		}
	}

	selectPreviousTrack(): void {
		const total = this.snapshot?.tracks.length ?? 0;
		if (total === 0) return;
		if (this.selectedTrack === 999) {
			this.selectTrack(total - 1);
		} else if (this.selectedTrack > 0) {
			this.selectTrack(Math.max(0, this.selectedTrack - 1));
		}
	}

	selectAllTracks(): void {
		const total = this.snapshot?.tracks.length ?? 1;
		trackSelection.selectAll(total);
	}

	clearTrackSelection(activeTrack?: number): void {
		const active = activeTrack ?? this.snapshot?.cursor.track ?? 0;
		trackSelection.clear(active);
	}

	copyEffect(trackIndex?: number, effectIndex?: number): void {
		const track = trackIndex ?? this.snapshot?.cursor.track;
		const at = effectIndex ?? this.selectedEffectIndex;
		if (track === undefined || at === null || at === undefined) return;
		const effect = this.snapshot?.tracks[track]?.effects[at];
		if (!effect) return;
		this.effectClipboard = {
			kind: effect.kind,
			params: [...effect.params],
			enabled: effect.enabled,
			sidechain: effect.sidechain
		};
	}

	pasteEffect(trackIndex?: number, effectIndex?: number | null): void {
		const clip = this.effectClipboard;
		if (clip === null) return;
		const track = trackIndex ?? this.snapshot?.cursor.track;
		if (track === undefined) return;
		const trackView = this.snapshot?.tracks[track];
		if (!trackView || trackView.effects.length >= 8) return;
		const at = effectIndex !== undefined ? effectIndex : this.selectedEffectIndex;
		this.dispatch({
			pasteEffect: {
				track,
				index: at,
				kind: clip.kind,
				params: [...clip.params],
				enabled: clip.enabled,
				sidechain: clip.sidechain
			}
		});
		const newIndex =
			at !== null && at !== undefined
				? Math.min(trackView.effects.length, at + 1)
				: trackView.effects.length;
		this.selectedEffectIndex = newIndex;
	}

	duplicateEffect(trackIndex?: number, effectIndex?: number): void {
		const track = trackIndex ?? this.snapshot?.cursor.track;
		const at = effectIndex ?? this.selectedEffectIndex;
		if (track === undefined || at === null || at === undefined) return;
		const trackView = this.snapshot?.tracks[track];
		if (!trackView || at >= trackView.effects.length || trackView.effects.length >= 8) return;
		this.dispatch({
			duplicateEffect: {
				track,
				index: at
			}
		});
		this.selectedEffectIndex = Math.min(trackView.effects.length, at + 1);
	}

	deselectEffect(): void {
		this.selectedEffectIndex = null;
	}

	openTrackRecolor(): void {
		const snapshot = this.snapshot;
		if (snapshot === null) return;
		const trackIndex = snapshot.cursor.track;
		const track = snapshot.tracks[trackIndex];
		if (!track) return;
		const header = document.querySelector(`[data-track-header="${trackIndex}"]`);
		const rect = header?.getBoundingClientRect();
		const x = rect ? rect.left + 20 : window.innerWidth / 2 - 100;
		const y = rect ? rect.top + 26 : window.innerHeight / 2 - 50;
		menu.open(x, y, [
			{ kind: 'heading', label: `Color: ${track.name}` },
			{
				kind: 'colors',
				colors: TRACK_PALETTE,
				selected: track.colorIndex,
				pick: (colorIndex) => void this.request({ setTrackColor: { track: trackIndex, colorIndex } })
			}
		]);
	}

	/** Switch the detail pane between Clip and Devices, opening it if hidden. */
	toggleDetail(): void {
		detail.toggle();
	}

	/**
	 * Quantize note starts: the open roll's block to the roll's grid, or else the
	 * selected MIDI block to the timeline grid.
	 */
	quantizeSelected(): void {
		const roll = this.roll;
		if (roll !== null) {
			this.dispatch({ quantizeBlock: { track: roll.track, block: roll.block, gridBeats: NOTE_GRID, notes: [] } });
			return;
		}
		const cursor = this.snapshot?.cursor;
		const view = cursor === undefined ? undefined : this.snapshot?.tracks[cursor.track]?.blocks[cursor.block];
		if (cursor === undefined || view?.kind !== 'midi') {
			this.report('select a MIDI block to quantize it');
			return;
		}
		this.dispatch({
			quantizeBlock: { track: cursor.track, block: cursor.block, gridBeats: gridStep(this.zoom), notes: [] }
		});
	}

	/** Open the piano roll on a block, if it is a MIDI block, and show the Clip view. */
	openRoll(track: number, block: number): void {
		const view = this.snapshot?.tracks[track]?.blocks[block];
		if (view === undefined || view.kind !== 'midi') return;
		detail.showClip();
		this.opened += 1;
		this.roll = {
			serial: this.opened,
			seated: this.snapshot?.cursor.track === track,
			track,
			block,
			name: view.name,
			startBeats: view.startBeats
		};
	}

	/** Open the piano roll on the block under the cursor. */
	openSelectedRoll(): void {
		const cursor = this.snapshot?.cursor;
		if (cursor !== undefined) this.openRoll(cursor.track, cursor.block);
	}

	/** Create a MIDI block on `track` at `startBeats`, deduplicating rapid double-clicks. */
	createMidiBlockAt(track: number, startBeats: number, lengthBeats = DEFAULT_LENGTH_BEATS): void {
		const now = performance.now();
		if (
			this.lastCreatedBlock !== null &&
			this.lastCreatedBlock.track === track &&
			Math.abs(this.lastCreatedBlock.startBeats - startBeats) < 0.01 &&
			now - this.lastCreatedBlock.at < 400
		) {
			return;
		}
		this.lastCreatedBlock = { track, startBeats, at: now };
		this.dispatch({
			createBlock: {
				track,
				startBeats,
				lengthBeats
			}
		});
	}

	/** Cut the block under the cursor in two at the playhead. */
	splitSelected(): void {
		const snapshot = this.snapshot;
		if (snapshot === null) return;
		const { track, block } = snapshot.cursor;
		const view = snapshot.tracks[track]?.blocks[block];
		const at = snapshot.transportPositionBeats;
		if (view === undefined || at <= view.startBeats || at >= view.startBeats + view.lengthBeats) {
			this.report('move the playhead inside the selected block to split it');
			return;
		}
		this.dispatch({ splitBlock: { track, block, atBeats: at } });
	}

	/**
	 * Cut the block under the pointer at the hover line, the way the line shows
	 * it. With the pointer off every block, cut the selected block at the
	 * playhead instead.
	 */
	splitAtPointer(): void {
		const hover = this.hover;
		if (hover === null || hover.block === null) {
			this.splitSelected();
			return;
		}
		this.dispatch({ splitBlock: { track: hover.track, block: hover.block, atBeats: hover.beats } });
	}

	/** Copy the block under the cursor onto the beat right after it. */
	duplicateSelected(): void {
		const cursor = this.snapshot?.cursor;
		if (cursor === undefined) return;
		if (this.snapshot?.tracks[cursor.track]?.blocks[cursor.block] === undefined) return;
		this.dispatch({ duplicateBlock: { track: cursor.track, block: cursor.block } });
	}

	/** Copy the current time selection to the clipboard. */
	copySelection(): void {
		if (selection.range !== null) {
			this.dispatch({ copyRange: { range: selection.range } });
		} else {
			const cursor = this.snapshot?.cursor;
			if (cursor !== undefined) {
				const block = this.snapshot?.tracks[cursor.track]?.blocks[cursor.block];
				if (block !== undefined) {
					this.dispatch({
						copyRange: {
							range: {
								firstTrack: cursor.track,
								lastTrack: cursor.track,
								startBeats: block.startBeats,
								endBeats: block.startBeats + block.lengthBeats
							}
						}
					});
				}
			}
		}
	}

	/** Cut the current time selection to the clipboard and clear the selection. */
	cutSelection(): void {
		if (selection.range !== null) {
			this.dispatch({ cutRange: { range: selection.range } });
			selection.clear();
		} else {
			const cursor = this.snapshot?.cursor;
			if (cursor !== undefined) {
				const block = this.snapshot?.tracks[cursor.track]?.blocks[cursor.block];
				if (block !== undefined) {
					this.dispatch({
						cutRange: {
							range: {
								firstTrack: cursor.track,
								lastTrack: cursor.track,
								startBeats: block.startBeats,
								endBeats: block.startBeats + block.lengthBeats
							}
						}
					});
				}
			}
		}
	}

	/** Move the selected block by tracks and beats, returning true if moved. */
	nudgeSelectedBlock(deltaTrack: number, deltaBeats: number): boolean {
		const cursor = this.snapshot?.cursor;
		if (cursor === undefined) return false;
		const track = cursor.track;
		const blockIndex = cursor.block;
		const blocks = this.snapshot?.tracks[track]?.blocks ?? [];
		if (blockIndex >= blocks.length) return false;
		const current = blocks[blockIndex];
		if (!current) return false;
		const tracksCount = this.snapshot?.tracks.length ?? 1;
		const targetTrack = Math.max(0, Math.min(tracksCount - 1, track + deltaTrack));
		const step = deltaBeats * gridStep(this.zoom);
		const startBeats = Math.max(0, current.startBeats + step);
		this.dispatch({
			placeBlock: {
				track,
				block: blockIndex,
				targetTrack,
				startBeats
			}
		});
		return true;
	}

	/** Seek playhead by relative direction in grid steps. */
	seekCursor(direction: number): void {
		const pos = this.snapshot?.transportPositionBeats ?? 0;
		const step = direction * gridStep(this.zoom);
		const next = Math.max(0, snapBeats(pos + step, gridStep(this.zoom)));
		this.dispatch({ seek: { beats: next } });
	}

	/** Paste clipboard contents at the marker on the selected track. */
	pasteSelection(): void {
		const track = this.snapshot?.cursor.track ?? 0;
		const startBeats = this.snapshot?.transportPositionBeats ?? 0;
		this.dispatch({ pasteRange: { track, startBeats } });
	}

	/** Duplicate either the active time selection or the selected block. */
	duplicate(): void {
		if (selection.range !== null) {
			this.dispatch({ duplicateRange: { range: selection.range } });
		} else {
			this.duplicateSelected();
		}
	}

	/** Delete either the active time selection or the block under the cursor. */
	delete(): void {
		if (selection.range !== null) {
			this.dispatch({ deleteRange: { range: selection.range } });
			selection.clear();
			return;
		}
		const cursorTrack = this.snapshot?.cursor.track;
		if (
			trackSelection.selectedTracks.size > 1 &&
			cursorTrack !== undefined &&
			trackSelection.selectedTracks.has(cursorTrack)
		) {
			this.deleteSelectedTracks();
			return;
		}
		this.dispatch('deleteBlock');
	}

	/** Toggle or set looping over the active selection or song. */
	loopSelection(): void {
		const range = selection.range;
		const snapshot = this.snapshot;
		if (range !== null && range.startBeats < range.endBeats) {
			const region = snapshot?.loopRegion;
			const matches =
				region !== null &&
				region !== undefined &&
				region.startBeats === range.startBeats &&
				region.endBeats === range.endBeats;
			if (snapshot?.looping && matches) {
				this.dispatch('toggleLoop');
				return;
			}
			this.dispatch({
				setLoopRegion: { startBeats: range.startBeats, endBeats: range.endBeats }
			});
			if (!snapshot?.looping) {
				this.dispatch('toggleLoop');
			}
		} else {
			if (snapshot?.looping) {
				this.dispatch('toggleLoop');
			}
			this.dispatch({ setLoopRegion: { startBeats: 0, endBeats: 0 } });
		}
	}

	/** Add a new track of the given kind if below the maximum. */
	addTrack(kind: TrackKind): void {
		if ((this.snapshot?.tracks.length ?? 0) < 16) {
			this.dispatch({ addTrack: { kind } });
		}
	}

	addMidiTrack(): void {
		this.addTrack('midi');
	}

	addAudioTrack(): void {
		this.addTrack('audio');
	}

	/** Delete a track if more than one track remains. */
	deleteTrack(track: number): void {
		const total = this.snapshot?.tracks.length ?? 0;
		if (total <= 1) return;
		const cursorTrack = this.snapshot?.cursor.track;
		if (
			trackSelection.selectedTracks.size > 1 &&
			((cursorTrack !== undefined && trackSelection.selectedTracks.has(cursorTrack)) ||
				trackSelection.selectedTracks.has(track))
		) {
			this.deleteSelectedTracks();
			return;
		}
		this.dispatch({ deleteTrack: { track } });
		trackSelection.deleteTracks([track]);
		if (trackSelection.selectedTracks.size === 0) {
			const remaining = Math.max(1, total - 1);
			const target = Math.max(0, Math.min(track, remaining - 1));
			trackSelection.clear(target);
		}
	}

	deleteSelectedTracks(): void {
		const currentCount = this.snapshot?.tracks.length ?? 0;
		if (currentCount <= 1) return;
		const tracks = Array.from(trackSelection.selectedTracks).sort((a, b) => b - a);
		const maxDeletions = currentCount - 1;
		const toDelete = tracks.slice(0, maxDeletions);
		if (toDelete.length === 0) return;
		this.dispatch('checkpoint');
		for (const t of toDelete) {
			this.dispatch({ deleteTrack: { track: t } });
		}
		trackSelection.deleteTracks(toDelete);
		if (trackSelection.selectedTracks.size === 0) {
			const remaining = Math.max(1, currentCount - toDelete.length);
			const target = Math.max(0, Math.min(toDelete[toDelete.length - 1], remaining - 1));
			trackSelection.clear(target);
			this.dispatch({ selectTrack: { track: target } });
		}
	}

	/** Duplicate a track if below the maximum track count. */
	duplicateTrack(track: number): void {
		const total = this.snapshot?.tracks.length ?? 0;
		if (total >= 16) return;
		if (
			trackSelection.selectedTracks.size > 1 &&
			trackSelection.isSelected(track)
		) {
			this.duplicateSelectedTracks();
			return;
		}
		this.dispatch({ duplicateTrack: { track } });
	}

	duplicateSelectedTracks(): void {
		let total = this.snapshot?.tracks.length ?? 0;
		if (total >= 16) return;
		this.dispatch('checkpoint');
		const tracks = Array.from(trackSelection.selectedTracks).sort((a, b) => b - a);
		for (const t of tracks) {
			if (total >= 16) break;
			this.dispatch({ duplicateTrack: { track: t } });
			total += 1;
		}
	}

	reorderTrack(from: number, to: number): void {
		trackSelection.reorderTrack(from, to);
		this.dispatch({ reorderTrack: { from, to } });
	}

	/** Rename a track. */
	renameTrack(track: number, name: string): void {
		this.dispatch({ renameTrack: { track, name } });
	}

	/** Queue an instrument tone change, collapsed to one message per frame. */
	queueInstrumentTone(
		track: number,
		cutoff: number,
		resonance: number,
		detune: number,
		sub: number
	): void {
		this.instrumentTone.set(track, {
			setInstrumentTone: { track, cutoff, resonance, detune, sub }
		});
	}

	/** Set an effect's sidechain audio source track, or null for self. */
	setEffectSidechain(track: number, index: number, source: number | null): void {
		this.dispatch({ setEffectSidechain: { track, index, source } });
	}

	/** Close the piano roll. */
	closeRoll(): void {
		this.roll = null;
	}

	/** Clear the active time selection, returning true if one was cleared. */
	clearSelection(): boolean {
		if (selection.range !== null) {
			selection.clear();
			return true;
		}
		return false;
	}

	/** Move a note of the open block, collapsed to one message per frame. */
	queueNoteMove(note: number, pitch: number, startBeats: number): void {
		const roll = this.roll;
		if (roll === null) return;
		this.noteDrag.set(note, {
			moveNote: { track: roll.track, block: roll.block, note, pitch, startBeats }
		});
	}

	/** Resize a note of the open block, collapsed to one message per frame. */
	queueNoteResize(note: number, lengthBeats: number): void {
		const roll = this.roll;
		if (roll === null) return;
		this.noteDrag.set(note, {
			resizeNote: { track: roll.track, block: roll.block, note, lengthBeats }
		});
	}

	/** Set a note's velocity on the open block, collapsed to one message per frame. */
	queueNoteVelocity(note: number, velocity: number): void {
		const roll = this.roll;
		if (roll === null) return;
		this.noteVelocity.set(note, {
			setNoteVelocity: { track: roll.track, block: roll.block, note, velocity }
		});
	}

	/** Ask for a project file and load it. */
	async loadProjectFromDialog(pick: () => Promise<string | null> = pickProject): Promise<void> {
		try {
			const path = await pick();
			if (path !== null) {
				await this.send({ loadProject: { path } });
				trackSelection.clear(0);
			}
		} catch (error) {
			this.fail(error, 'send');
		}
	}

	/** Save the project to its remembered path, or ask for one if never saved. */
	async saveProject(pick: (name?: string) => Promise<string | null> = pickSaveProject): Promise<void> {
		try {
			this.saving = true;
			try {
				const event = await resolveSaveAction(this.snapshot?.projectPath ?? null, () => pick());
				if (event !== null) await this.send(event);
			} finally {
				this.saving = false;
			}
		} catch (error) {
			this.fail(error, 'send');
		}
	}

	/** Save the project under a new path selected via dialog. */
	async saveProjectAs(pick: (name?: string) => Promise<string | null> = pickSaveProject): Promise<void> {
		try {
			this.saving = true;
			try {
				const event = await resolveSaveAsAction(() => pick());
				if (event !== null) await this.send(event);
			} finally {
				this.saving = false;
			}
		} catch (error) {
			this.fail(error, 'send');
		}
	}

	/** Export audio to a WAV file picked from a dialog. */
	async exportAudio(pick: (name?: string) => Promise<string | null> = pickExportWav): Promise<void> {
		try {
			const path = await pick();
			if (path === null) return;
			this.exporting = true;
			this.exportProgress = 0;
			if (this.snapshot?.playing) {
				await this.send('stop');
			}
			const sent = await this.send({ exportAudio: { path } });
			if (!sent) {
				this.exporting = false;
				this.exportProgress = null;
			}
		} catch (error) {
			this.exporting = false;
			this.exportProgress = null;
			this.fail(error, 'send');
		}
	}

	/** Set the zoom to a scale, clamped to the zoom range. */
	zoomTo(pxPerBeat: number): void {
		this.zoom = clampZoom(pxPerBeat);
	}

	/** Move one zoom press in the given direction. */
	setZoom(direction: number): void {
		this.zoom = zoomBy(this.zoom, direction);
	}

	/** Ask the timeline to scroll back to the top of the song. */
	requestScrollStart(): void {
		this.scrollRequest += 1;
	}

	private async send(event: EngineEvent): Promise<boolean> {
		try {
			await sendEvent(event);
			this.clear('send');
			return true;
		} catch (error) {
			this.fail(error, 'send');
			return false;
		}
	}

	private async step(): Promise<void> {
		try {
			try {
				const next = await state(this.notes.known());
				this.snapshot = next === null ? null : this.notes.merge(next);
			} catch (error) {
				this.fail(error, 'poll');
				this.backoff = nextBackoff(this.backoff);
				this.resumeAt = performance.now() + this.backoff;
				return;
			}
			this.clear('poll');
			if (this.snapshot) {
				const count = this.snapshot.tracks.length;
				let changed = false;
				const valid = new Set<number>();
				for (const t of trackSelection.selectedTracks) {
					if (t < count) {
						valid.add(t);
					} else {
						changed = true;
					}
				}
				if (valid.size === 0) {
					const target = Math.max(0, Math.min(this.snapshot.cursor.track, count - 1));
					trackSelection.clear(target);
				} else if (changed) {
					trackSelection.selectedTracks = valid;
				}
				if (this.selectedTrack === 999) {
					if (this.activeView === 'timeline') {
						this.selectedTrack = Math.max(0, Math.min(this.snapshot.cursor.track, count - 1));
					}
				} else if (this.selectedTrack >= count) {
					this.selectedTrack = Math.max(0, count - 1);
				} else if (this.activeView === 'timeline' && this.selectedTrack !== this.snapshot.cursor.track) {
					this.selectedTrack = this.snapshot.cursor.track;
				}
			}
			if (this.snapshot?.exportProgress !== undefined && this.snapshot?.exportProgress !== null) {
				this.exportProgress = this.snapshot.exportProgress;
				this.exporting = true;
				this.exportActive = true;
			} else if (
				this.exportActive ||
				(this.exporting &&
					(this.snapshot?.status?.startsWith('exported audio') ||
						this.snapshot?.status?.startsWith('export audio failed')))
			) {
				this.exportProgress = null;
				this.exporting = false;
				this.exportActive = false;
			}
			this.followRoll();
			this.backoff = 0;
			this.resumeAt = 0;
			await this.drag.flush();
			await this.flushQueued();
		} finally {
			this.drag.settle();
		}
	}

	private async flushQueued(): Promise<void> {
		const queued = [
			...this.mix.drain(),
			...this.voice.drain(),
			...this.noteDrag.drain(),
			...this.noteVelocity.drain(),
			...this.effectParam.drain(),
			...this.instrumentTone.drain()
		];
		for (const event of queued) await this.send(event);
	}

	/**
	 * Keep the piano roll on its block as the lane re-sorts, and close it once the
	 * block is gone or the cursor has left its track.
	 */
	private followRoll(): void {
		let roll = this.roll;
		const snapshot = this.snapshot;
		if (roll === null || snapshot === null) return;
		const onTrack = snapshot.cursor.track === roll.track;
		if (roll.seated && !onTrack) {
			this.roll = null;
			return;
		}
		if (!roll.seated && onTrack) {
			roll = { ...roll, seated: true };
			this.roll = roll;
		}
		const blocks = snapshot.tracks[roll.track]?.blocks ?? [];
		const index = locateBlock(blocks, roll.block, roll.name, roll.startBeats);
		if (index === null) {
			this.roll = null;
			return;
		}
		const view = blocks[index];
		const same = index === roll.block && view.name === roll.name;
		if (same && view.startBeats === roll.startBeats) return;
		this.roll = { ...roll, block: index, name: view.name, startBeats: view.startBeats };
	}

	private fail(error: unknown, from: 'poll' | 'send'): void {
		this.error = message(error);
		this.errorFrom = from;
	}

	private clear(from: 'poll' | 'send'): void {
		if (this.errorFrom !== from) return;
		this.error = null;
		this.errorFrom = null;
	}
}

export const session = new Session();
