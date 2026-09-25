<script lang="ts">
	/**
	 * The session window: the transport on top, the sidebar beside the timeline,
	 * and the status bar below. Every piece of state lives in the session store.
	 */

	import { onMount } from 'svelte';

	import Sidebar from '$lib/components/Sidebar.svelte';
	import StatusBar from '$lib/components/StatusBar.svelte';
	import Timeline from '$lib/components/Timeline.svelte';
	import Transport from '$lib/components/Transport.svelte';
	import { session } from '$lib/session.svelte';

	onMount(() => session.attachKeyboard());

	onMount(() => {
		session.start();
		return () => session.stop();
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
