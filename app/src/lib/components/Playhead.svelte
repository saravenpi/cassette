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

<div class="playhead" style="left: {left}px; top: {top}px; height: calc(100% - {top}px)">
	<span class="head"></span>
</div>

<style>
	.playhead {
		position: absolute;
		width: 1px;
		background: var(--head);
		pointer-events: none;
		z-index: 2;
	}

	.head {
		position: absolute;
		top: 0;
		left: -4px;
		width: 9px;
		height: 7px;
		background: var(--head);
		clip-path: polygon(0 0, 100% 0, 50% 100%);
	}
</style>
