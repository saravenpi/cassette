<script lang="ts">
	/**
	 * The waveform thumbnail of one audio file, drawn from `readWaveform`. It shows
	 * a failure rather than an empty box, because a missing thumbnail and a file
	 * that cannot be decoded look the same otherwise.
	 */

	import { loadWaveform } from '$lib/api';
	import { waveformPoints } from '$lib/geometry';
	import type { SampleEntry } from '$lib/wire';

	const HEIGHT = 44;

	interface Props {
		entry: SampleEntry | null;
	}

	let { entry }: Props = $props();

	let peaks = $state<number[] | null>(null);
	let failure = $state<string | null>(null);

	$effect(() => {
		const path = entry?.kind === 'wav' ? entry.path : '';
		if (path === '') {
			peaks = null;
			failure = null;
			return;
		}
		let live = true;
		void loadWaveform(path)
			.then((value) => {
				if (!live) return;
				peaks = value;
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

{#if entry !== null && entry.kind === 'wav'}
	<div class="preview">
		<span class="file">{entry.name}</span>
		{#if peaks !== null}
			<svg
				class="thumb"
				viewBox="0 0 {peaks.length - 1} {HEIGHT}"
				preserveAspectRatio="none"
				role="img"
				aria-label="Waveform of {entry.name}"
			>
				<polyline points={waveformPoints(peaks, HEIGHT)} />
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
		border-top: 1px solid var(--line);
		padding: 5px 8px;
	}

	.file {
		display: block;
		font-size: 10px;
		color: var(--dim);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.thumb {
		display: block;
		width: 100%;
		height: 44px;
		margin-top: 3px;
	}

	.thumb polyline {
		fill: none;
		stroke: var(--accent);
		stroke-width: 1;
		vector-effect: non-scaling-stroke;
	}

	.failure {
		display: block;
		margin-top: 3px;
		font-size: 10px;
		color: var(--warn);
	}
</style>
