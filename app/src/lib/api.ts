/**
 * The Tauri command surface. One function per command, so every invoke payload
 * is built in one place and no component reaches for `invoke` itself.
 */

import { invoke } from '@tauri-apps/api/core';

import type { EngineEvent, SampleEntry, UiState } from './wire';

/** The engine's snapshot, or null in the moment before it first publishes. */
export function state(): Promise<UiState | null> {
	return invoke<UiState | null>('state');
}

/** Apply one intent. Resolves once the engine thread has taken it. */
export function send(event: EngineEvent): Promise<void> {
	return invoke<void>('send', { event });
}

/** Fire an intent without waiting, for note-on and note-off. */
export function sendNow(event: EngineEvent): void {
	void send(event).catch((error) => console.error('cassette: intent rejected', event, error));
}

/** The directory the file browser opens on. */
export function samplesRoot(): Promise<string> {
	return invoke<string>('samples_root');
}

/** One directory of samples; a `dir` of null means the default root. */
export function listSamples(dir: string | null): Promise<SampleEntry[]> {
	return invoke<SampleEntry[]>('list_samples', { dir });
}

/** The parent of a path, or null at the top of the filesystem. */
export function parentOf(path: string): Promise<string | null> {
	return invoke<string | null>('parent_of', { path });
}

/** Peak amplitudes in 0..1 for one audio file. */
export function readWaveform(path: string): Promise<number[]> {
	return invoke<number[]>('read_waveform', { path });
}

const waveforms = new Map<string, Promise<number[]>>();

/** Peaks for a file, cached so a block does not re-read it on every frame. */
export function loadWaveform(path: string): Promise<number[]> {
	const cached = waveforms.get(path);
	if (cached) return cached;
	const request = readWaveform(path);
	request.catch(() => waveforms.delete(path));
	waveforms.set(path, request);
	return request;
}
