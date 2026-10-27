/**
 * Parametric equalizer calculations: biquad frequency responses, composite curve
 * evaluation and coordinate projections for the EQ Eight display.
 */

export interface EqBand {
	freq: number;
	gain: number;
	q: number;
	type: number;
}

export const EQ_FILTER_TYPES = [
	'Low Cut',
	'Low Shelf',
	'Bell',
	'High Shelf',
	'High Cut',
	'Notch'
] as const;

export const FREQ_GRID = [50, 100, 250, 500, 1000, 2000, 5000, 10000, 20000];
export const GAIN_GRID = [18, 12, 6, 0, -6, -12, -18];

export function biquadGainDb(
	f: number,
	f0: number,
	gainDb: number,
	q: number,
	type: number,
	sampleRate = 44100
): number {
	const nyquist = sampleRate * 0.5;
	const clampedF0 = Math.min(nyquist * 0.95, Math.max(10, f0));
	const clampedQ = Math.max(0.01, q);
	const w0 = (2 * Math.PI * clampedF0) / sampleRate;
	const cosW0 = Math.cos(w0);
	const sinW0 = Math.sin(w0);
	const alpha = sinW0 / (2 * clampedQ);
	const a = Math.pow(10, gainDb / 40);
	const root = 2 * Math.sqrt(a) * alpha;

	let b0 = 1;
	let b1 = 0;
	let b2 = 0;
	let a0 = 1;
	let a1 = 0;
	let a2 = 0;

	switch (type) {
		case 0:
			b0 = (1 + cosW0) / 2;
			b1 = -(1 + cosW0);
			b2 = (1 + cosW0) / 2;
			a0 = 1 + alpha;
			a1 = -2 * cosW0;
			a2 = 1 - alpha;
			break;
		case 1:
			b0 = a * (a + 1 - (a - 1) * cosW0 + root);
			b1 = 2 * a * (a - 1 - (a + 1) * cosW0);
			b2 = a * (a + 1 - (a - 1) * cosW0 - root);
			a0 = a + 1 + (a - 1) * cosW0 + root;
			a1 = -2 * (a - 1 + (a + 1) * cosW0);
			a2 = a + 1 + (a - 1) * cosW0 - root;
			break;
		case 2:
			b0 = 1 + alpha * a;
			b1 = -2 * cosW0;
			b2 = 1 - alpha * a;
			a0 = 1 + alpha / a;
			a1 = -2 * cosW0;
			a2 = 1 - alpha / a;
			break;
		case 3:
			b0 = a * (a + 1 + (a - 1) * cosW0 + root);
			b1 = -2 * a * (a - 1 + (a + 1) * cosW0);
			b2 = a * (a + 1 + (a - 1) * cosW0 - root);
			a0 = a + 1 - (a - 1) * cosW0 + root;
			a1 = 2 * (a - 1 - (a + 1) * cosW0);
			a2 = a + 1 - (a - 1) * cosW0 - root;
			break;
		case 4:
			b0 = (1 - cosW0) / 2;
			b1 = 1 - cosW0;
			b2 = (1 - cosW0) / 2;
			a0 = 1 + alpha;
			a1 = -2 * cosW0;
			a2 = 1 - alpha;
			break;
		case 5:
			b0 = 1;
			b1 = -2 * cosW0;
			b2 = 1;
			a0 = 1 + alpha;
			a1 = -2 * cosW0;
			a2 = 1 - alpha;
			break;
		default:
			return 0;
	}

	const w = (2 * Math.PI * f) / sampleRate;
	const cosW = Math.cos(w);
	const sinW = Math.sin(w);
	const cos2W = Math.cos(2 * w);
	const sin2W = Math.sin(2 * w);

	const nRe = b0 + b1 * cosW + b2 * cos2W;
	const nIm = -b1 * sinW - b2 * sin2W;
	const dRe = a0 + a1 * cosW + a2 * cos2W;
	const dIm = -a1 * sinW - a2 * sin2W;

	const num = nRe * nRe + nIm * nIm;
	const den = dRe * dRe + dIm * dIm;
	if (den <= 1e-12) return 0;
	const magSq = num / den;
	if (magSq <= 1e-12) return -120;
	return 10 * Math.log10(magSq);
}

