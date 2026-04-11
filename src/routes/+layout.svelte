<script lang="ts">
	import '../app.css';
	import Sidebar from '../components/Sidebar.svelte';
	import { status } from '$lib/stores/status';
	import { onMount, onDestroy } from 'svelte';
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();

	onMount(() => {
		status.startListening();
	});

	onDestroy(() => {
		status.stopListening();
	});
</script>

<div id="app">
	<Sidebar />
	<main class="main-content">
		{@render children()}
	</main>
</div>

<style>
	.main-content {
		flex: 1;
		padding: 24px;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
	}
</style>
