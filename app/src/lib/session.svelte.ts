/**
 * The one place that owns interface state. It polls the engine once per animation
 * frame, holds the snapshot every component reads, funnels intent back through one
 * queue so a drag or a fader sends at most one message per frame, and hands the
 * block drags to the drag controller.
 */

import { pickProject, send as sendEvent, sendNow as sendWithoutWaiting, state } from './api';
import { DragController } from './drag.svelte';
import { ZOOM_DEFAULT, zoomBy } from './geometry';
import { attachKeyboard } from './keyboard';
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
	private mix = new KeyedQueue<EngineEvent>();
	private voice = new KeyedQueue<EngineEvent>();
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
				this.snapshot = await state();
			} catch (error) {
				this.fail(error, 'poll');
				this.backoff = nextBackoff(this.backoff);
				this.resumeAt = performance.now() + this.backoff;
				return;
			}
			this.clear('poll');
			this.backoff = 0;
			this.resumeAt = 0;
			await this.drag.flush();
			await this.flushQueued();
		} finally {
			this.drag.settle();
		}
	}

	private async flushQueued(): Promise<void> {
		for (const event of [...this.mix.drain(), ...this.voice.drain()]) await this.send(event);
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
