export {};

interface Matchers {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	toContainEqual(expected: unknown): void;
	toBeCloseTo(expected: number, precision: number): void;
	toBeNull(): void;
	not: {
		toBeNull(): void;
	};
}

declare const describe: (name: string, fn: () => void) => void;
declare const test: (name: string, fn: () => void | Promise<void>) => void;
declare const expect: (actual: unknown) => Matchers;

if (typeof (globalThis as Record<string, unknown>).$state === 'undefined') {
	(globalThis as Record<string, unknown>).$state = <T>(value: T) => value;
}

const { AudioRecorder, encodeWav } = await import('./audioRecording.svelte');
const { DragController } = await import('./drag.svelte');

describe('encodeWav', () => {
	test('produces a blob with audio/wav type', () => {
		const samples = new Float32Array([0, 0.5, -0.5]);
		const blob = encodeWav(samples, 44100);
		expect(blob.type).toBe('audio/wav');
		expect(blob.size).toBe(44 + samples.length * 2);
	});

	test('formats valid RIFF WAV header for mono audio', async () => {
		const samples = new Float32Array([0.1, -0.2, 0.3, -0.4]);
		const sampleRate = 48000;
		const blob = encodeWav(samples, sampleRate, 1);
		const buffer = await blob.arrayBuffer();
		const view = new DataView(buffer);

		const riff = String.fromCharCode(
			view.getUint8(0),
			view.getUint8(1),
			view.getUint8(2),
			view.getUint8(3)
		);
		expect(riff).toBe('RIFF');

		expect(view.getUint32(4, true)).toBe(36 + samples.length * 2);

		const wave = String.fromCharCode(
			view.getUint8(8),
			view.getUint8(9),
			view.getUint8(10),
			view.getUint8(11)
		);
		expect(wave).toBe('WAVE');

		const fmt = String.fromCharCode(
			view.getUint8(12),
			view.getUint8(13),
			view.getUint8(14),
			view.getUint8(15)
		);
		expect(fmt).toBe('fmt ');

		expect(view.getUint32(16, true)).toBe(16);
		expect(view.getUint16(20, true)).toBe(1);
		expect(view.getUint16(22, true)).toBe(1);
		expect(view.getUint32(24, true)).toBe(sampleRate);
		expect(view.getUint32(28, true)).toBe(sampleRate * 2);
		expect(view.getUint16(32, true)).toBe(2);
		expect(view.getUint16(34, true)).toBe(16);

		const dataTag = String.fromCharCode(
			view.getUint8(36),
			view.getUint8(37),
			view.getUint8(38),
			view.getUint8(39)
		);
		expect(dataTag).toBe('data');
		expect(view.getUint32(40, true)).toBe(samples.length * 2);
	});

	test('formats valid RIFF WAV header for stereo audio', async () => {
		const samples = new Float32Array([0.2, -0.2, 0.4, -0.4]);
		const sampleRate = 44100;
		const blob = encodeWav(samples, sampleRate, 2);
		const buffer = await blob.arrayBuffer();
		const view = new DataView(buffer);

		expect(view.getUint16(22, true)).toBe(2);
		expect(view.getUint32(28, true)).toBe(sampleRate * 4);
		expect(view.getUint16(32, true)).toBe(4);
	});

	test('converts sample amplitudes to signed 16-bit PCM and clamps outliers', async () => {
		const samples = new Float32Array([0, 1.0, -1.0, 1.5, -2.0, 0.5]);
		const blob = encodeWav(samples, 44100);
		const buffer = await blob.arrayBuffer();
		const view = new DataView(buffer);

		expect(view.getInt16(44, true)).toBe(0);
		expect(view.getInt16(46, true)).toBe(32767);
		expect(view.getInt16(48, true)).toBe(-32768);
		expect(view.getInt16(50, true)).toBe(32767);
		expect(view.getInt16(52, true)).toBe(-32768);
		expect(view.getInt16(54, true)).toBe(16384);
	});

	test('encodes empty buffer without error', async () => {
		const samples = new Float32Array(0);
		const blob = encodeWav(samples, 44100);
		expect(blob.size).toBe(44);
		const buffer = await blob.arrayBuffer();
		const view = new DataView(buffer);
		expect(view.getUint32(4, true)).toBe(36);
		expect(view.getUint32(40, true)).toBe(0);
	});
});

