// The wire contract, mirroring `src/ui/view.rs` and `src/events/types.rs`.
// Rust serializes with `rename_all = "camelCase"`, so these names match exactly.

export type TrackKind = 'midi' | 'audio';

export interface Cursor {
	block: number;
	track: number;
}

export interface TrackView {
	name: string;
	kind: TrackKind;
	preset: string;
	waveform: string;
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
	blocks: string[];
	addSlot: boolean;
	colorIndex: number;
}

export interface UiState {
	bpm: number;
	playing: boolean;
	looping: boolean;
	transportPositionBeats: number;
	cursor: Cursor;
	octave: number;
	tracks: TrackView[];
	status: string;
	renaming: boolean;
	backend: string;
	backendNote: string;
	sampleRate: number;
}

// Serde's default external tagging: unit variants are bare strings, newtype and
// struct variants are single-key objects.
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
	| { volumeDelta: number }
	| { panDelta: number }
	| { bpmDelta: number }
	| { octaveDelta: number }
	| { textInput: string }
	| { noteOn: { pitch: number; velocity: number } }
	| { noteOff: { pitch: number } }
	| { oscControl: { address: string; value: number } };