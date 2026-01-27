<script lang="ts">
	import { onMount } from 'svelte';
	import MountStatus from '../components/MountStatus.svelte';
	import DiskList from '../components/DiskList.svelte';
	import { checkCli } from '$lib/api';
	import { status } from '$lib/stores/status';

	let cliMissing = $state(false);
	let vmNotInitialized = $state(false);

	async function checkCliStatus() {
		const cliStatus = await checkCli();
		cliMissing = !cliStatus.available;
		vmNotInitialized = cliStatus.available && !cliStatus.initialized;
	}

	onMount(() => {
		checkCliStatus();
	});

	// Recheck init status when mount status changes (VM gets initialized on first mount)
	$effect(() => {
		if ($status.info.mounted && vmNotInitialized) {
			checkCliStatus();
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
				<p>Searched in PATH and common locations.</p>
				<p>Install it with: <code>brew install nohajc/anylinuxfs/anylinuxfs</code></p>
			</div>
		</div>
	{:else if vmNotInitialized}
		<div class="cli-warning init-warning">
			<span class="warning-icon">i</span>
			<div class="warning-content">
				<strong>First run setup required</strong>
				<p>The first mount will download the Linux VM image (~50 MB).</p>
				<p>This may take a minute depending on your connection.</p>
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

	.init-warning {
		background: #eff6ff;
		border-color: #3b82f6;
		color: #1e40af;
	}

	.init-warning .warning-icon {
		background: #3b82f6;
		color: white;
		width: 24px;
		height: 24px;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 14px;
		font-weight: bold;
		font-style: italic;
	}
</style>