describe('AudioRecorder', () => {
	test('initialises with isRecording false', () => {
		const recorder = new AudioRecorder();
		expect(recorder.isRecording).toBe(false);
	});

	test('stop returns null when idle', async () => {
		const recorder = new AudioRecorder();
		const result = await recorder.stop();
		expect(result).toBeNull();
	});

	test('start handles missing mediaDevices gracefully without throwing', async () => {
		const recorder = new AudioRecorder();
		const originalNavigator = globalThis.navigator;
		Object.defineProperty(globalThis, 'navigator', {
			value: {},
			configurable: true,
			writable: true
		});

		const started = await recorder.start(0, 0);
		expect(started).toBe(false);
		expect(recorder.isRecording).toBe(false);

		Object.defineProperty(globalThis, 'navigator', {
			value: originalNavigator,
			configurable: true,
			writable: true
		});
	});

	test('start handles mediaDevices permission denial gracefully', async () => {
		const recorder = new AudioRecorder();
		const originalNavigator = globalThis.navigator;
		Object.defineProperty(globalThis, 'navigator', {
			value: {
				mediaDevices: {
					getUserMedia: () => Promise.reject(new Error('Permission denied'))
				}
			},
			configurable: true,
			writable: true
		});

		const started = await recorder.start(1, 4);
		expect(started).toBe(false);
		expect(recorder.isRecording).toBe(false);

		Object.defineProperty(globalThis, 'navigator', {
			value: originalNavigator,
			configurable: true,
			writable: true
		});
	});

	test('exposes encodeWav helper on instance', () => {
		const recorder = new AudioRecorder();
		const blob = recorder.encodeWav(new Float32Array([0.2]), 44100);
		expect(blob.type).toBe('audio/wav');
		expect(blob.size).toBe(46);
	});

	test('records audio chunks and sends request on stop', async () => {
		const requests: unknown[] = [];
		const fakeSession = {
			request: (ev: unknown) => {
				requests.push(ev);
				return Promise.resolve(true);
			}
		};

		const recorder = new AudioRecorder(fakeSession);

		let processCallback: ((ev: { inputBuffer: { getChannelData: () => Float32Array } }) => void) | null = null;
		const mockStream = {
			getTracks: () => [{ stop: () => {} }]
		};

		class MockAudioContext {
			sampleRate = 44100;
			state = 'running';
			destination = {};
			createMediaStreamSource() {
				return {
					connect: () => {},
					disconnect: () => {}
				};
			}
			createScriptProcessor() {
				const node = {
					onaudioprocess: null,
					connect: () => {},
					disconnect: () => {}
				};
				processCallback = (ev) => {
					if (typeof (node as any).onaudioprocess === 'function') {
						(node.onaudioprocess as any)(ev);
					}
				};
				return node;
			}
			close() {
				this.state = 'closed';
				return Promise.resolve();
			}
		}

		const origNav = globalThis.navigator;
		const origWin = globalThis.window;

		Object.defineProperty(globalThis, 'navigator', {
			value: {
				mediaDevices: {
					getUserMedia: () => Promise.resolve(mockStream)
				}
			},
			configurable: true,
			writable: true
		});

		Object.defineProperty(globalThis, 'window', {
			value: {
				AudioContext: MockAudioContext,
				dispatchEvent: () => true
			},
			configurable: true,
			writable: true
		});

		const started = await recorder.start(2, 8);
		expect(started).toBe(true);
		expect(recorder.isRecording).toBe(true);

		if (processCallback) {
			(processCallback as any)({
				inputBuffer: {
					getChannelData: () => new Float32Array([0.1, 0.2, 0.3])
				}
			});
		}

		const result = await recorder.stop();
		expect(recorder.isRecording).toBe(false);
		expect(result).not.toBeNull();
		expect(result?.sampleRate).toBe(44100);
		expect(result?.pcm.length).toBe(3);
		expect(result?.pcm[0]).toBeCloseTo(0.1, 5);

		Object.defineProperty(globalThis, 'navigator', {
			value: origNav,
			configurable: true,
			writable: true
		});
		Object.defineProperty(globalThis, 'window', {
			value: origWin,
			configurable: true,
			writable: true
		});
	});
});

describe('DragController block mismatch guard', () => {
	test('flags invalidTarget when moving MIDI block to audio track and protects targetTrack on flush/release', async () => {
		const sent: unknown[] = [];
		const snapshot = {
			bpm: 120,
			cursor: { track: 0, beats: 0 },
			tracks: [
				{ name: 'Track 1', kind: 'midi', blocks: [] },
				{ name: 'Track 2', kind: 'audio', blocks: [] }
			]
		};

		const context = {
			snapshot: () => snapshot as any,
			zoom: () => 16,
			send: (ev: unknown) => {
				sent.push(ev);
				return Promise.resolve(true);
			}
		};

		const origWindow = globalThis.window;
		Object.defineProperty(globalThis, 'window', {
			value: {
				addEventListener: () => {},
				removeEventListener: () => {},
				dispatchEvent: () => true
			},
			configurable: true,
			writable: true
		});

		const controller = new DragController(context);
		const midiBlock = {
			name: 'Clip',
			kind: 'midi' as const,
			startBeats: 0,
			lengthBeats: 4,
			offsetSeconds: 0,
			rate: 1,
			selected: false,
			notes: [],
			source: ''
		};

		controller.beginMove(0, 0, midiBlock, 0);
		expect(controller.state?.invalidTarget).toBe(false);

		const fakeLaneElement = {
			dataset: { laneTrack: '1' },
			getBoundingClientRect: () => ({ left: 0, top: 0, width: 500, height: 100 }),
			closest: () => fakeLaneElement
		};

		const origDoc = globalThis.document;
		Object.defineProperty(globalThis, 'document', {
			value: {
				elementsFromPoint: () => [fakeLaneElement]
			},
			configurable: true,
			writable: true
		});

		const pointerMoveHandler = (controller as any).onPointerMove;
		pointerMoveHandler({
			clientX: 32,
			clientY: 50,
			buttons: 1,
			altKey: false
		});

		expect(controller.state?.invalidTarget).toBe(true);

		await controller.flush();
		expect(controller.state?.targetTrack).toBe(0);
		expect(sent).toContainEqual({
			placeBlock: {
				track: 0,
				block: 0,
				targetTrack: 0,
				startBeats: 2
			}
		});

		const pointerUpHandler = (controller as any).onPointerUp;
		pointerUpHandler();
		expect(controller.state?.released).toBe(true);
		expect(controller.state?.targetTrack).toBe(0);

		Object.defineProperty(globalThis, 'document', {
			value: origDoc,
			configurable: true,
			writable: true
		});
		Object.defineProperty(globalThis, 'window', {
			value: origWindow,
			configurable: true,
			writable: true
		});
	});
});
