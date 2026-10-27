/**
 * The wire contract, mirroring `src/ui/view.rs` and `src/events/types.rs`. Rust
 * serializes with `rename_all = "camelCase"`, so these names match exactly.
 */

export type TrackKind = 'midi' | 'audio';

/** Sentinel track index representing the master bus. */
export const MASTER_TRACK = 999;

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

export type EffectKind =
	| 'eq'
	| 'filter'
	| 'saturator'
	| 'compressor'
	| 'delay'
	| 'reverb'
	| 'chorus'
	| 'phaser'
	| 'autoPan'
	| 'bitCrusher'
	| 'utility'
	| 'limiter';

export type ParamUnit =
	| 'decibels'
	| 'hertz'
	| 'milliseconds'
	| 'beats'
	| 'ratio'
	| 'percent'
	| 'choice';

/** One effect in a track's chain; `params` follows its kind's spec. */
export interface EffectView {
	id: number;
	kind: EffectKind;
	name: string;
	enabled: boolean;
	params: number[];
	sidechain: number | null;
}

/** One parameter of an effect kind. A choice indexes `choices`. */
export interface ParamSpecView {
	name: string;
	min: number;
	max: number;
	default: number;
	unit: ParamUnit;
	log: boolean;
	choices: string[];
}

/** One effect kind the engine builds in. */
export interface EffectSpecView {
	kind: EffectKind;
	name: string;
	takesSidechain: boolean;
	params: ParamSpecView[];
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
	effects: EffectView[];
	instrumentEnabled: boolean;
	cutoff: number;
	resonance: number;
	detune: number;
	sub: number;
}

/** A rectangle of the arrangement: a run of tracks and a span of beats. */
export interface BeatRange {
	firstTrack: number;
	lastTrack: number;
	startBeats: number;
	endBeats: number;
}

/** The new values of one note in a batch edit. */
export interface NoteEdit {
	note: number;
	pitch: number;
	startBeats: number;
	lengthBeats: number;
	velocity: number;
}

/** What the clipboard holds. */
export interface ClipboardView {
	tracks: number;
	lengthBeats: number;
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
	metronome: boolean;
	countInBars: number;
	countInBeatsLeft: number;
	recordQuantizeBeats: number;
	looping: boolean;
	loopRegion: LoopRegion | null;
	transportPositionBeats: number;
	lengthBeats: number;
	bars: number;
	beatsPerBar: number;
	cursor: Cursor;
	octave: number;
	presets: string[];
	effectCatalog: EffectSpecView[];
	tracks: TrackView[];
	status: string;
	renaming: boolean;
	canUndo: boolean;
	canRedo: boolean;
	backend: string;
	backendNote: string;
	sampleRate: number;
	projectPath: string | null;
	projectDirty: boolean;
	clipboard: ClipboardView | null;
	exportProgress?: number | null;
	masterVolume?: number;
	masterLevel?: number;
	masterEffects?: EffectView[];
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
	| 'toggleMetronome'
	| { setCountIn: { bars: number } }
	| { setRecordQuantize: { gridBeats: number } }
	| { quantizeBlock: { track: number; block: number; gridBeats: number; notes: number[] } }
	| { addNotes: { track: number; block: number; notes: NoteView[] } }
	| { editNotes: { track: number; block: number; edits: NoteEdit[] } }
	| { deleteNotes: { track: number; block: number; notes: number[] } }
	| { addTrack: { kind: TrackKind } }
	| { deleteTrack: { track: number } }
	| { duplicateTrack: { track: number } }
	| { renameTrack: { track: number; name: string } }
	| { reorderTrack: { from: number; to: number } }
	| { setTrackColor: { track: number; colorIndex: number } }
	| { copyRange: { range: BeatRange } }
	| { cutRange: { range: BeatRange } }
	| { deleteRange: { range: BeatRange } }
	| { pasteRange: { track: number; startBeats: number } }
	| { duplicateRange: { range: BeatRange } }
	| { setEffectSidechain: { track: number; index: number; source: number | null } }
	| { setInstrumentEnabled: { track: number; enabled: boolean } }
	| { setInstrumentTone: { track: number; cutoff: number; resonance: number; detune: number; sub: number } }
	| { addEffect: { track: number; kind: EffectKind; index: number | null } }
	| {
			pasteEffect: {
				track: number;
				index?: number | null;
				kind: EffectKind;
				params: number[];
				enabled: boolean;
				sidechain?: number | null;
			};
	  }
	| { duplicateEffect: { track: number; index: number } }
	| { removeEffect: { track: number; index: number } }
	| { moveEffect: { track: number; from: number; to: number } }
	| { setEffectEnabled: { track: number; index: number; enabled: boolean } }
	| { setEffectParam: { track: number; index: number; param: number; value: number } }
	| { setMasterVolume: { volume: number } }
	| { exportStems: { dir?: string | null } }
	| { exportAudio: { path: string | null } }
	| { exportProject: { path: string | null } }
	| { loadProject: { path: string | null } }
	| 'undo'
	| 'redo'
	| 'checkpoint';

