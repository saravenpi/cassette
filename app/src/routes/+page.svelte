<script lang="ts">
	/**
	 * The session window: the transport on top, the sidebar beside the timeline,
	 * and the status bar below. Every piece of state lives in the session store.
	 */

	import { onMount } from 'svelte';
	import { getCurrentWebview } from '@tauri-apps/api/webview';

	import Sidebar from '$lib/components/Sidebar.svelte';
	import StatusBar from '$lib/components/StatusBar.svelte';
	import Timeline from '$lib/components/Timeline.svelte';
	import Transport from '$lib/components/Transport.svelte';
	import { laneUnder } from '$lib/drag.svelte';
	import { freeStart, snapBeats } from '$lib/geometry';
	import { session } from '$lib/session.svelte';

	const AUDIO_EXTS = /\.(wav|mp3|flac|m4a|aac|ogg|oga|opus|aiff|aif|caf|alac|wma)$/i;

	onMount(() => session.attachKeyboard());

	onMount(() => {
		session.start();
		return () => session.stop();
	});

	onMount(() => {
		if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) {
			return;
		}
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
				? (lane.beats < 2.0 ? 0 : snapBeats(lane.beats))
				: snapBeats(freeStart(blocks));
			for (const path of paths) {
				if (path.endsWith('.json')) {
					session.dispatch({ loadProject: { path } });
				} else if (AUDIO_EXTS.test(path)) {
					session.dispatch({ importSample: { track: targetTrack, path, startBeats } });
				}
			}
		}).then((cleanup) => {
			unlisten = cleanup;
		});
		return () => {
			unlisten?.();
		};
	});
</script>

<div class="app">
	<Transport />
	<div class="body">
		<Sidebar />
		<Timeline />
	</div>
	<StatusBar />
</div>

<style>
	.app {
		display: flex;
		flex-direction: column;
		height: 100vh;
	}

	.body {
		flex: 1;
		min-height: 0;
		display: flex;
	}
</style>
