<script lang="ts">
	import type { Partition } from '$lib/types';
	import { disks } from '$lib/stores/disks';
	import { status, isMounted } from '$lib/stores/status';

	interface Props {
		partition: Partition;
		onRequestPassphrase: (device: string) => void;
	}

	let { partition, onRequestPassphrase }: Props = $props();

	let mounting = $derived($disks.mountingDevice === partition.device);
	let isUnavailable = $derived(partition.mounted_by_system || !partition.supported);
	let isDisabled = $derived(isUnavailable || $isMounted);

	async function handleMount() {
		if (partition.encrypted) {
			onRequestPassphrase(partition.device);
		} else {
			await disks.mount(partition.device);
			// Always refresh status after mount attempt
			status.refresh();
		}
	}
</script>

<div class="disk-card" class:unavailable={isUnavailable}>
	<div class="partition-info">
		<div class="partition-device">
			<span class="device-name">{partition.device}</span>
			{#if partition.encrypted}
				<span class="encrypted-badge" title="Encrypted">Encrypted</span>
			{/if}
			{#if partition.mounted_by_system}
				<span class="mounted-badge" title="Already mounted by macOS">Mounted</span>
			{/if}
			{#if !partition.supported}
				<span class="unsupported-badge" title={partition.support_note || 'Unsupported'}>Unsupported</span>
			{/if}
		</div>
		<div class="partition-details">
			{#if partition.label}
				<span class="detail label">{partition.label}</span>
			{/if}
			<span class="detail filesystem">{partition.filesystem}</span>
			<span class="detail size">{partition.size}</span>
			{#if partition.system_mount_point}
				<span class="detail mount-point" title={partition.system_mount_point}>{partition.system_mount_point}</span>
			{/if}
		</div>
	</div>
	{#if partition.mounted_by_system}
		<span class="status-note">In use by macOS</span>
	{:else if !partition.supported}
		<span class="status-note" title={partition.support_note || ''}>{partition.support_note || 'Unsupported filesystem'}</span>
	{:else}
		<button
			class="mount-btn"
			onclick={handleMount}
			disabled={mounting || $isMounted}
			title={$isMounted ? 'Unmount current disk first' : 'Mount this partition'}
		>
			{#if mounting}
				<span class="spinner"></span>
				Mounting...
			{:else}
				Mount
			{/if}
		</button>
	{/if}
</div>

<style>
	.disk-card {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 16px;
		background: var(--card-bg);
		border: 1px solid var(--border-color);
		border-radius: 8px;
		transition: border-color 0.15s;
	}

	.disk-card:hover {
		border-color: var(--border-hover);
	}

	.partition-info {
		flex: 1;
	}

	.partition-device {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.device-name {
		font-family: monospace;
		font-size: 14px;
		font-weight: 500;
		color: var(--text-primary);
	}

	.encrypted-badge {
		font-size: 10px;
		padding: 2px 6px;
		border-radius: 4px;
		background: var(--warning-bg);
		color: var(--warning-color);
		font-weight: 500;
	}

	.mounted-badge {
		font-size: 10px;
		padding: 2px 6px;
		border-radius: 4px;
		background: var(--neutral-bg);
		color: var(--text-secondary);
		font-weight: 500;
	}

	.unsupported-badge {
		font-size: 10px;
		padding: 2px 6px;
		border-radius: 4px;
		background: #fef3c7;
		color: #b45309;
		font-weight: 500;
	}

	.disk-card.unavailable {
		opacity: 0.6;
		background: var(--neutral-bg);
	}

	.status-note {
		font-size: 12px;
		color: var(--text-tertiary);
		font-style: italic;
		text-align: right;
		white-space: nowrap;
	}

	.mount-point {
		font-family: monospace;
		font-size: 11px;
		max-width: 150px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.partition-details {
		display: flex;
		gap: 8px;
		margin-top: 4px;
	}

	.detail {
		font-size: 12px;
		color: var(--text-secondary);
	}

	.detail.filesystem {
		font-family: monospace;
		background: var(--badge-bg);
		padding: 1px 6px;
		border-radius: 4px;
	}

	.mount-btn {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 8px 16px;
		border-radius: 6px;
		border: none;
		background: var(--accent-color);
		color: white;
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
		transition: background-color 0.15s;
	}

	.mount-btn:hover:not(:disabled) {
		background: var(--accent-hover);
	}

	.mount-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.spinner {
		width: 12px;
		height: 12px;
		border: 2px solid rgba(255, 255, 255, 0.3);
		border-top-color: white;
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
