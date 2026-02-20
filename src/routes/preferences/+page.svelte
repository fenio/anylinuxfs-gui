<script lang="ts">
	import PreferencesPanel from '../../components/PreferencesPanel.svelte';
</script>

<svelte:head>
	<title>Preferences - anylinuxfs</title>
</svelte:head>

<div class="page">
	<PreferencesPanel />
</div>

<style>
	.page {
		display: flex;
		flex-direction: column;
		height: 100%;
	}
</style>
