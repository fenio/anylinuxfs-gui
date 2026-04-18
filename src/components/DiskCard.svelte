<script lang="ts">
	import type { Partition } from '$lib/types';
	import { disks } from '$lib/stores/disks';
	import { status, mountedDevices } from '$lib/stores/status';

	interface Props {
		partition: Partition;
		onRequestPassphrase: (device: string, readOnly: boolean, extraOptions: string, ignorePermissions: boolean) => void;
	}

	const DEFAULT_CHIPS = ['noatime', 'nodiratime', 'nobarrier', 'compress-force=zstd:5'];
	const CHIPS_STORAGE_KEY = 'mountOptionChips';

	let { partition, onRequestPassphrase }: Props = $props();

	let mounting = $derived($disks.mountingDevices.has(partition.device));
	let alreadyMounted = $derived($mountedDevices.has(partition.device));
	let isUnavailable = $derived(partition.mounted_by_system || !partition.supported);

	// Storage key for per-drive options: prefer UUID, fall back to device path
	function storageKey(): string {
		const id = partition.uuid || partition.device;
		return `mountOptions:${id}`;
	}

	function ignorePermsKey(): string {
		const id = partition.uuid || partition.device;
		return `ignorePerms:${id}`;
	}

	function loadIgnorePerms(): boolean {
		try {
			return localStorage.getItem(ignorePermsKey()) === 'true';
		} catch {
			return false;
		}
	}

	function saveIgnorePerms(val: boolean) {
		try {
			if (val) {
				localStorage.setItem(ignorePermsKey(), 'true');
			} else {
				localStorage.removeItem(ignorePermsKey());
			}
		} catch {
			// Ignore storage errors
		}
	}

	// Load saved options for this drive
	function loadSavedOptions(): string {
		try {
			return localStorage.getItem(storageKey()) || '';
		} catch {
			return '';
		}
	}

	// Save options for this drive
	function saveOptions(opts: string) {
		try {
			if (opts) {
				localStorage.setItem(storageKey(), opts);
			} else {
				localStorage.removeItem(storageKey());
			}
		} catch {
			// Ignore storage errors
		}
	}

	// Load/save editable chips list (global)
	function loadChips(): string[] {
		try {
			const stored = localStorage.getItem(CHIPS_STORAGE_KEY);
			if (stored) return JSON.parse(stored);
		} catch {
			// Ignore parse errors
		}
		return [...DEFAULT_CHIPS];
	}

	function saveChips(chips: string[]) {
		try {
			localStorage.setItem(CHIPS_STORAGE_KEY, JSON.stringify(chips));
		} catch {
			// Ignore storage errors
		}
	}

	// Initialize state from localStorage
	let savedOptions = loadSavedOptions();
	let extraOptions = $state(savedOptions);
	let ignorePermissions = $state(loadIgnorePerms());
	let showOptions = $state(false);
	let quickChips = $state(loadChips());
	let addingChip = $state(false);
	let newChipValue = $state('');

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

	function removeChip(chip: string) {
		quickChips = quickChips.filter((c) => c !== chip);
		saveChips(quickChips);
		// Also remove from active options if it was active
		const parts = optionParts().filter((p) => p !== chip);
		extraOptions = parts.join(',');
	}

	function addChip() {
		const value = newChipValue.trim();
		if (value && !quickChips.includes(value)) {
			quickChips = [...quickChips, value];
			saveChips(quickChips);
		}
		newChipValue = '';
		addingChip = false;
	}

	function focusOnMount(node: HTMLElement) {
		node.focus();
	}

	function handleAddChipKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault();
			addChip();
		} else if (e.key === 'Escape') {
			newChipValue = '';
			addingChip = false;
		}
	}

	async function handleMount() {
		// Split ro out of extraOptions for the backend API
		const parts = optionParts();
		const ro = parts.includes('ro');
		const opts = parts.filter((p) => p !== 'ro').join(',');

		// Save the full options string (including ro) for this drive
		saveOptions(extraOptions);
		saveIgnorePerms(ignorePermissions);

		if (partition.encrypted) {
			onRequestPassphrase(partition.device, ro, opts, ignorePermissions);
		} else {
			const result = await disks.mount(partition.device, undefined, ro, opts, ignorePermissions);
			if (result === 'encryption_required') {
				onRequestPassphrase(partition.device, ro, opts, ignorePermissions);
			} else {
				status.refresh();
			}
		}
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
			{#if (extraOptions || ignorePermissions) && !showOptions && !isUnavailable}
				<div class="saved-options" title={[ignorePermissions ? '--ignore-permissions' : '', extraOptions].filter(Boolean).join(' ')}>
					<span class="saved-options-label">opts:</span> {[ignorePermissions ? '--ignore-permissions' : '', extraOptions].filter(Boolean).join(' ')}
				</div>
			{/if}
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
						disabled={mounting || alreadyMounted}
					/>
					<span>RO</span>
				</label>
				<div class="split-btn">
					<button
						class="mount-btn"
						onclick={handleMount}
						disabled={mounting || alreadyMounted}
						title={alreadyMounted ? 'Already mounted' : 'Mount this partition'}
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
						disabled={mounting || alreadyMounted}
						title="Mount options"
					>+</button>
				</div>
			</div>
		{/if}
	</div>
	{#if showOptions && !isUnavailable}
		<div class="options-panel">
			<label class="flag-toggle" title="Bypass Unix file permissions: files appear owned by the current macOS user (--ignore-permissions)">
				<input
					type="checkbox"
					bind:checked={ignorePermissions}
					disabled={mounting || alreadyMounted}
				/>
				<span>Ignore permissions</span>
			</label>
			<input
				class="options-input"
				type="text"
				bind:value={extraOptions}
				placeholder="option1,option2"
				disabled={mounting || alreadyMounted}
			/>
			<div class="chips">
				{#each quickChips as chip}
					<span class="chip-wrapper" class:active={isChipActive(chip, extraOptions)}>
						<button
							class="chip"
							class:active={isChipActive(chip, extraOptions)}
							onclick={() => toggleChip(chip)}
							disabled={mounting || alreadyMounted}
						>{chip}</button>
						<button
							class="chip-remove"
							onclick={() => removeChip(chip)}
							title="Remove option"
						>&times;</button>
					</span>
				{/each}
				{#if addingChip}
					<input
						class="chip-input"
						type="text"
						bind:value={newChipValue}
						onkeydown={handleAddChipKeydown}
						onblur={addChip}
						placeholder="option"
						use:focusOnMount
					/>
				{:else}
					<button
						class="chip chip-add"
						onclick={() => (addingChip = true)}
						title="Add custom option"
					>+</button>
				{/if}
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

	.saved-options {
		display: flex;
		align-items: baseline;
		gap: 4px;
		font-size: 11px;
		font-family: monospace;
		color: var(--text-secondary);
		margin-top: 4px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.saved-options-label {
		color: var(--text-tertiary);
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

	.flag-toggle {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 13px;
		color: var(--text-secondary);
		cursor: pointer;
	}

	.flag-toggle input {
		cursor: pointer;
	}

	.flag-toggle input:disabled {
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

	.chip-wrapper {
		display: inline-flex;
		align-items: stretch;
		border-radius: 12px;
		border: 1px solid var(--border-color);
		background: var(--card-bg);
		transition: all 0.15s;
	}

	.chip-wrapper:hover {
		border-color: var(--accent-color);
	}

	.chip-wrapper.active {
		background: var(--accent-color);
		border-color: var(--accent-color);
	}

	.chip {
		padding: 4px 6px 4px 10px;
		border-radius: 12px 0 0 12px;
		border: none;
		background: transparent;
		color: var(--text-secondary);
		font-size: 12px;
		font-family: monospace;
		cursor: pointer;
		transition: color 0.15s;
	}

	.chip-wrapper:hover .chip {
		color: var(--text-primary);
	}

	.chip-wrapper.active .chip {
		color: white;
	}

	.chip:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.chip-remove {
		padding: 4px 8px 4px 2px;
		border: none;
		border-radius: 0 12px 12px 0;
		background: transparent;
		color: var(--text-tertiary);
		font-size: 14px;
		line-height: 1;
		cursor: pointer;
		transition: color 0.15s;
	}

	.chip-remove:hover {
		color: var(--error-color);
	}

	.chip-wrapper.active .chip-remove {
		color: rgba(255, 255, 255, 0.6);
	}

	.chip-wrapper.active .chip-remove:hover {
		color: white;
	}

	.chip-input {
		width: 100px;
		padding: 4px 10px;
		border-radius: 12px;
		border: 1px solid var(--accent-color);
		background: var(--card-bg);
		color: var(--text-primary);
		font-size: 12px;
		font-family: monospace;
		outline: none;
	}

	.chip-input::placeholder {
		color: var(--text-tertiary);
	}

	.chip-add {
		padding: 4px 12px;
		border-radius: 12px;
		border: 1px dashed var(--border-color);
		background: transparent;
		color: var(--text-tertiary);
		font-size: 14px;
		cursor: pointer;
		transition: all 0.15s;
	}

	.chip-add:hover {
		border-color: var(--accent-color);
		color: var(--accent-color);
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
