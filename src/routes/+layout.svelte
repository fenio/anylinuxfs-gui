<script lang="ts">
	import '../app.css';
	import Sidebar from '../components/Sidebar.svelte';
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();
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
