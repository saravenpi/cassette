<script lang="ts">
	/**
	 * One track's lane: the bar, beat and snap grid behind it, the blocks on it,
	 * and the right-click menu that adds a clip. A drop from the file browser
	 * finds this element by its `data-lane-track` attribute.
	 */

	import { ROW_HEIGHT, beatsToPx, gridStep, pxToBeats } from '$lib/geometry';
	import { laneMenu } from '$lib/menu';
	import { menu } from '$lib/menu.svelte';
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

	const beatWidth = $derived(session.zoom);
	const stepWidth = $derived(gridStep(session.zoom) * session.zoom);
	const showBeats = $derived(beatWidth >= 12 && beatWidth < barWidth);
	const showSteps = $derived(stepWidth < beatWidth);
	const gridStyle = $derived.by((): string => {
		const layers: [string, number][] = [['--grid-major', barWidth]];
		if (showBeats) layers.push(['--edge', beatWidth]);
		if (showSteps) layers.push(['--grid-minor', stepWidth]);
		const image = layers
			.map(([color]) => `linear-gradient(90deg, var(${color}) 0 1px, transparent 1px)`)
			.join(', ');
		const size = layers.map(([, period]) => `${period}px 100%`).join(', ');
		return `background-image: ${image}; background-size: ${size}; background-repeat: repeat-x`;
	});
	const activeTake = $derived(session.audioTake?.track === index ? session.audioTake : null);
	const takeLengthBeats = $derived(
		activeTake === null
			? 0
			: Math.max(
					0.25,
					(session.snapshot?.transportPositionBeats ?? activeTake.startBeats) - activeTake.startBeats
				)
	);

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
			const editing = drag !== null && drag.kind !== 'move' && home && drag.index === position;
			out.push({
				key: `block-${position}`,
				block,
				index: position,
				startBeats: editing ? drag.startBeats : block.startBeats,
				lengthBeats: editing ? drag.lengthBeats : block.lengthBeats,
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

	function onContextMenu(event: MouseEvent): void {
		if (event.target instanceof Element && event.target.closest('[data-block]') !== null) return;
		const lane = event.currentTarget;
		if (!(lane instanceof HTMLElement)) return;
		event.preventDefault();
		const beats = pxToBeats(event.clientX - lane.getBoundingClientRect().left, session.zoom);
		const beatsPerBar = session.snapshot?.beatsPerBar ?? 4;
		menu.open(event.clientX, event.clientY, laneMenu(session, index, track.kind, beats, beatsPerBar, gridStep(session.zoom)));
	}
</script>

<div
	class="lane"
	role="list"
	aria-label="Blocks on {track.name}"
	data-lane-track={index}
	style="width: {width}px; height: {height ?? ROW_HEIGHT}px; {gridStyle}"
	oncontextmenu={onContextMenu}
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
	{#if activeTake !== null}
		<div
			class="recording-take"
			style="left: {beatsToPx(activeTake.startBeats, session.zoom)}px; width: {beatsToPx(takeLengthBeats, session.zoom)}px"
			aria-label="Recording audio take"
		>
			<span class="rec-dot"></span>
			<span class="rec-label">REC</span>
		</div>
	{/if}
</div>

<style>
	.lane {
		position: relative;
		flex: none;
		border-bottom: 1px solid var(--border-subtle);
		background-color: var(--bg);
	}

	.recording-take {
		position: absolute;
		top: 2px;
		bottom: 2px;
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 0 8px;
		background: color-mix(in srgb, var(--alert) 24%, var(--bg));
		border: 1px solid var(--alert);
		color: var(--text);
		font-size: 10px;
		letter-spacing: 0.12em;
		pointer-events: none;
		z-index: 3;
	}

	.rec-dot {
		width: 6px;
		height: 6px;
		background: var(--alert);
	}

	.rec-label {
		color: var(--alert);
	}
</style>
