/**
 * The Tauri command surface. One function per command, so every invoke payload
 * is built in one place and no component reaches for `invoke` itself.
 */

import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';

import type { EngineEvent, SampleEntry, UiState, Waveform } from './wire';

/**
 * The engine's snapshot, or null in the moment before it first publishes. Tracks
 * whose notes revision appears in `known` at their index come back without notes.
 */
export function state(known: number[]): Promise<UiState | null> {
	return invoke<UiState | null>('state', { known });
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

/** Peak amplitudes in 0..1 for one audio file, and how many seconds it runs. */
export function readWaveform(path: string): Promise<Waveform> {
	return invoke<Waveform>('read_waveform', { path });
}

export function readSampleWav(path: string): Promise<number[]> {
	return invoke<number[]>('read_sample_wav', { path });
}

const waveforms = new Map<string, Promise<Waveform>>();

/** Peaks for a file, cached so a block does not re-read it on every frame. */
export function loadWaveform(path: string): Promise<Waveform> {
	const cached = waveforms.get(path);
	if (cached) return cached;
	const request = readWaveform(path);
	request.catch(() => waveforms.delete(path));
	waveforms.set(path, request);
	return request;
}

/** Ask the user for a project file, or null when they cancel. */
export function pickProject(): Promise<string | null> {
	return open({
		multiple: false,
		directory: false,
		filters: [{ name: 'Project', extensions: ['cassette', 'json'] }]
	});
}

/** Ask where to save a project, or null when they cancel. */
export function pickSaveProject(defaultName?: string): Promise<string | null> {
	return save({
		defaultPath: defaultName,
		filters: [{ name: 'Cassette Project', extensions: ['cassette'] }]
	});
}

/** Ask where to export audio to WAV, or null when they cancel. */
export function pickExportWav(defaultName?: string): Promise<string | null> {
	return save({
		defaultPath: defaultName,
		filters: [{ name: 'WAV Audio', extensions: ['wav'] }]
	});
}

/** Write recorded audio bytes to a WAV file in the temporary directory. */
export function saveRecordedSample(name: string, bytes: Uint8Array | number[]): Promise<string> {
	return invoke<string>('save_recorded_sample', { name, bytes: Array.from(bytes) });
}
