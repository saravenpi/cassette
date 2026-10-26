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

/** One note of a MIDI block; its start is relative to the block. */
export interface NoteView {
	pitch: number;
	startBeats: number;
	lengthBeats: number;
	velocity: number;
}

/** One block on a track's lane, in beats from the top of the song. */
export interface BlockView {
	name: string;
	kind: TrackKind;
	startBeats: number;
	lengthBeats: number;
	source: string;
	selected: boolean;
	notes: NoteView[];
	offsetSeconds: number;
	rate: number;
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
	notesRevision: number;
	blocks: BlockView[];
}

/** The stretch of the timeline the transport loops over, in beats. */
export interface LoopRegion {
	startBeats: number;
	endBeats: number;
}

/** Everything worth drawing this frame. */
export interface UiState {
	bpm: number;
	playing: boolean;
	recording: boolean;
	looping: boolean;
	loopRegion: LoopRegion | null;
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
	canUndo: boolean;
	canRedo: boolean;
	backend: string;
	backendNote: string;
	sampleRate: number;
}

/** One audio file as a block draws it: its peaks and its length in seconds. */
export interface Waveform {
	peaks: number[];
	seconds: number;
}

export type SampleKind = 'dir' | 'wav' | 'audio' | 'project' | 'other';

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
	| { seek: { beats: number } }
	| { setLoopRegion: { startBeats: number; endBeats: number } }
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
	| { splitBlock: { track: number; block: number; atBeats: number } }
	| { duplicateBlock: { track: number; block: number } }
	| { trimBlockStart: { track: number; block: number; startBeats: number } }
	| { stretchBlock: { track: number; block: number; lengthBeats: number } }
	| {
			addNote: {
				track: number;
				block: number;
				pitch: number;
				startBeats: number;
				lengthBeats: number;
				velocity: number;
			};
	  }
	| { moveNote: { track: number; block: number; note: number; pitch: number; startBeats: number } }
	| { resizeNote: { track: number; block: number; note: number; lengthBeats: number } }
	| { setNoteVelocity: { track: number; block: number; note: number; velocity: number } }
	| { deleteNote: { track: number; block: number; note: number } }
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
	| { noteOff: { pitch: number } }
	| 'toggleRecord'
	| { exportAudio: { path: string | null } }
	| { exportProject: { path: string | null } }
	| { loadProject: { path: string | null } }
	| 'undo'
	| 'redo'
	| 'checkpoint';

