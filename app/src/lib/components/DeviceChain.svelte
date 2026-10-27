<script lang="ts">
	/**
	 * One track's device chain, left to right in signal order: the instrument on
	 * a MIDI track or the sample source on an audio track, then the insert
	 * effects, then a drop zone with a `+` button listing the effects the chain
	 * does not have yet. Dragging a title bar reorders the effects, with a line
	 * marking the gap the card drops into; an effect dragged from the browser
	 * drops into a gap the same way. Dials go through the session's per-frame
	 * queue, with a checkpoint at the start of each gesture so it undoes in one
	 * step.
	 */

	import { onDestroy, onMount } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';

	import { addableKinds, dropIndex, indicatorX, moveTarget, specFor, type Span } from '$lib/chain';
	import { effectDrag } from '$lib/effectDrag.svelte';
	import { deviceMenu, type MenuEntry } from '$lib/menu';
	import { menu } from '$lib/menu.svelte';
	import { trackColor } from '$lib/palette';
	import { session } from '$lib/session.svelte';
	import type { EffectKind, TrackView } from '$lib/wire';
	import Device from './Device.svelte';
	import Knob from './Knob.svelte';
	import VoiceEditor from './VoiceEditor.svelte';

	const GAP = 6;
	const THRESHOLD_PX = 4;

	interface Props {
		track: TrackView;
		index: number;
	}

	let { track, index }: Props = $props();

	interface Reorder {
		from: number;
		x: number;
		y: number;
		active: boolean;
		slot: number;
	}

	const folded = new SvelteSet<string>();
	let scroller = $state<HTMLDivElement | null>(null);
	let row = $state<HTMLDivElement | null>(null);
	let reorder = $state<Reorder | null>(null);
	let browserSlot = $state<number | null>(null);
	let lineX = $state<number | null>(null);

	const allTracks = $derived(session.snapshot?.tracks ?? []);
	const catalog = $derived(session.snapshot?.effectCatalog ?? []);
	const effects = $derived(track.effects);
	const addable = $derived(addableKinds(catalog, effects));
	const slot = $derived(reorder?.active ? reorder.slot : browserSlot);

	$effect(() => {
		const ghost = effectDrag.ghost;
		browserSlot = ghost === null || !over(ghost.x, ghost.y) ? null : dropIndex(ghost.x, spans());
	});

	$effect(() => {
		const current = slot;
		const content = row;
		if (current === null || content === null) {
			lineX = null;
			return;
		}
		const x = indicatorX(current, spans(), GAP);
		lineX = x === null ? null : x - content.getBoundingClientRect().left;
	});

	onMount(() => {
		effectDrag.target = (kind, x, y) => {
			if (over(x, y)) session.addEffect(index, kind, dropIndex(x, spans()));
		};
	});

	onDestroy(() => {
		effectDrag.target = null;
		stopReorder();
	});

	function spans(): Span[] {
		if (row === null) return [];
		return [...row.querySelectorAll('[data-effect-card]')].map((card) => {
			const rect = card.getBoundingClientRect();
			return { left: rect.left, width: rect.width };
		});
	}

	function over(x: number, y: number): boolean {
		if (scroller === null) return false;
		const rect = scroller.getBoundingClientRect();
		return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
	}

	function foldKey(key: number | string): string {
		return `${index}:${key}`;
	}

	function toggleFold(key: string): void {
		if (folded.has(key)) folded.delete(key);
		else folded.add(key);
	}

	function move(from: number, direction: number): void {
		const to = from + direction;
		if (to < 0 || to >= effects.length) return;
		session.dispatch({ moveEffect: { track: index, from, to } });
	}

	function remove(at: number): void {
		session.dispatch({ removeEffect: { track: index, index: at } });
	}

	function openDeviceMenu(event: MouseEvent, at: number, id: number): void {
		const key = foldKey(id);
		menu.open(event.clientX, event.clientY, deviceMenu(session, index, at, effects.length, folded.has(key), () => toggleFold(key)));
	}

	function openAddMenu(event: MouseEvent): void {
		const button = event.currentTarget;
		if (!(button instanceof HTMLElement)) return;
		const rect = button.getBoundingClientRect();
		const entries: MenuEntry[] = addable.map((spec) => ({
			kind: 'item',
			label: spec.name,
			run: () => session.addEffect(index, spec.kind)
		}));
		menu.open(rect.left, rect.bottom + 2, entries, button);
	}

	function setChoice(at: number, param: number, choice: number): void {
		session.dispatch('checkpoint');
		session.dispatch({ setEffectParam: { track: index, index: at, param, value: choice } });
	}

	function grab(event: PointerEvent, from: number): void {
		stopReorder();
		reorder = { from, x: event.clientX, y: event.clientY, active: false, slot: from };
		window.addEventListener('pointermove', onReorderMove);
		window.addEventListener('pointerup', onReorderEnd);
		window.addEventListener('pointercancel', onReorderEnd);
	}

	function onReorderMove(event: PointerEvent): void {
		if (reorder === null) return;
		if (event.buttons === 0) {
			stopReorder();
			return;
		}
		if (!reorder.active && Math.hypot(event.clientX - reorder.x, event.clientY - reorder.y) < THRESHOLD_PX) return;
		event.preventDefault();
		reorder = { ...reorder, active: true, slot: dropIndex(event.clientX, spans()) };
	}

	function onReorderEnd(event: PointerEvent): void {
		const done = reorder;
		stopReorder();
		if (done === null || !done.active || event.type === 'pointercancel') return;
		const to = moveTarget(done.from, dropIndex(event.clientX, spans()));
		if (to !== null) session.dispatch({ moveEffect: { track: index, from: done.from, to } });
	}

	function stopReorder(): void {
		reorder = null;
		window.removeEventListener('pointermove', onReorderMove);
		window.removeEventListener('pointerup', onReorderEnd);
		window.removeEventListener('pointercancel', onReorderEnd);
	}
