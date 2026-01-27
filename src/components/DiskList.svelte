<script lang="ts">
	import { disks } from '$lib/stores/disks';
	import { status, isMounted } from '$lib/stores/status';
	import DiskCard from './DiskCard.svelte';
	import PassphraseDialog from './PassphraseDialog.svelte';
	import { onMount } from 'svelte';
	import { listen } from '@tauri-apps/api/event';
	import { startDiskWatcher, ejectDisk } from '$lib/api';

	let ejectingDevice: string | null = $state(null);

	let passphraseDevice: string | null = $state(null);

	onMount(() => {
		disks.refresh();

		// Start watching for disk changes
		startDiskWatcher().catch(console.error);

		// Listen for disk change events and auto-refresh
		const unlisten = listen('disks-changed', () => {
			disks.refresh();
		});

		return () => {
			unlisten.then((fn) => fn());
		};
	});

	// Clear mounting state when we detect mount succeeded via status polling
	$effect(() => {
		if ($isMounted && $disks.mountingDevice) {
			disks.clearMounting();
		}
	});

	function handleRequestPassphrase(device: string) {
		passphraseDevice = device;
	}

	async function handlePassphraseSubmit(passphrase: string) {
		if (passphraseDevice) {
			const success = await disks.mount(passphraseDevice, passphrase);
			if (success) {
				status.refresh();
			}
			passphraseDevice = null;
		}
	}

	function handlePassphraseCancel() {
		passphraseDevice = null;
	}

	function handleRefresh() {
		disks.refresh();
	}

	function handleAdminModeToggle(e: Event) {
		const checked = (e.target as HTMLInputElement).checked;
		disks.setAdminMode(checked);
		disks.refresh(checked);
	}

	async function handleEject(device: string) {
		ejectingDevice = device;
		try {
			await ejectDisk(device);
			// Refresh after successful eject
			disks.refresh();
		} catch (e) {
			console.error('Failed to eject:', e);
		} finally {
			ejectingDevice = null;
		}
	}
</script>

