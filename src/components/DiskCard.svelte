<script lang="ts">
	import type { Partition } from '$lib/types';
	import { disks } from '$lib/stores/disks';
	import { status, isMounted } from '$lib/stores/status';

	interface Props {
		partition: Partition;
		onRequestPassphrase: (device: string, readOnly: boolean, extraOptions: string) => void;
	}

	let { partition, onRequestPassphrase }: Props = $props();

	let extraOptions = $state('');
	let showOptions = $state(false);
	let mounting = $derived($disks.mountingDevice === partition.device);
	let isUnavailable = $derived(partition.mounted_by_system || !partition.supported);
	let isDisabled = $derived(isUnavailable || $isMounted);

	const quickChips = ['noatime', 'nodiratime', 'nobarrier', 'compress-force=zstd:5'];

	function optionParts(): string[] {
		return extraOptions.split(',').map((s) => s.trim()).filter(Boolean);
	}

	function readOnly(): boolean {
		return optionParts().includes('ro');
	}

	function toggleReadOnly(e: Event) {
		const checked = (e.target as HTMLInputElement).checked;
		toggleOption('ro', checked);
	}

	function toggleOption(opt: string, forceOn?: boolean) {
		const parts = optionParts();
		const idx = parts.indexOf(opt);
		const shouldAdd = forceOn !== undefined ? forceOn : idx < 0;
		if (shouldAdd && idx < 0) {
			parts.push(opt);
		} else if (!shouldAdd && idx >= 0) {
			parts.splice(idx, 1);
		}
		extraOptions = parts.join(',');
	}

	function toggleChip(chip: string) {
		toggleOption(chip);
	}

	function isChipActive(chip: string, opts: string): boolean {
		return opts.split(',').map((s) => s.trim()).includes(chip);
	}

	async function handleMount() {
		// Split ro out of extraOptions for the backend API
		const parts = optionParts();
		const ro = parts.includes('ro');
		const opts = parts.filter((p) => p !== 'ro').join(',');
		if (partition.encrypted) {
			onRequestPassphrase(partition.device, ro, opts);
		} else {
			const result = await disks.mount(partition.device, undefined, ro, opts);
			if (result === 'encryption_required') {
				onRequestPassphrase(partition.device, ro, opts);
			} else {
				status.refresh();
			}
		}
		extraOptions = '';
		showOptions = false;
	}
</script>

<div class="disk-card" class:unavailable={isUnavailable}>
	<div class="card-main">
		<div class="partition-info">
			<div class="partition-device">
				<span class="device-name">{partition.device}</span>
				{#if partition.encrypted}
					<span class="encrypted-badge" title="Encrypted">Encrypted</span>
				{/if}
				{#if partition.mounted_by_system}
					<span class="mounted-badge" title="Already mounted by macOS">Mounted</span>
				{/if}
				{#if !partition.supported && !partition.support_note}
					<span class="unsupported-badge">Unsupported</span>
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
			<div class="mount-controls">
				<label class="ro-toggle" title="Mount read-only">
					<input
						type="checkbox"
						checked={readOnly()}
						onchange={toggleReadOnly}
						disabled={mounting || $isMounted}
					/>
					<span>RO</span>
				</label>
				<div class="split-btn">
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
					<button
						class="options-toggle-btn"
						class:active={showOptions}
						onclick={() => (showOptions = !showOptions)}
						disabled={mounting || $isMounted}
						title="Mount options"
					>+</button>
				</div>
			</div>
		{/if}
	</div>
	{#if showOptions && !isUnavailable}
		<div class="options-panel">
			<input
				class="options-input"
				type="text"
				bind:value={extraOptions}
				placeholder="option1,option2"
				disabled={mounting || $isMounted}
			/>
			<div class="chips">
				{#each quickChips as chip}
					<button
						class="chip"
						class:active={isChipActive(chip, extraOptions)}
						onclick={() => toggleChip(chip)}
						disabled={mounting || $isMounted}
					>{chip}</button>
				{/each}
			</div>
		</div>
	{/if}
</div>

<style>
	.disk-card {
		display: flex;
		flex-direction: column;
		padding: 12px 16px;
		background: var(--card-bg);
		border: 1px solid var(--border-color);
		border-radius: 8px;
		transition: border-color 0.15s;
	}

	.disk-card:hover {
		border-color: var(--border-hover);
	}

	.card-main {
		display: flex;
		align-items: center;
		justify-content: space-between;
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
		background: var(--warning-bg-solid);
		color: var(--warning-border);
		font-weight: 500;
	}

	.disk-card.unavailable {
		opacity: 0.6;
		background: var(--neutral-bg);
	}

	.status-note {
		font-size: 12px;
		color: var(--text-secondary);
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

	.mount-controls {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.ro-toggle {
		display: flex;
		align-items: center;
		gap: 4px;
		font-size: 12px;
		color: var(--text-secondary);
		cursor: pointer;
		white-space: nowrap;
	}

	.ro-toggle input {
		cursor: pointer;
	}

	.ro-toggle input:disabled {
		cursor: not-allowed;
	}

	.split-btn {
		display: flex;
		align-items: stretch;
	}

	.mount-btn {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 8px 16px;
		border-radius: 6px 0 0 6px;
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

	.options-toggle-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 8px 10px;
		border-radius: 0 6px 6px 0;
		border: none;
		border-left: 1px solid rgba(255, 255, 255, 0.2);
		background: var(--accent-color);
		color: white;
		font-size: 15px;
		font-weight: 600;
		cursor: pointer;
		transition: background-color 0.15s;
	}

	.options-toggle-btn:hover:not(:disabled) {
		background: var(--accent-hover);
	}

	.options-toggle-btn.active {
		background: var(--accent-hover);
	}

	.options-toggle-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.options-panel {
		margin-top: 10px;
		padding-top: 10px;
		border-top: 1px solid var(--border-color);
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.options-input {
		width: 100%;
		padding: 6px 10px;
		border: 1px solid var(--border-color);
		border-radius: 6px;
		background: var(--card-bg);
		color: var(--text-primary);
		font-size: 13px;
		font-family: monospace;
		outline: none;
		box-sizing: border-box;
	}

	.options-input:focus {
		border-color: var(--accent-color);
	}

	.options-input::placeholder {
		color: var(--text-tertiary);
	}

	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}

	.chip {
		padding: 4px 10px;
		border-radius: 12px;
		border: 1px solid var(--border-color);
		background: var(--card-bg);
		color: var(--text-secondary);
		font-size: 12px;
		font-family: monospace;
		cursor: pointer;
		transition: all 0.15s;
	}

	.chip:hover:not(:disabled) {
		border-color: var(--accent-color);
		color: var(--text-primary);
	}

	.chip.active {
		background: var(--accent-color);
		color: white;
		border-color: var(--accent-color);
	}

	.chip:disabled {
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
