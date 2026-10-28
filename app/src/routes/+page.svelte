<script lang="ts">
	/**
	 * The session window: the transport on top, the sidebar beside the timeline,
	 * the detail pane (clip or devices) under the timeline, and the status bar
	 * below. Every piece of state lives in the session store.
	 */

	import { onMount } from 'svelte';
	import { getCurrentWebview } from '@tauri-apps/api/webview';

	import ContextMenu from '$lib/components/ContextMenu.svelte';
	import DetailPane from '$lib/components/DetailPane.svelte';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import StatusBar from '$lib/components/StatusBar.svelte';
	import Timeline from '$lib/components/Timeline.svelte';
	import Mixer from '$lib/components/Mixer.svelte';
	import Settings from '$lib/components/Settings.svelte';
	import Transport from '$lib/components/Transport.svelte';
	import { AUDIO_EXTS } from '$lib/audio';
	import { runDropQueue } from '$lib/dropQueue';
	import type { BlockView } from '$lib/wire';
	import { trackDropUnder } from '$lib/drag.svelte';
	import { freeStart, gridStep, snapBeats } from '$lib/geometry';
	import { projectTitle } from '$lib/project';
	import { session } from '$lib/session.svelte';
	import { settingsStore } from '$lib/settings.svelte';

	function onWindowKeyDown(event: KeyboardEvent): void {
		if (event.key === ',' && (event.metaKey || event.ctrlKey) && !event.shiftKey && !event.altKey) {
			event.preventDefault();
			settingsStore.toggle();
			return;
		}
		if (
			event.key === 'Tab' &&
			!event.shiftKey &&
			!event.ctrlKey &&
			!event.altKey &&
			!event.metaKey
		) {
			const target = event.target;
			if (
				target instanceof HTMLInputElement ||
				target instanceof HTMLTextAreaElement ||
				target instanceof HTMLSelectElement ||
				(target instanceof HTMLElement && target.isContentEditable)
			) {
				return;
			}
			if (settingsStore.isOpen) {
				return;
			}
			event.preventDefault();
			session.activeView = session.activeView === 'timeline' ? 'mixer' : 'timeline';
		}
	}

	onMount(() => session.attachKeyboard());

	onMount(() => {
		session.start();
		return () => session.stop();
	});

	$effect(() => {
		if (typeof document !== 'undefined') {
			document.title = projectTitle(
				session.snapshot?.projectPath ?? null,
				session.snapshot?.projectDirty ?? false
			);
		}
	});

	onMount(() => {
		if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) {
			return;
		}
		let disposed = false;
		let unlisten: (() => void) | undefined;
		void getCurrentWebview().onDragDropEvent((event) => {
			if (event.payload.type !== 'drop') return;
			const { paths, position } = event.payload;
			const dpr = window.devicePixelRatio || 1;
			const dropTarget =
				trackDropUnder(position.x / dpr, position.y / dpr, session.zoom) ??
				trackDropUnder(position.x, position.y, session.zoom);
			let targetTrack = dropTarget?.track ?? session.snapshot?.cursor.track ?? 0;
			if (dropTarget === null && session.snapshot?.tracks[targetTrack]?.kind !== 'audio') {
				const audioIdx = session.snapshot?.tracks.findIndex((t) => t.kind === 'audio') ?? -1;
				if (audioIdx >= 0) targetTrack = audioIdx;
			}
			const blocks = session.snapshot?.tracks[targetTrack]?.blocks ?? [];
			const startBeats =
				dropTarget !== null && dropTarget.beats !== null
					? dropTarget.beats < 2.0
						? 0
						: snapBeats(dropTarget.beats, gridStep(session.zoom))
					: snapBeats(freeStart(blocks), gridStep(session.zoom));
			const audio = paths.filter((path) => AUDIO_EXTS.test(path));
			for (const path of paths) {
				if (path.endsWith('.cassette') || path.endsWith('.json')) {
					session.dispatch({ loadProject: { path } });
				}
			}
			let known = new Set<string>();
			const blockKey = (block: BlockView) => `${block.startBeats}:${block.lengthBeats}:${block.source}`;
			const current = () => session.snapshot?.tracks[targetTrack]?.blocks ?? [];
			void runDropQueue({
				paths: audio,
				first: startBeats,
				mark: () => {
					known = new Set(current().map(blockKey));
				},
				send: (path, start) =>
					session.request({ importSample: { track: targetTrack, path, startBeats: start } }),
				arrived: () => {
					const block = current().find((item) => !known.has(blockKey(item)));
					return block ? block.startBeats + block.lengthBeats : null;
				},
				fail: (path) => session.report(`could not import ${path}`)
			});
		}).then((cleanup) => {
			if (disposed) cleanup();
			else unlisten = cleanup;
		});
		return () => {
			disposed = true;
			unlisten?.();
		};
	});
</script>

<svelte:window onkeydown={onWindowKeyDown} />

<div class="app">
	<Transport bind:activeView={session.activeView} />
	<div class="body">
		<Sidebar />
		<div class="main">
			<div class="lanes">
				{#if session.activeView === 'timeline'}
					<Timeline />
				{:else}
					<Mixer />
				{/if}
			</div>
			<DetailPane />
		</div>
	</div>
	<StatusBar />
</div>

<ContextMenu />
<Settings />

<style>
	.app {
		display: flex;
		flex-direction: column;
		height: 100vh;
		background: var(--bg);
	}

	.body {
		flex: 1;
		min-height: 0;
		display: flex;
	}

	.main {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
	}

	.lanes {
		flex: 1;
		min-height: 0;
		display: flex;
	}
</style>
