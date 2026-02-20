<script lang="ts">
	import { config } from '$lib/stores/config';
	import { onMount } from 'svelte';

	let ramMb = $state(1024);
	let vcpus = $state(1);
	let logLevel = $state('off');
	let hasChanges = $state(false);

	const logLevels = ['off', 'error', 'warn', 'info', 'debug', 'trace'];

	onMount(() => {
		config.load();
	});

	$effect(() => {
		// Extract config to ensure store is tracked
		const cfg = $config.config;
		const loading = $config.loading;

		// Sync from config when loading completes
		if (!loading) {
			ramMb = cfg.ram_mb ?? 1024;
			vcpus = cfg.vcpus ?? 1;
			logLevel = cfg.log_level ?? 'off';
			hasChanges = false;
		}
	});

	function checkChanges() {
		const cfg = $config.config;
		hasChanges =
			(cfg.ram_mb !== null && ramMb !== cfg.ram_mb) ||
			(cfg.vcpus !== null && vcpus !== cfg.vcpus) ||
			(cfg.log_level !== null && logLevel !== cfg.log_level) ||
			(cfg.ram_mb === null && ramMb !== 1024) ||
			(cfg.vcpus === null && vcpus !== 1) ||
			(cfg.log_level === null && logLevel !== 'off');
	}

	async function handleSave() {
		await config.save(ramMb, vcpus, logLevel);
		hasChanges = false;
	}

	function handleReset() {
		const cfg = $config.config;
		ramMb = cfg.ram_mb ?? 1024;
		vcpus = cfg.vcpus ?? 1;
		logLevel = cfg.log_level ?? 'off';
		hasChanges = false;
	}

</script>

<div class="config-panel">
	<div class="header">
		<h2>Settings</h2>
	</div>

	{#if $config.error}
		<div class="error-banner" role="alert">
			<span>{$config.error}</span>
			<button onclick={() => config.clearError()}>Dismiss</button>
		</div>
	{/if}

	{#if $config.loading}
		<div class="loading">Loading configuration...</div>
	{:else}
		<div class="settings-grid">
			<div class="setting-group">
				<h3>Virtual Machine</h3>
				<p class="description">Configure the Linux VM used to mount filesystems.</p>

				<div class="setting">
					<label for="ram">Memory (RAM)</label>
					<div class="input-with-unit">
						<input
							type="number"
							id="ram"
							bind:value={ramMb}
							oninput={checkChanges}
							min="256"
							max="65536"
							step="128"
						/>
						<span class="unit">MB</span>
					</div>
					<span class="hint">More RAM improves performance for large file operations.</span>
				</div>

				<div class="setting">
					<label for="vcpus">vCPUs</label>
					<input
						type="number"
						id="vcpus"
						bind:value={vcpus}
						oninput={checkChanges}
						min="1"
						max="32"
					/>
					<span class="hint">More cores improve parallel file operations.</span>
				</div>
			</div>

			<div class="setting-group">
				<h3>Logging</h3>
				<p class="description">Configure logging verbosity.</p>

				<div class="setting">
					<label for="log-level">Log Level</label>
					<select
						id="log-level"
						bind:value={logLevel}
						onchange={checkChanges}
					>
						{#each logLevels as level}
							<option value={level}>{level.charAt(0).toUpperCase() + level.slice(1)}</option>
						{/each}
					</select>
					<span class="hint">Higher verbosity generates more log output.</span>
				</div>
			</div>
		</div>

		<div class="actions">
			<button
				class="btn-secondary"
				onclick={handleReset}
				disabled={!hasChanges || $config.saving}
			>
				Reset
			</button>
			<button
				class="btn-primary"
				onclick={handleSave}
				disabled={!hasChanges || $config.saving}
			>
				{#if $config.saving}
					Saving...
				{:else}
					Save Changes
				{/if}
			</button>
		</div>
	{/if}
</div>

<style>
	.config-panel {
		height: 100%;
		display: flex;
		flex-direction: column;
	}

	.header {
		margin-bottom: 20px;
	}

	.header h2 {
		font-size: 18px;
		font-weight: 600;
		color: var(--text-primary);
		margin: 0;
	}

	.error-banner {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 14px;
		background: var(--error-bg);
		border: 1px solid var(--error-border);
		border-radius: 6px;
		color: var(--error-color);
		font-size: 13px;
		margin-bottom: 16px;
	}

	.error-banner button {
		padding: 4px 10px;
		border-radius: 4px;
		border: none;
		background: var(--error-color);
		color: white;
		font-size: 12px;
		cursor: pointer;
	}

	.loading {
		padding: 24px;
		text-align: center;
		color: var(--text-secondary);
	}

	.settings-grid {
		flex: 1;
		overflow-y: auto;
	}

	.setting-group {
		background: var(--card-bg);
		border: 1px solid var(--border-color);
		border-radius: 8px;
		padding: 20px;
		margin-bottom: 16px;
	}

	.setting-group h3 {
		font-size: 15px;
		font-weight: 600;
		color: var(--text-primary);
		margin: 0 0 4px;
	}

	.setting-group .description {
		font-size: 13px;
		color: var(--text-secondary);
		margin: 0 0 16px;
	}

	.setting {
		margin-bottom: 16px;
	}

	.setting:last-child {
		margin-bottom: 0;
	}

	.setting label {
		display: block;
		font-size: 13px;
		font-weight: 500;
		color: var(--text-primary);
		margin-bottom: 6px;
	}

	.setting input[type='number'],
	.setting select {
		width: 100%;
		max-width: 200px;
		padding: 8px 12px;
		border: 1px solid var(--border-color);
		border-radius: 6px;
		font-size: 14px;
		background: var(--input-bg);
		color: var(--text-primary);
	}

	.setting select {
		cursor: pointer;
	}

	.setting input[type='number']:focus,
	.setting select:focus {
		border-color: var(--accent-color);
		outline: none;
	}

	.input-with-unit {
		display: flex;
		align-items: center;
		gap: 8px;
		max-width: 200px;
	}

	.input-with-unit input {
		flex: 1;
		min-width: 0;
	}

	.input-with-unit .unit {
		font-size: 13px;
		color: var(--text-secondary);
		white-space: nowrap;
	}

	.setting .hint {
		display: block;
		font-size: 12px;
		color: var(--text-tertiary);
		margin-top: 4px;
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: 10px;
		padding-top: 16px;
		border-top: 1px solid var(--border-color);
	}

	.btn-secondary,
	.btn-primary {
		padding: 8px 16px;
		border-radius: 6px;
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
	}

	.btn-secondary {
		border: 1px solid var(--border-color);
		background: var(--button-secondary-bg);
		color: var(--text-primary);
	}

	.btn-secondary:hover:not(:disabled) {
		background: var(--button-secondary-hover);
	}

	.btn-primary {
		border: none;
		background: var(--accent-color);
		color: white;
	}

	.btn-primary:hover:not(:disabled) {
		background: var(--accent-hover);
	}

	.btn-secondary:disabled,
	.btn-primary:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
