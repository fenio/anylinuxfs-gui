<script lang="ts">
	import { status, isMounted } from '$lib/stores/status';
	import { disks } from '$lib/stores/disks';
	import { forceCleanup, setTrayUnmountEnabled } from '$lib/api';
	import { Timeouts } from '$lib/constants';
	import { logAction, logError } from '$lib/logger';
	import { parseError } from '$lib/errors';
	let unmountingDevices = $state(new Set<string>());
	let cleaning = $state(false);
	let error = $state<string | null>(null);

	// Sync tray "Unmount" menu item enabled state with mount status
	$effect(() => {
		setTrayUnmountEnabled($isMounted).catch(() => {});
	});

	async function handleUnmount(device: string) {
		unmountingDevices = new Set([...unmountingDevices, device]);
		await disks.unmount(device);
		await new Promise((r) => setTimeout(r, Timeouts.LOG_POLL_INTERVAL));
		unmountingDevices = new Set([...unmountingDevices].filter((d) => d !== device));
		status.refresh();
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

{#if $status.mounts.length > 0}
	{#each $status.mounts as mount (mount.device)}
		<div class="mount-status mounted">
			<div class="status-icon">
				<span class="icon-mounted" aria-hidden="true"></span>
				<span class="sr-only">Success</span>
			</div>
			<div class="status-info">
				<div class="status-label">Mounted</div>
				<div class="status-details">
					<span class="detail-item">{mount.device}</span>
					<span class="detail-item">{mount.mount_point}</span>
					{#if mount.filesystem}
						<span class="detail-item fs-badge">{mount.filesystem}</span>
					{/if}
				</div>
			</div>
			<button
				class="unmount-btn"
				onclick={() => handleUnmount(mount.device)}
				disabled={unmountingDevices.has(mount.device)}
			>
				{unmountingDevices.has(mount.device) ? 'Unmounting...' : 'Unmount'}
			</button>
		</div>
	{/each}
{/if}

{#if $disks.mountingDevices.size > 0}
	{#each [...$disks.mountingDevices] as device (device)}
		<div class="mount-status mounting">
			<div class="status-icon" role="status" aria-busy="true">
				<span class="spinner" aria-hidden="true"></span>
				<span class="sr-only">Loading</span>
			</div>
			<div class="status-info">
				<div class="status-label">Mounting...</div>
				<div class="status-details">
					<span class="detail-item">{device}</span>
				</div>
			</div>
		</div>
	{/each}
{/if}

{#if $status.mounts.length === 0 && $disks.mountingDevices.size === 0}
	<div class="mount-status not-mounted">
		<div class="status-info">
			<div class="status-label">No disk mounted</div>
			<div class="status-details">
				<span class="detail-item">{$disks.disks.length > 0 ? 'Select a partition below to mount' : 'Connect a drive to get started'}</span>
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
		margin-bottom: 8px;
	}

	.mount-status:last-child {
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
