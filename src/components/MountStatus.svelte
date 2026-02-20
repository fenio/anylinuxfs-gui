<script lang="ts">
	import { status, isMounted } from '$lib/stores/status';
	import { disks } from '$lib/stores/disks';
	import { forceCleanup, setTrayUnmountEnabled } from '$lib/api';
	import { Timeouts } from '$lib/constants';
	import { logAction, logError } from '$lib/logger';
	import { parseError } from '$lib/errors';
	import { onMount, onDestroy } from 'svelte';

	let unmounting = $state(false);
	let cleaning = $state(false);
	let error = $state<string | null>(null);

	onMount(() => {
		status.startListening();
	});

	onDestroy(() => {
		status.stopListening();
	});

	// Sync tray "Unmount" menu item enabled state with mount status
	$effect(() => {
		setTrayUnmountEnabled($isMounted).catch(() => {});
	});

	async function handleUnmount() {
		unmounting = true;
		status.stopListening();
		await disks.unmount();
		// Wait for VM cleanup before checking status
		await new Promise((r) => setTimeout(r, Timeouts.LOG_POLL_INTERVAL));
		unmounting = false;
		status.refresh();
		status.startListening();
	}

	async function handleForceCleanup() {
		cleaning = true;
		error = null;
		try {
			logAction('Force cleanup started');
			await forceCleanup();
			logAction('Force cleanup completed');
		} catch (e) {
			logError('forceCleanup', e);
			error = parseError(e).message;
		}
		cleaning = false;
		status.refresh();
	}
</script>

{#if error}
	<div class="cleanup-error" role="alert">
		<span class="error-message">Force cleanup failed: {error}</span>
		<button class="dismiss-btn" onclick={() => (error = null)}>Dismiss</button>
	</div>
{/if}

{#if $isMounted}
	<div class="mount-status mounted">
		<div class="status-icon">
			<span class="icon-mounted" aria-hidden="true"></span>
			<span class="sr-only">Success</span>
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
		<div class="status-icon" role="status" aria-busy="true">
			<span class="spinner" aria-hidden="true"></span>
			<span class="sr-only">Loading</span>
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
			<span class="icon-warning" aria-hidden="true"></span>
			<span class="sr-only">Warning</span>
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
	.cleanup-error {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 10px 16px;
		background: var(--error-bg);
		border: 1px solid var(--error-border);
		border-radius: 8px;
		margin-bottom: 8px;
	}

	.cleanup-error .error-message {
		flex: 1;
		font-size: 13px;
		color: var(--error-color);
	}

	.cleanup-error .dismiss-btn {
		padding: 4px 10px;
		border-radius: 4px;
		border: none;
		background: var(--error-color);
		color: white;
		font-size: 12px;
		cursor: pointer;
		flex-shrink: 0;
	}

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
		background: var(--warning-bg-solid);
		border: 1px solid var(--warning-border);
	}

	.mount-status.mounting {
		background: var(--info-bg);
		border: 1px solid var(--info-border);
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
		color: var(--warning-border);
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
		background: var(--warning-border);
		color: white;
		font-size: 13px;
		cursor: pointer;
		transition: background-color 0.15s;
	}

	.cleanup-btn:hover:not(:disabled) {
		background: var(--warning-text);
	}

	.cleanup-btn:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.spinner {
		width: 20px;
		height: 20px;
		border: 2px solid var(--spinner-border);
		border-top-color: var(--spinner-border-top);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		padding: 0;
		margin: -1px;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
		border-width: 0;
	}
</style>
