/**
 * The wire contract, mirroring `src/ui/view.rs` and `src/events/types.rs`. Rust
 * serializes with `rename_all = "camelCase"`, so these names match exactly.
 */

export type TrackKind = 'midi' | 'audio';

/** Which block of which track the engine's cursor sits on. */
export interface Cursor {
	block: number;
	track: number;
}

/** One block on a track's lane, in beats from the top of the song. */
export interface BlockView {
	name: string;
	kind: TrackKind;
	startBeats: number;
	lengthBeats: number;
	source: string;
	selected: boolean;
}

/** A track: its voice, its mixer strip, and its blocks. */
export interface TrackView {
	name: string;
	kind: TrackKind;
	preset: string;
	waveform: string;
	gain: number;
	octave: number;
	attack: number;
	decay: number;
	sustain: number;
	release: number;
	volume: number;
	pan: number;
	muted: boolean;
	solo: boolean;
	level: number;
	colorIndex: number;
	blocks: BlockView[];
}

/** Everything worth drawing this frame. */
export interface UiState {
	bpm: number;
	playing: boolean;
	looping: boolean;
	transportPositionBeats: number;
	lengthBeats: number;
	bars: number;
	beatsPerBar: number;
	cursor: Cursor;
	octave: number;
	presets: string[];
	tracks: TrackView[];
	status: string;
	renaming: boolean;
	backend: string;
	backendNote: string;
	sampleRate: number;
}

export type SampleKind = 'dir' | 'wav' | 'other';

/** One entry the file browser can show. */
export interface SampleEntry {
	name: string;
	path: string;
	kind: SampleKind;
	size: number;
	modifiedMs: number;
}

/**
 * Serde's default external tagging: unit variants are bare strings, newtype and
 * struct variants are single-key objects keyed by the camelCase variant name.
 */
export type EngineEvent =
	| 'togglePlay'
	| 'stop'
	| 'toggleLoop'
	| 'addBlock'
	| 'deleteBlock'
	| 'cycleTrackKind'
	| 'renameBlock'
	| 'textBackspace'
	| 'cancelRename'
	| 'toggleMute'
	| 'toggleSolo'
	| { moveTrack: number }
	| { moveBlock: number }
	| { selectTrack: { track: number } }
	| { selectBlock: { track: number; block: number } }
	| { createBlock: { track: number; startBeats: number; lengthBeats: number } }
	| { placeBlock: { track: number; block: number; targetTrack: number; startBeats: number } }
	| { resizeBlock: { track: number; block: number; lengthBeats: number } }
	| { deleteBlockAt: { track: number; block: number } }
	| { importSample: { track: number; path: string; startBeats: number } }
	| { textInput: string }
	| { volumeDelta: number }
	| { panDelta: number }
	| { bpmDelta: number }
	| { octaveDelta: number }
	| { setTrackMix: { track: number; volume: number; pan: number; muted: boolean; solo: boolean } }
	| { setPreset: { track: number; preset: string } }
	| {
			setInstrument: {
				track: number;
				gain: number;
				octave: number;
				attack: number;
				decay: number;
				sustain: number;
				release: number;
			};
	  }
	| { noteOn: { pitch: number; velocity: number } }
	| { noteOff: { pitch: number } };
