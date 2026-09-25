/**
 * The one place that owns interface state. It polls the engine once per animation
 * frame, holds the snapshot every component reads, funnels intent back through one
 * queue so a drag or a fader sends at most one message per frame, and hands the
 * block drags to the drag controller.
 */

import { send as sendEvent, sendNow as sendWithoutWaiting, state } from './api';
import { DragController } from './drag.svelte';
import { ZOOM_DEFAULT, zoomBy } from './geometry';
import { attachKeyboard } from './keyboard';
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

function message(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

class Session {
	snapshot = $state<UiState | null>(null);
	error = $state<string | null>(null);
	zoom = $state(ZOOM_DEFAULT);
	scrollRequest = $state(0);
	drag: DragController;

	private errorFrom: 'poll' | 'send' | null = null;
	private frame = 0;
	private running = false;
	private polling = false;
	private mix: EngineEvent | null = null;
	private voice: EngineEvent | null = null;

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
			if (this.polling) return;
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

	/** Apply one intent without waiting, for note-on and note-off. */
	sendNow(event: EngineEvent): void {
		sendWithoutWaiting(event);
	}

	/** Send a mixer strip change, collapsed to one message per frame. */
	queueMix(index: number, volume: number, pan: number, muted: boolean, solo: boolean): void {
		this.mix = { setTrackMix: { track: index, volume, pan, muted, solo } };
	}

	/** Send a voice change, collapsed to one message per frame. */
	queueVoice(index: number, voice: Voice): void {
		this.voice = { setInstrument: { track: index, ...voice } };
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
			this.snapshot = await state();
			this.clear('poll');
		} catch (error) {
			this.fail(error, 'poll');
			return;
		}
		await this.drag.flush();
		await this.flushQueued();
		this.drag.settle();
	}

	private async flushQueued(): Promise<void> {
		const queued: EngineEvent[] = [];
		if (this.mix !== null) {
			queued.push(this.mix);
			this.mix = null;
		}
		if (this.voice !== null) {
			queued.push(this.voice);
			this.voice = null;
		}
		for (const event of queued) await this.send(event);
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