<div class="disk-list">
	<div class="header">
		<h2>Available Disks</h2>
		<div class="header-controls">
			<label class="admin-toggle" title="Show more details (requires admin password)">
				<input
					type="checkbox"
					checked={$disks.adminMode}
					onchange={handleAdminModeToggle}
					disabled={$disks.loading}
				/>
				<span>Admin mode</span>
			</label>
			<button class="refresh-btn" onclick={handleRefresh} disabled={$disks.loading}>
				{#if $disks.loading}
					<span class="spinner"></span>
				{:else}
					Refresh
				{/if}
			</button>
		</div>
	</div>

	{#if $disks.error}
		<div class="error-banner">
			<span class="error-icon">!</span>
			<span class="error-message">{$disks.error}</span>
			<button class="dismiss-btn" onclick={() => disks.clearError()}>Dismiss</button>
		</div>
	{/if}

	{#if !$disks.loading && !$disks.hasSupportedPartitions && !$disks.adminMode && $disks.disks.length > 0}
		<div class="admin-hint">
			<span class="hint-icon">i</span>
			<span class="hint-text">
				No supported filesystems detected. Enable <strong>Admin mode</strong> to detect Linux filesystems (ext4, btrfs, etc.)
			</span>
		</div>
	{/if}

	{#if $disks.loading && $disks.disks.length === 0}
		<div class="loading">
			<span class="spinner large"></span>
			<span>Scanning disks...</span>
		</div>
	{:else if $disks.disks.length === 0}
		<div class="empty">
			<p>No Linux disks found.</p>
			<p class="hint">Connect a drive with a Linux filesystem to get started.</p>
		</div>
	{:else}
		{#each $disks.disks as disk}
			<div class="disk-group">
				<div class="disk-header">
					<span class="disk-device">{disk.device}</span>
					{#if disk.model}
						<span class="disk-model">{disk.model}</span>
					{/if}
					<span class="disk-size">{disk.size}</span>
					{#if disk.is_external}
						<button
							class="eject-btn"
							onclick={() => handleEject(disk.device)}
							disabled={ejectingDevice === disk.device}
							title="Eject disk (safely remove)"
						>
							{#if ejectingDevice === disk.device}
								<span class="spinner small"></span>
							{:else}
								⏏
							{/if}
						</button>
					{/if}
				</div>
				<div class="partitions">
					{#each disk.partitions as partition}
						<DiskCard {partition} onRequestPassphrase={handleRequestPassphrase} />
					{/each}
				</div>
			</div>
		{/each}
	{/if}
</div>

{#if passphraseDevice}
	<PassphraseDialog
		device={passphraseDevice}
		onSubmit={handlePassphraseSubmit}
		onCancel={handlePassphraseCancel}
	/>
{/if}

<style>
	.disk-list {
		flex: 1;
		overflow-y: auto;
	}

	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 16px;
	}

	.header h2 {
		font-size: 18px;
		font-weight: 600;
		color: var(--text-primary);
		margin: 0;
	}

	.header-controls {
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.admin-toggle {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		color: var(--text-secondary);
		cursor: pointer;
	}

	.admin-toggle input {
		cursor: pointer;
	}

	.admin-toggle input:disabled {
		cursor: not-allowed;
	}

	.refresh-btn {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 6px 12px;
		border-radius: 6px;
		border: 1px solid var(--border-color);
		background: var(--button-secondary-bg);
		color: var(--text-primary);
		font-size: 13px;
		cursor: pointer;
		transition: background-color 0.15s;
	}

	.refresh-btn:hover:not(:disabled) {
		background: var(--button-secondary-hover);
	}

	.refresh-btn:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.error-banner {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 12px 16px;
		background: var(--error-bg);
		border: 1px solid var(--error-border);
		border-radius: 8px;
		margin-bottom: 16px;
	}

	.error-icon {
		width: 20px;
		height: 20px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--error-color);
		color: white;
		border-radius: 50%;
		font-size: 12px;
		font-weight: bold;
	}

	.error-message {
		flex: 1;
		font-size: 13px;
		color: var(--error-color);
	}

	.dismiss-btn {
		padding: 4px 10px;
		border-radius: 4px;
		border: none;
		background: var(--error-color);
		color: white;
		font-size: 12px;
		cursor: pointer;
	}

	.admin-hint {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 12px 16px;
		background: #eff6ff;
		border: 1px solid #3b82f6;
		border-radius: 8px;
		margin-bottom: 16px;
	}

	.hint-icon {
		width: 20px;
		height: 20px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: #3b82f6;
		color: white;
		border-radius: 50%;
		font-size: 12px;
		font-weight: bold;
		font-style: italic;
	}

	.hint-text {
		flex: 1;
		font-size: 13px;
		color: #1e40af;
	}

	.hint-text strong {
		color: #1e3a8a;
	}

	.loading,
	.empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 48px;
		color: var(--text-secondary);
	}

	.loading {
		gap: 12px;
	}

	.empty p {
		margin: 4px 0;
	}

	.empty .hint {
		font-size: 13px;
		color: var(--text-tertiary);
	}

	.disk-group {
		margin-bottom: 20px;
	}

	.disk-header {
		display: flex;
		align-items: center;
		gap: 10px;
		margin-bottom: 8px;
		padding: 0 4px;
	}

	.disk-device {
		font-family: monospace;
		font-weight: 600;
		color: var(--text-primary);
	}

	.disk-model {
		font-size: 13px;
		color: var(--text-secondary);
	}

	.disk-size {
		font-size: 12px;
		color: var(--text-tertiary);
		margin-left: auto;
	}

	.eject-btn {
		padding: 4px 8px;
		border-radius: 4px;
		border: 1px solid var(--border-color);
		background: var(--button-secondary-bg);
		color: var(--text-secondary);
		font-size: 14px;
		cursor: pointer;
		transition: all 0.15s;
		display: flex;
		align-items: center;
		justify-content: center;
		min-width: 32px;
	}

	.eject-btn:hover:not(:disabled) {
		background: var(--button-secondary-hover);
		color: var(--text-primary);
	}

	.eject-btn:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.spinner.small {
		width: 12px;
		height: 12px;
		border-width: 2px;
	}

	.partitions {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.spinner {
		width: 14px;
		height: 14px;
		border: 2px solid var(--border-color);
		border-top-color: var(--accent-color);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	.spinner.large {
		width: 24px;
		height: 24px;
		border-width: 3px;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
