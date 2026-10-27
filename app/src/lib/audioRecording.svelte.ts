import { saveRecordedSample } from './api';

export interface RecordedAudio {
	pcm: Float32Array;
	sampleRate: number;
}

interface RecordingSession {
	request(event: unknown): Promise<boolean>;
}

export function encodeWav(samples: Float32Array, sampleRate: number, numChannels = 1): Blob {
	const dataSize = samples.length * 2;
	const buffer = new ArrayBuffer(44 + dataSize);
	const view = new DataView(buffer);

	view.setUint8(0, 0x52);
	view.setUint8(1, 0x49);
	view.setUint8(2, 0x46);
	view.setUint8(3, 0x46);

	view.setUint32(4, 36 + dataSize, true);

	view.setUint8(8, 0x57);
	view.setUint8(9, 0x41);
	view.setUint8(10, 0x56);
	view.setUint8(11, 0x45);

	view.setUint8(12, 0x66);
	view.setUint8(13, 0x6d);
	view.setUint8(14, 0x74);
	view.setUint8(15, 0x20);

	view.setUint32(16, 16, true);
	view.setUint16(20, 1, true);
	view.setUint16(22, numChannels, true);
	view.setUint32(24, sampleRate, true);
	view.setUint32(28, sampleRate * numChannels * 2, true);
	view.setUint16(32, numChannels * 2, true);
	view.setUint16(34, 16, true);

	view.setUint8(36, 0x64);
	view.setUint8(37, 0x61);
	view.setUint8(38, 0x74);
	view.setUint8(39, 0x61);

	view.setUint32(40, dataSize, true);

	let offset = 44;
	for (let i = 0; i < samples.length; i++) {
		const sample = Math.max(-1, Math.min(1, samples[i]));
		const int16 = sample < 0 ? Math.round(sample * 0x8000) : Math.round(sample * 0x7fff);
		view.setInt16(offset, int16, true);
		offset += 2;
	}

	return new Blob([buffer], { type: 'audio/wav' });
}

export class AudioRecorder {
	isRecording = $state(false);

	private targetTrack = 0;
	private startBeats = 0;
	private stream: MediaStream | null = null;
	private audioContext: AudioContext | null = null;
	private processor: ScriptProcessorNode | null = null;
	private sourceNode: MediaStreamAudioSourceNode | null = null;
	private chunks: Float32Array[] = [];
	private totalSamples = 0;
	private sessionRef: RecordingSession | null = null;

	constructor(sessionContext?: RecordingSession) {
		this.sessionRef = sessionContext ?? null;
	}

	encodeWav(samples: Float32Array, sampleRate: number, numChannels = 1): Blob {
		return encodeWav(samples, sampleRate, numChannels);
	}

	async start(trackIndex: number, startBeats: number): Promise<boolean> {
		if (this.isRecording) {
			await this.stop();
		}

		this.targetTrack = trackIndex;
		this.startBeats = startBeats;
		this.chunks = [];
		this.totalSamples = 0;

		try {
			if (typeof navigator === 'undefined' || !navigator.mediaDevices?.getUserMedia) {
				return false;
			}

			this.stream = await navigator.mediaDevices.getUserMedia({
				audio: {
					echoCancellation: false,
					noiseSuppression: false,
					autoGainControl: false
				}
			});

			const AudioCtx =
				typeof window !== 'undefined'
					? window.AudioContext ||
						(window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext
					: null;

			if (!AudioCtx) {
				this.cleanup();
				return false;
			}

			this.audioContext = new AudioCtx();
			this.sourceNode = this.audioContext.createMediaStreamSource(this.stream);
			this.processor = this.audioContext.createScriptProcessor(4096, 1, 1);

			this.processor.onaudioprocess = (event: AudioProcessingEvent) => {
				if (!this.isRecording) return;
				const input = event.inputBuffer.getChannelData(0);
				const copy = new Float32Array(input);
				this.chunks.push(copy);
				this.totalSamples += copy.length;
			};

			this.sourceNode.connect(this.processor);
			this.processor.connect(this.audioContext.destination);

			this.isRecording = true;
			return true;
		} catch {
			this.cleanup();
			this.isRecording = false;
			return false;
		}
	}

	async stop(): Promise<RecordedAudio | null> {
		if (!this.isRecording && this.chunks.length === 0) {
			return null;
		}

		this.isRecording = false;
		const sampleRate = this.audioContext?.sampleRate ?? 44100;

		this.cleanup();

		if (this.totalSamples === 0) {
			return null;
		}

		const pcm = new Float32Array(this.totalSamples);
		let offset = 0;
		for (const chunk of this.chunks) {
			pcm.set(chunk, offset);
			offset += chunk.length;
		}
		this.chunks = [];
		this.totalSamples = 0;

		const wavBlob = encodeWav(pcm, sampleRate);
		const arrayBuffer = await wavBlob.arrayBuffer();
		const bytes = new Uint8Array(arrayBuffer);
		const filename = `recording_${Date.now()}_track${this.targetTrack + 1}.wav`;

		let wavPath: string | null = null;
		try {
			wavPath = await saveRecordedSample(filename, bytes);
		} catch {
			wavPath = null;
		}

		let targetSession = this.sessionRef;
		if (!targetSession) {
			try {
				const mod = await import('./session.svelte');
				targetSession = mod.session;
			} catch {
				targetSession = null;
			}
		}

		if (wavPath !== null && targetSession) {
			await targetSession.request({
				importSample: {
					track: this.targetTrack,
					path: wavPath,
					startBeats: this.startBeats
				}
			});
		}

		return { pcm, sampleRate };
	}

	private cleanup(): void {
		if (this.processor) {
			try {
				this.processor.disconnect();
				this.processor.onaudioprocess = null;
			} catch {}
			this.processor = null;
		}
		if (this.sourceNode) {
			try {
				this.sourceNode.disconnect();
			} catch {}
			this.sourceNode = null;
		}
		if (this.audioContext) {
			try {
				if (this.audioContext.state !== 'closed') {
					void this.audioContext.close();
				}
			} catch {}
			this.audioContext = null;
		}
		if (this.stream) {
			try {
				this.stream.getTracks().forEach((track) => track.stop());
			} catch {}
			this.stream = null;
		}
	}
}

export const audioRecorder = new AudioRecorder();