export function freqToX(freq: number, width: number, minF = 20, maxF = 20000): number {
	const clamped = Math.max(minF, Math.min(maxF, freq));
	const norm = Math.log10(clamped / minF) / Math.log10(maxF / minF);
	return norm * width;
}

export function xToFreq(x: number, width: number, minF = 20, maxF = 20000): number {
	if (width <= 0) return minF;
	const norm = Math.max(0, Math.min(1, x / width));
	return minF * Math.pow(maxF / minF, norm);
}

export function gainToY(gainDb: number, height: number, minDb = -24, maxDb = 24): number {
	const norm = (maxDb - gainDb) / (maxDb - minDb);
	return Math.max(0, Math.min(height, norm * height));
}

export function yToGain(y: number, height: number, minDb = -24, maxDb = 24): number {
	if (height <= 0) return 0;
	const norm = Math.max(0, Math.min(1, y / height));
	return maxDb - norm * (maxDb - minDb);
}

export function generateLogFrequencies(count = 100, minF = 20, maxF = 20000): number[] {
	const freqs: number[] = [];
	const logMin = Math.log10(minF);
	const logMax = Math.log10(maxF);
	const step = (logMax - logMin) / (count - 1);
	for (let i = 0; i < count; i++) {
		freqs.push(Math.pow(10, logMin + i * step));
	}
	return freqs;
}

export function computeCompositeCurve(
	bands: EqBand[],
	width: number,
	height: number,
	sampleRate = 44100
): { path: string; area: string } {
	const freqs = generateLogFrequencies(100);
	const points: [number, number][] = [];
	const zeroY = gainToY(0, height);

	for (const f of freqs) {
		let totalDb = 0;
		for (const b of bands) {
			totalDb += biquadGainDb(f, b.freq, b.gain, b.q, b.type, sampleRate);
		}
		const x = freqToX(f, width);
		const y = gainToY(Math.max(-36, Math.min(36, totalDb)), height);
		points.push([x, y]);
	}

	if (points.length === 0) return { path: '', area: '' };

	let linePath = `M ${points[0][0].toFixed(1)} ${points[0][1].toFixed(1)}`;
	for (let i = 1; i < points.length; i++) {
		linePath += ` L ${points[i][0].toFixed(1)} ${points[i][1].toFixed(1)}`;
	}

	const lastX = points[points.length - 1][0].toFixed(1);
	const firstX = points[0][0].toFixed(1);
	const areaPath = `${linePath} L ${lastX} ${zeroY.toFixed(1)} L ${firstX} ${zeroY.toFixed(1)} Z`;

	return { path: linePath, area: areaPath };
}

export function unpackBands(params: readonly number[]): EqBand[] {
	const defaultFreqs = [30, 120, 350, 1000, 2500, 6000, 10000, 18000];
	const defaultTypes = [0, 1, 2, 2, 2, 2, 3, 4];
	const defaultQs = [0.71, 0.71, 1.0, 1.0, 1.0, 1.0, 0.71, 0.71];

	const bands: EqBand[] = [];
	for (let i = 0; i < 8; i++) {
		const base = i * 4;
		bands.push({
			freq: params[base] ?? defaultFreqs[i],
			gain: params[base + 1] ?? 0,
			q: params[base + 2] ?? defaultQs[i],
			type: Math.round(params[base + 3] ?? defaultTypes[i])
		});
	}
	return bands;
}

export function isBandActive(band: EqBand): boolean {
	if (band.type === 0 || band.type === 4 || band.type === 5) return true;
	return Math.abs(band.gain) > 0.05;
}

export function formatFreq(f: number): string {
	return f >= 1000 ? `${(f / 1000).toFixed(f % 1000 === 0 ? 0 : 1)}k` : `${Math.round(f)}`;
}
