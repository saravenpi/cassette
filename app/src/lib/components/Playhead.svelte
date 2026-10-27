<script lang="ts">
	/**
	 * The playhead. The transport has no seek event yet, so this draws the
	 * position and never takes the pointer.
	 */

	import { beatsToPx } from '$lib/geometry';

	interface Props {
		beats: number;
		pxPerBeat: number;
		offset: number;
		top: number;
	}

	let { beats, pxPerBeat, offset, top }: Props = $props();

	const left = $derived(offset + beatsToPx(beats, pxPerBeat));
</script>

<div class="playhead" style="transform: translate3d({left}px, 0, 0); top: {top}px; height: calc(100% - {top}px)">
	<span class="head"></span>
</div>

<style>
	.playhead {
		position: absolute;
		left: 0;
		width: 1px;
		background: var(--olive);
		pointer-events: none;
		z-index: 2;
		will-change: transform;
	}

	.head {
		position: absolute;
		top: 0;
		left: -3px;
		width: 7px;
		height: 7px;
		background: var(--olive);
	}
</style>
