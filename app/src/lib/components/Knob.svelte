<script lang="ts">
	/**
	 * A device dial in the Kono style: a square cell holding a 270 degree arc, the
	 * value arc in green, a pointer line, and the value with its unit underneath.
	 * Drag up or down to turn it (200 px sweep the range, Shift is ten times
	 * finer), scroll over it, or use the arrows, Page Up/Down, Home and End while
	 * it has focus. A double-click resets it to the default. Log parameters move
	 * through `knob.ts`'s normalized position. The dragged value is drawn from
	 * local state until the snapshot catches up, so the dial never jitters.
	 */

	import { onDestroy } from 'svelte';

	import { arcPath, clampParam, dragParam, formatParam, pointerAt, speakParam, stepParam, toNormal, type StepSize } from '$lib/knob';
	import type { ParamSpecView } from '$lib/wire';

	const SIZE = 40;
	const CENTER = SIZE / 2;
	const RADIUS = 14;
	const SETTLE_MS = 400;
	const SETTLE_EPSILON = 1e-4;

	interface Props {
		spec: ParamSpecView;
		value: number;
		label: string;
		disabled?: boolean;
		onbegin: () => void;
		oninput: (value: number) => void;
	}

	let { spec, value, label, disabled = false, onbegin, oninput }: Props = $props();

	let local = $state<number | null>(null);
	let dragging = $state(false);
	let anchor = { y: 0, value: 0, fine: false };
	let settle: ReturnType<typeof setTimeout> | null = null;

	const shown = $derived(local ?? clampParam(value, spec));
	const position = $derived(toNormal(shown, spec));
	const tip = $derived(pointerAt(CENTER, CENTER, RADIUS - 4, position));
	const track = arcPath(CENTER, CENTER, RADIUS, 0, 1);

	$effect(() => {
		if (local !== null && !dragging && Math.abs(value - local) < SETTLE_EPSILON) local = null;
	});

	onDestroy(() => {
		if (settle !== null) clearTimeout(settle);
	});

	function set(next: number): void {
		if (settle !== null) clearTimeout(settle);
		settle = null;
		if (next === shown) return;
		local = next;
		oninput(next);
		if (!dragging) release();
	}

	function release(): void {
		if (settle !== null) clearTimeout(settle);
		settle = setTimeout(() => {
			settle = null;
			if (!dragging) local = null;
		}, SETTLE_MS);
	}

	function onPointerDown(event: PointerEvent): void {
		if (event.button !== 0 || disabled) return;
		const element = event.currentTarget;
		if (!(element instanceof HTMLElement)) return;
		event.preventDefault();
		element.focus();
		element.setPointerCapture(event.pointerId);
		onbegin();
		dragging = true;
		anchor = { y: event.clientY, value: shown, fine: event.shiftKey };
	}

	function onPointerMove(event: PointerEvent): void {
		if (!dragging) return;
		if (event.shiftKey !== anchor.fine) anchor = { y: event.clientY, value: shown, fine: event.shiftKey };
		set(dragParam(anchor.value, spec, event.clientY - anchor.y, anchor.fine));
	}

	function onPointerUp(): void {
		if (!dragging) return;
		dragging = false;
		release();
	}

	function onWheel(event: WheelEvent): void {
		if (disabled || event.deltaY === 0) return;
		event.preventDefault();
		set(stepParam(shown, spec, -event.deltaY, event.shiftKey ? 'fine' : 'normal'));
	}

	function keyStep(event: KeyboardEvent): { direction: number; size: StepSize } | 'min' | 'max' | null {
		const size: StepSize = event.shiftKey ? 'fine' : 'normal';
		if (event.key === 'ArrowUp' || event.key === 'ArrowRight') return { direction: 1, size };
		if (event.key === 'ArrowDown' || event.key === 'ArrowLeft') return { direction: -1, size };
		if (event.key === 'PageUp') return { direction: 1, size: 'big' };
		if (event.key === 'PageDown') return { direction: -1, size: 'big' };
		if (event.key === 'Home') return 'min';
		if (event.key === 'End') return 'max';
		return null;
	}

	function onKeyDown(event: KeyboardEvent): void {
		const move = keyStep(event);
		if (move === null || disabled) return;
		event.preventDefault();
		event.stopPropagation();
		if (!event.repeat) onbegin();
		if (move === 'min') set(clampParam(spec.min, spec));
		else if (move === 'max') set(clampParam(spec.max, spec));
		else set(stepParam(shown, spec, move.direction, move.size));
	}

	function onDoubleClick(): void {
		if (disabled) return;
		onbegin();
		set(clampParam(spec.default, spec));
	}
</script>

<div class="knob" class:disabled>
	<span class="name" aria-hidden="true">{spec.name}</span>
	<div
		class="dial"
		class:dragging
		role="slider"
		tabindex={disabled ? -1 : 0}
		aria-label={label}
		aria-valuemin={spec.min}
		aria-valuemax={spec.max}
		aria-valuenow={Number(shown.toFixed(4))}
		aria-valuetext={speakParam(shown, spec)}
		aria-disabled={disabled ? 'true' : undefined}
		title="Drag or scroll to change, Shift for fine steps, double-click to reset"
		onpointerdown={onPointerDown}
		onpointermove={onPointerMove}
		onpointerup={onPointerUp}
		onpointercancel={onPointerUp}
		onwheel={onWheel}
		onkeydown={onKeyDown}
		ondblclick={onDoubleClick}
	>
		<svg width={SIZE} height={SIZE} viewBox="0 0 {SIZE} {SIZE}" aria-hidden="true">
			<path class="track" d={track} />
			<path class="value" d={arcPath(CENTER, CENTER, RADIUS, 0, position)} />
			<line class="pointer" x1={CENTER} y1={CENTER} x2={tip.x} y2={tip.y} />
		</svg>
	</div>
	<span class="readout" aria-hidden="true">{formatParam(shown, spec)}</span>
</div>

<style>
	.knob {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 4px;
		width: 64px;
		flex: none;
	}

	.name {
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 9px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--text-subtle);
	}

	.dial {
		display: flex;
		width: 44px;
		height: 44px;
		align-items: center;
		justify-content: center;
		background: var(--bg);
		box-shadow: inset 0 0 0 1px var(--border-subtle);
		cursor: ns-resize;
		touch-action: none;
		user-select: none;
	}

	.dial:hover,
	.dial.dragging {
		box-shadow: inset 0 0 0 1px var(--border);
	}

	.dial:focus-visible {
		outline-offset: 1px;
	}

	.track,
	.value {
		fill: none;
		stroke-width: 3;
		stroke-linecap: butt;
	}

	.track {
		stroke: var(--border);
	}

	.value {
		stroke: var(--green);
	}

	.pointer {
		stroke: var(--text);
		stroke-width: 1.5;
	}

	.readout {
		font-size: 10px;
		color: var(--text);
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}

	.disabled .value {
		stroke: var(--dim);
	}

	.disabled .dial {
		cursor: default;
	}
</style>
