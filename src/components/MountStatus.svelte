<script lang="ts">
	import { status, isMounted } from '$lib/stores/status';
	import { disks } from '$lib/stores/disks';
	import { forceCleanup } from '$lib/api';
	import { onMount, onDestroy } from 'svelte';

	let unmounting = $state(false);
	let cleaning = $state(false);

	onMount(() => {
		status.startPolling(2000);
	});

	onDestroy(() => {
		status.stopPolling();
	});

	async function handleUnmount() {
		unmounting = true;
		status.stopPolling();
		await disks.unmount();
		// Wait for VM cleanup before checking status
		await new Promise((r) => setTimeout(r, 1000));
		unmounting = false;
		status.refresh();
		status.startPolling(2000);
	}

	async function handleForceCleanup() {
		cleaning = true;
		try {
			await forceCleanup();
		} catch (e) {
			console.error('Force cleanup failed:', e);
		}
		cleaning = false;
		status.refresh();
	}
</script>

{#if $isMounted}
	<div class="mount-status mounted">
		<div class="status-icon">
			<span class="icon-mounted"></span>
		</div>
		<div class="status-info">
			<div class="status-label">Mounted</div>
			<div class="status-details">
				{#if $status.info.device}
					<span class="detail-item">{$status.info.device}</span>
				{/if}
				{#if $status.info.mount_point}
					<span class="detail-item">{$status.info.mount_point}</span>
				{/if}
				{#if $status.info.filesystem}
					<span class="detail-item fs-badge">{$status.info.filesystem}</span>
				{/if}
			</div>
		</div>
		<button
			class="unmount-btn"
			onclick={handleUnmount}
			disabled={unmounting}
		>
			{unmounting ? 'Unmounting...' : 'Unmount'}
		</button>
	</div>
{:else if $disks.mountingDevice}
	<div class="mount-status mounting">
		<div class="status-icon">
			<span class="spinner"></span>
		</div>
		<div class="status-info">
			<div class="status-label">
				{$disks.mountingDevice === 'unmounting' ? 'Unmounting...' : 'Mounting...'}
			</div>
			{#if $disks.mountingDevice !== 'unmounting'}
				<div class="status-details">
					<span class="detail-item">{$disks.mountingDevice}</span>
				</div>
			{/if}
		</div>
	</div>
{:else if $status.info.orphaned_instance && !$disks.recentUnmount}
	<div class="mount-status orphaned">
		<div class="status-icon">
			<span class="icon-warning"></span>
		</div>
		<div class="status-info">
			<div class="status-label">Orphaned instance detected</div>
			<div class="status-details">
				<span class="detail-item">A previous mount failed but the VM is still running.</span>
			</div>
		</div>
		<button
			class="cleanup-btn"
			onclick={handleForceCleanup}
			disabled={cleaning}
		>
			{cleaning ? 'Cleaning...' : 'Force Cleanup'}
		</button>
	</div>
{:else}
	<div class="mount-status not-mounted">
		<div class="status-info">
			<div class="status-label">No disk mounted</div>
			<div class="status-details">
				<span class="detail-item">Select a partition below to mount</span>
			</div>
		</div>
	</div>
{/if}

<style>
	.mount-status {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 12px 16px;
		border-radius: 8px;
		margin-bottom: 16px;
	}

	.mount-status.mounted {
		background: var(--success-bg);
		border: 1px solid var(--success-border);
	}

	.mount-status.not-mounted {
		background: var(--neutral-bg);
		border: 1px solid var(--border-color);
	}

	.mount-status.orphaned {
		background: #fef3c7;
		border: 1px solid #f59e0b;
	}

	.mount-status.mounting {
		background: #eff6ff;
		border: 1px solid #3b82f6;
	}

	.status-icon {
		width: 32px;
		height: 32px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 50%;
	}

	.icon-mounted::before {
		content: '\2713';
		font-size: 18px;
		color: var(--success-color);
	}

	.icon-warning::before {
		content: '\26A0';
		font-size: 18px;
		color: #b45309;
	}

	.status-info {
		flex: 1;
	}

	.status-label {
		font-weight: 600;
		font-size: 14px;
		color: var(--text-primary);
	}

	.status-details {
		display: flex;
		gap: 8px;
		margin-top: 2px;
		flex-wrap: wrap;
	}

	.detail-item {
		font-size: 12px;
		color: var(--text-secondary);
	}

	.fs-badge {
		background: var(--badge-bg);
		padding: 1px 6px;
		border-radius: 4px;
		font-family: monospace;
	}

	.unmount-btn {
		padding: 6px 14px;
		border-radius: 6px;
		border: none;
		background: var(--button-secondary-bg);
		color: var(--text-primary);
		font-size: 13px;
		cursor: pointer;
		transition: background-color 0.15s;
	}

	.unmount-btn:hover:not(:disabled) {
		background: var(--button-secondary-hover);
	}

	.unmount-btn:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.cleanup-btn {
		padding: 6px 14px;
		border-radius: 6px;
		border: none;
		background: #b45309;
		color: white;
		font-size: 13px;
		cursor: pointer;
		transition: background-color 0.15s;
	}

	.cleanup-btn:hover:not(:disabled) {
		background: #92400e;
	}

	.cleanup-btn:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.spinner {
		width: 20px;
		height: 20px;
		border: 2px solid #93c5fd;
		border-top-color: #3b82f6;
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
