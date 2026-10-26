/**
 * The one place that owns interface state. It polls the engine once per animation
 * frame, holds the snapshot every component reads, funnels intent back through one
 * queue so a drag or a fader sends at most one message per frame, hands the
 * block drags to the drag controller, and remembers which block the piano roll
 * has open.
 */

import { pickProject, send as sendEvent, sendNow as sendWithoutWaiting, state } from './api';
import { DragController } from './drag.svelte';
import { ZOOM_DEFAULT, zoomBy } from './geometry';
import { attachKeyboard } from './keyboard';
import { NoteCache } from './noteCache';
import { locateBlock } from './pianoRoll';
import { KeyedQueue, nextBackoff } from './queue';
import type { EngineEvent, UiState } from './wire';

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

class Session {
	snapshot = $state<UiState | null>(null);
	error = $state<string | null>(null);
	zoom = $state(ZOOM_DEFAULT);
	scrollRequest = $state(0);
	roll = $state<OpenBlock | null>(null);
	hover = $state<Hover | null>(null);
	drag: DragController;

	private errorFrom: 'poll' | 'send' | null = null;
	private frame = 0;
	private running = false;
	private polling = false;
	private mix = new KeyedQueue<EngineEvent>();
	private voice = new KeyedQueue<EngineEvent>();
	private noteDrag = new KeyedQueue<EngineEvent>();
	private noteVelocity = new KeyedQueue<EngineEvent>();
	private opened = 0;
	private notes = new NoteCache();
	private backoff = 0;
	private resumeAt = 0;

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
		void this.send(event);
	}

	/** Apply one intent and resolve to whether the engine accepted it. */
	request(event: EngineEvent): Promise<boolean> {
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

	/** Open the piano roll on a block, if it is a MIDI block. */
	openRoll(track: number, block: number): void {
		const view = this.snapshot?.tracks[track]?.blocks[block];
		if (view === undefined || view.kind !== 'midi') return;
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

	/** Close the piano roll. */
	closeRoll(): void {
		this.roll = null;
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
	async loadProjectFromDialog(): Promise<void> {
		try {
			const path = await pickProject();
			if (path !== null) await this.send({ loadProject: { path } });
		} catch (error) {
			this.fail(error, 'send');
		}
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
			...this.noteVelocity.drain()
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
