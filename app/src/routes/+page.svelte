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
	import Transport from '$lib/components/Transport.svelte';
	import { AUDIO_EXTS } from '$lib/audio';
	import { runDropQueue } from '$lib/dropQueue';
	import type { BlockView } from '$lib/wire';
	import { laneUnder } from '$lib/drag.svelte';
	import { freeStart, gridStep, snapBeats } from '$lib/geometry';
	import { session } from '$lib/session.svelte';

	onMount(() => session.attachKeyboard());

	onMount(() => {
		session.start();
		return () => session.stop();
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
			const clientX = position.x / dpr;
			const clientY = position.y / dpr;
			const lane = laneUnder(clientX, clientY, session.zoom);
			const targetTrack = lane?.track ?? session.snapshot?.cursor.track ?? 0;
			const blocks = session.snapshot?.tracks[targetTrack]?.blocks ?? [];
			const startBeats = lane
				? (lane.beats < 2.0 ? 0 : snapBeats(lane.beats, gridStep(session.zoom)))
				: snapBeats(freeStart(blocks), gridStep(session.zoom));
			const audio = paths.filter((path) => AUDIO_EXTS.test(path));
			for (const path of paths) {
				if (path.endsWith('.json')) session.dispatch({ loadProject: { path } });
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

<div class="app">
	<Transport />
	<div class="body">
		<Sidebar />
		<div class="main">
			<div class="lanes">
				<Timeline />
			</div>
			<DetailPane />
		</div>
	</div>
	<StatusBar />
</div>

<ContextMenu />

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
