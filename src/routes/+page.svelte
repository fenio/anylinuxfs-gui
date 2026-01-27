<script lang="ts">
	import { onMount } from 'svelte';
	import MountStatus from '../components/MountStatus.svelte';
	import DiskList from '../components/DiskList.svelte';
	import { checkCli } from '$lib/api';

	let cliMissing = $state(false);
	let cliPath = $state('');

	onMount(async () => {
		const status = await checkCli();
		if (!status.available) {
			cliMissing = true;
			cliPath = status.path;
		}
	});
</script>

<svelte:head>
	<title>anylinuxfs</title>
</svelte:head>

<div class="page">
	{#if cliMissing}
		<div class="cli-warning">
			<span class="warning-icon">⚠</span>
			<div class="warning-content">
				<strong>anylinuxfs CLI not found</strong>
				<p>Expected at: <code>{cliPath}</code></p>
				<p>Install it with: <code>brew install anylinuxfs</code></p>
			</div>
		</div>
	{/if}

	<main class="main-content">
		<MountStatus />
		<DiskList />
	</main>
</div>

<style>
	.page {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--bg-primary);
	}

	.cli-warning {
		display: flex;
		align-items: flex-start;
		gap: 12px;
		padding: 16px;
		margin-bottom: 16px;
		background: #fef3c7;
		border: 1px solid #f59e0b;
		border-radius: 8px;
		color: #92400e;
	}

	.warning-icon {
		font-size: 24px;
		line-height: 1;
	}

	.warning-content {
		flex: 1;
	}

	.warning-content strong {
		display: block;
		margin-bottom: 4px;
	}

	.warning-content p {
		margin: 4px 0;
		font-size: 13px;
	}

	.warning-content code {
		background: rgba(0, 0, 0, 0.1);
		padding: 2px 6px;
		border-radius: 4px;
		font-family: monospace;
	}
</style>
