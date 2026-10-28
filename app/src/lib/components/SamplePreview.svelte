<script lang="ts">
	/**
	 * The waveform thumbnail of one audio file, drawn from `readWaveform`, with
	 * audition playback. It shows a failure rather than an empty box, because a
	 * missing thumbnail and a file that cannot be decoded look the same otherwise.
	 */

	import { onDestroy } from 'svelte';
	import Icon, { addCollection } from '@iconify/svelte';
	import pixelarticons from '@iconify-json/pixelarticons/icons.json';

	import { loadWaveform, readSampleWav } from '$lib/api';
	import { downsamplePeaks, waveformPoints } from '$lib/geometry';
	import type { SampleEntry } from '$lib/wire';

	addCollection(pixelarticons);

	const HEIGHT = 44;
	const MAX_PREVIEW_POINTS = 256;

	interface Props {
		entry: SampleEntry | null;
	}

	let { entry }: Props = $props();

	let peaks = $state<number[] | null>(null);
	let failure = $state<string | null>(null);
	let playing = $state(false);
	let loadingAudio = $state(false);
	let currentAudio: HTMLAudioElement | null = null;
	let currentUrl: string | null = null;
	let progress = $state(0);
	let frame = 0;

	function track(): void {
		if (currentAudio === null) return;
		const length = currentAudio.duration;
		progress = Number.isFinite(length) && length > 0 ? currentAudio.currentTime / length : 0;
		frame = requestAnimationFrame(track);
	}

	function stopPreview(): void {
		cancelAnimationFrame(frame);
		progress = 0;
		if (currentAudio !== null) {
			currentAudio.pause();
			currentAudio.currentTime = 0;
			currentAudio = null;
		}
		if (currentUrl !== null) {
			URL.revokeObjectURL(currentUrl);
			currentUrl = null;
		}
		playing = false;
		loadingAudio = false;
	}

	/** Start the preview from the top, replacing one already playing. */
	export function play(): void {
		stopPreview();
		void togglePreview();
	}

	async function togglePreview(): Promise<void> {
		if (playing || loadingAudio) {
			stopPreview();
			return;
		}
		const path = entry?.kind === 'wav' || entry?.kind === 'audio' ? entry.path : '';
		if (path === '') return;
		loadingAudio = true;
		try {
			const bytes = await readSampleWav(path);
			if (!loadingAudio) return;
			const blob = new Blob([new Uint8Array(bytes)], { type: 'audio/wav' });
			const url = URL.createObjectURL(blob);
			const audio = new Audio(url);
			currentUrl = url;
			currentAudio = audio;
			loadingAudio = false;
			playing = true;
			audio.onended = () => {
				stopPreview();
			};
			audio.onerror = () => {
				stopPreview();
			};
			await audio.play();
			frame = requestAnimationFrame(track);
		} catch {
			stopPreview();
		}
	}

	onDestroy(() => stopPreview());

	$effect(() => {
		const path = entry?.kind === 'wav' || entry?.kind === 'audio' ? entry.path : '';
		stopPreview();
		if (path === '') {
			peaks = null;
			failure = null;
			return;
		}
		let live = true;
		void loadWaveform(path)
			.then((value) => {
				if (!live) return;
				peaks = downsamplePeaks(value.peaks, MAX_PREVIEW_POINTS);
				failure = null;
			})
			.catch((error: unknown) => {
				if (!live) return;
				peaks = null;
				failure = `${entry?.name ?? path} could not be read: ${
					error instanceof Error ? error.message : String(error)
				}`;
			});
		return () => {
			live = false;
		};
	});
</script>

{#if entry !== null && (entry.kind === 'wav' || entry.kind === 'audio')}
	<div class="preview">
		<div class="top-row">
			<span class="file">{entry.name}</span>
			<button
				type="button"
				class="play-btn"
				class:active={playing}
				aria-label={playing ? 'Stop preview' : 'Play preview'}
				title={playing ? 'Stop preview' : 'Play preview'}
				onclick={() => void togglePreview()}
			>
				{#if playing}
					<Icon icon="pixelarticons:stop" width="14" height="14" />
				{:else}
					<Icon icon="pixelarticons:play" width="14" height="14" />
				{/if}
			</button>
		</div>
		{#if peaks !== null && peaks.length > 0}
			<!-- svelte-ignore a11y_click_events_have_key_events -->
			<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
			<svg
				class="thumb"
				viewBox="0 0 {Math.max(1, peaks.length - 1)} {HEIGHT}"
				preserveAspectRatio="none"
				role="img"
				aria-label="Waveform of {entry.name}"
				onclick={() => void togglePreview()}
			>
				<polygon points={waveformPoints(peaks, HEIGHT)} />
				{#if playing}
					<line
						class="marker"
						x1={progress * Math.max(1, peaks.length - 1)}
						x2={progress * Math.max(1, peaks.length - 1)}
						y1="0"
						y2={HEIGHT}
					/>
				{/if}
			</svg>
		{/if}
		{#if failure !== null}
			<span class="failure">{failure}</span>
		{/if}
	</div>
{/if}

<style>
	.preview {
		flex: none;
		padding: 10px 12px 12px;
		border-top: 1px solid var(--border-subtle);
	}

	.top-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
	}

	.file {
		flex: 1;
		min-width: 0;
		font-size: 11px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--text-subtle);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.play-btn {
		flex: none;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 22px;
		height: 22px;
		padding: 0;
		background: var(--surface);
		border: 1px solid var(--border);
		color: var(--text-subtle);
		cursor: pointer;
	}

	.play-btn:hover {
		color: var(--text);
		border-color: var(--green);
	}

	.play-btn.active {
		background: var(--green);
		color: var(--on-green);
		border-color: var(--green);
	}

	.thumb {
		display: block;
		width: 100%;
		height: 44px;
		margin-top: 8px;
		cursor: pointer;
	}

	.thumb polygon {
		fill: var(--green);
		stroke: none;
	}

	.thumb .marker {
		stroke: var(--text);
		stroke-width: 1.5px;
		vector-effect: non-scaling-stroke;
	}

	.failure {
		display: block;
		margin-top: 8px;
		font-size: 11px;
		color: var(--alert);
	}
</style>
