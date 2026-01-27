<script lang="ts">
	import LogViewer from '../../components/LogViewer.svelte';
</script>

<svelte:head>
	<title>Logs - anylinuxfs</title>
</svelte:head>

<div class="page">
	<LogViewer />
</div>

<style>
	.page {
		display: flex;
		flex-direction: column;
		height: 100%;
	}
</style>