</script>

<div class="scroller" bind:this={scroller} role="list" aria-label="Devices on {track.name}">
	<div class="row" bind:this={row} style="gap: {GAP}px">
		<div class="slot" role="listitem">
			{#if track.kind === 'midi'}
				<Device
					name="Instrument"
					folded={folded.has(foldKey('source'))}
					enabled={track.instrumentEnabled}
					onfold={() => toggleFold(foldKey('source'))}
					ontoggle={() => session.dispatch({ setInstrumentEnabled: { track: index, enabled: !track.instrumentEnabled } })}
				>
					<VoiceEditor {track} {index} />
				</Device>
			{:else}
				<Device name="Sample" folded={folded.has(foldKey('source'))} onfold={() => toggleFold(foldKey('source'))}>
					<p class="note">The block's sample plays through the chain.</p>
				</Device>
			{/if}
		</div>

		{#each effects as effect, at (effect.id)}
			{@const spec = specFor(catalog, effect.kind)}
			{@const key = foldKey(effect.id)}
			<div class="slot" role="listitem">
				<Device
					name={effect.name}
					folded={folded.has(key)}
					enabled={effect.enabled}
					effect
					dragging={reorder?.active === true && reorder.from === at}
					onfold={() => toggleFold(key)}
					ontoggle={() => session.dispatch({ setEffectEnabled: { track: index, index: at, enabled: !effect.enabled } })}
					ondelete={() => remove(at)}
					onmove={(direction) => move(at, direction)}
					onmenu={(event) => openDeviceMenu(event, at, effect.id)}
					ongrab={(event) => grab(event, at)}
				>
					{#if spec === null}
						<p class="note">The engine lists no controls for {effect.name}.</p>
					{:else}
						<div class="params">
							{#each spec.params as param, p (param.name)}
								{#if param.unit === 'choice'}
									<div class="choice" role="group" aria-label="{param.name} of {effect.name}">
										<span class="label">{param.name}</span>
										<div class="segments">
											{#each param.choices as choice, c (choice)}
												<button
													type="button"
													class="segment"
													aria-pressed={Math.round(effect.params[p] ?? param.default) === c}
													onclick={() => setChoice(at, p, c)}
												>
													{choice}
												</button>
											{/each}
										</div>
									</div>
								{:else}
									<Knob
										spec={param}
										value={effect.params[p] ?? param.default}
										label="{param.name} of {effect.name}"
										onbegin={() => session.dispatch('checkpoint')}
										oninput={(value) => session.queueEffectParam(index, at, p, value)}
									/>
								{/if}
							{/each}
						</div>

						{#if spec.takesSidechain}
							<div class="sidechain-picker" role="group" aria-label="Sidechain for {effect.name}">
								<span class="sidechain-label">Sidechain</span>
								<select
									class="sidechain-select"
									aria-label="Sidechain source for {effect.name}"
									value={effect.sidechain ?? ''}
									onchange={(event) => {
										const val = event.currentTarget.value;
										const source = val === '' ? null : Number(val);
										session.setEffectSidechain(index, at, source);
									}}
								>
									<option value="">Off</option>
									{#each allTracks as otherTrack, t (t)}
										{#if t !== index}
											<option value={t}>{otherTrack.name}</option>
										{/if}
									{/each}
								</select>
								{#if effect.sidechain !== null && allTracks[effect.sidechain]}
									<span
										class="track-swatch"
										style="background: {trackColor(allTracks[effect.sidechain].colorIndex)}"
										title="Sidechain: {allTracks[effect.sidechain].name}"
									></span>
								{/if}
							</div>
						{/if}
					{/if}
				</Device>
			</div>
		{/each}

		<div class="slot add" role="listitem" class:target={slot !== null && effects.length === 0}>
			<span class="hint">Drop an effect here</span>
			<button
				type="button"
				class="plus"
				aria-label="Add an effect to {track.name}"
				title={addable.length === 0 ? 'Track chain is full (maximum 8 effects)' : 'Pick an effect to add'}
				disabled={addable.length === 0}
				onclick={openAddMenu}
			>
				+
			</button>
		</div>

		{#if lineX !== null}
			<div class="line" style="left: {lineX - 1}px" aria-hidden="true"></div>
		{/if}
	</div>
</div>

<style>
	.scroller {
		flex: 1;
		min-height: 0;
		overflow-x: auto;
		overflow-y: hidden;
		background: var(--bg);
	}

	.row {
		position: relative;
		display: flex;
		align-items: stretch;
		height: 100%;
		width: max-content;
		padding: 8px;
	}

	.slot {
		display: flex;
		min-height: 0;
	}

	.note {
		max-width: 200px;
		font-size: 11px;
		line-height: 1.5;
		color: var(--dim);
	}

	.params {
		display: flex;
		align-items: flex-start;
		gap: 8px;
	}

	.choice {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.label {
		font-size: 9px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--text-subtle);
	}

	.segments {
		display: flex;
		flex-direction: column;
	}

	.segment {
		height: 22px;
		padding: 0 8px;
		justify-content: flex-start;
		background: transparent;
		border-bottom: 1px solid var(--border-subtle);
		color: var(--text-subtle);
		font-size: 10px;
	}

	.segment:hover:not(:disabled) {
		filter: none;
		color: var(--text);
		background: var(--surface-hover);
	}

	.segment[aria-pressed='true'] {
		background: var(--green);
		color: var(--on-green);
	}

	.add {
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 8px;
		width: 160px;
		border: 1px dashed var(--border);
		color: var(--text-subtle);
	}

	.add.target {
		border-color: var(--green);
		background: var(--green-soft);
	}

	.hint {
		font-size: 10px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
	}

	.plus {
		width: 28px;
		height: 28px;
		padding: 0;
		font-size: 15px;
		letter-spacing: 0;
		background: transparent;
		border: 1px solid var(--border);
		color: var(--text);
	}

	.plus:hover:not(:disabled) {
		filter: none;
		border-color: var(--green);
		color: var(--green);
	}

	.line {
		position: absolute;
		top: 8px;
		bottom: 8px;
		width: 2px;
		background: var(--green);
		pointer-events: none;
	}

	.sidechain-picker {
		display: flex;
		align-items: center;
		gap: 6px;
		margin-top: 10px;
		padding-top: 8px;
		border-top: 1px solid var(--border-subtle);
	}

	.sidechain-label {
		font-size: 9px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		color: var(--text-subtle);
	}

	.sidechain-select {
		flex: 1;
		min-width: 0;
		height: 22px;
		padding: 0 4px;
		font-size: 11px;
		color: var(--text);
		background: var(--bg);
		border: 1px solid var(--border-subtle);
	}

	.sidechain-select:focus-visible {
		outline: 1px solid var(--green);
	}

	.track-swatch {
		flex: none;
		width: 8px;
		height: 8px;
	}
</style>
