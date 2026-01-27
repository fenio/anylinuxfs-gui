<script lang="ts">
	import ConfigPanel from '../../components/ConfigPanel.svelte';
</script>

<svelte:head>
	<title>Settings - anylinuxfs</title>
</svelte:head>

<div class="page">
	<ConfigPanel />
</div>

<style>
	.page {
		display: flex;
		flex-direction: column;
		height: 100%;
	}
</style>
