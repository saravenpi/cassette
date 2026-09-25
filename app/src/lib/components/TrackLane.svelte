<script lang="ts">
	/**
	 * One track's lane: the grid behind it, the blocks on it, and the double-click
	 * that creates a MIDI block. A drop from the file browser finds this element by
	 * its `data-lane-track` attribute.
	 */

	import { DEFAULT_LENGTH_BEATS, ROW_HEIGHT, pxToBeats, snapBeats } from '$lib/geometry';
	import { session } from '$lib/session.svelte';
	import type { BlockView, TrackView } from '$lib/wire';
	import Block from './Block.svelte';

	interface Props {
		track: TrackView;
		index: number;
		width: number;
		barWidth: number;
		height?: number;
	}

	let { track, index, width, barWidth, height }: Props = $props();

	interface LaneItem {
		key: string;
		block: BlockView;
		index: number;
		startBeats: number;
		lengthBeats: number;
		dragging: boolean;
	}

	const items = $derived.by((): LaneItem[] => {
		const drag = session.drag.state;
		const home = drag !== null && drag.track === index;
		const target = drag !== null && drag.kind === 'move' && drag.targetTrack === index;
		const out: LaneItem[] = [];
		track.blocks.forEach((block, position) => {
			if (drag !== null && drag.kind === 'move' && home && drag.index === position) {
				out.push({
					key: `block-${position}`,
					block: drag.view,
					index: position,
					startBeats: drag.startBeats,
					lengthBeats: drag.lengthBeats,
					dragging: true
				});
				return;
			}
			const resizing = drag !== null && drag.kind === 'resize' && home && drag.index === position;
			out.push({
				key: `block-${position}`,
				block,
				index: position,
				startBeats: block.startBeats,
				lengthBeats: resizing ? drag.lengthBeats : block.lengthBeats,
				dragging: false
			});
		});
		if (drag !== null && drag.kind === 'move' && target && !home) {
			out.push({
				key: 'dragged',
				block: drag.view,
				index: drag.index,
				startBeats: drag.startBeats,
				lengthBeats: drag.lengthBeats,
				dragging: true
			});
		}
		return out;
	});

	function onDoubleClick(event: MouseEvent): void {
		if (track.kind !== 'midi') return;
		if (event.target instanceof Element && event.target.closest('[data-block]') !== null) return;
		const lane = event.currentTarget;
		if (!(lane instanceof HTMLElement)) return;
		const beats = pxToBeats(event.clientX - lane.getBoundingClientRect().left, session.zoom);
		session.dispatch({
			createBlock: { track: index, startBeats: snapBeats(beats), lengthBeats: DEFAULT_LENGTH_BEATS }
		});
	}
</script>

<div
	class="lane"
	role="list"
	aria-label="Blocks on {track.name}"
	data-lane-track={index}
	style="width: {width}px; height: {height ?? ROW_HEIGHT}px; --bar: {barWidth}px"
	ondblclick={onDoubleClick}
>
	{#each items as item (item.key)}
		<Block
			block={item.block}
			track={index}
			index={item.index}
			startBeats={item.startBeats}
			lengthBeats={item.lengthBeats}
			dragging={item.dragging}
			colorIndex={track.colorIndex}
		/>
	{/each}
</div>

<style>
	.lane {
		position: relative;
		flex: none;
		border-bottom: 1px solid var(--line);
		background-color: var(--bg);
		background-image: repeating-linear-gradient(
			to right,
			var(--line) 0 1px,
			transparent 1px var(--bar)
		);
	}
</style>
