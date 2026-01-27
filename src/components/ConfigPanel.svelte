<script lang="ts">
	import { config } from '$lib/stores/config';
	import { onMount } from 'svelte';

	let ramMb = $state(2048);
	let vcpus = $state(2);
	let logLevel = $state('info');
	let hasChanges = $state(false);

	const logLevels = ['off', 'error', 'warn', 'info', 'debug', 'trace'];
	const ramOptions = [512, 1024, 2048, 4096, 8192, 16384];
	const vcpuOptions = [1, 2, 4, 8, 16];

	onMount(() => {
		config.load();
	});

	$effect(() => {
		if ($config.config.ram_mb !== null) {
			ramMb = $config.config.ram_mb;
		}
		if ($config.config.vcpus !== null) {
			vcpus = $config.config.vcpus;
		}
		if ($config.config.log_level !== null) {
			logLevel = $config.config.log_level;
		}
	});

	function checkChanges() {
		const cfg = $config.config;
		hasChanges =
			(cfg.ram_mb !== null && ramMb !== cfg.ram_mb) ||
			(cfg.vcpus !== null && vcpus !== cfg.vcpus) ||
			(cfg.log_level !== null && logLevel !== cfg.log_level) ||
			(cfg.ram_mb === null && ramMb !== 2048) ||
			(cfg.vcpus === null && vcpus !== 2) ||
			(cfg.log_level === null && logLevel !== 'info');
	}

	async function handleSave() {
		await config.save(ramMb, vcpus, logLevel);
		hasChanges = false;
	}

	function handleReset() {
		const cfg = $config.config;
		ramMb = cfg.ram_mb ?? 2048;
		vcpus = cfg.vcpus ?? 2;
		logLevel = cfg.log_level ?? 'info';
		hasChanges = false;
	}

	function formatRam(mb: number): string {
		if (mb >= 1024) {
			return `${mb / 1024} GB`;
		}
		return `${mb} MB`;
	}
</script>

<div class="config-panel">
	<div class="header">
		<h2>Settings</h2>
	</div>

	{#if $config.error}
		<div class="error-banner">
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
					<select
						id="ram"
						bind:value={ramMb}
						onchange={checkChanges}
					>
						{#each ramOptions as option}
							<option value={option}>{formatRam(option)}</option>
						{/each}
					</select>
					<span class="hint">More RAM improves performance for large file operations.</span>
				</div>

				<div class="setting">
					<label for="vcpus">vCPUs</label>
					<select
						id="vcpus"
						bind:value={vcpus}
						onchange={checkChanges}
					>
						{#each vcpuOptions as option}
							<option value={option}>{option} {option === 1 ? 'core' : 'cores'}</option>
						{/each}
					</select>
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

	.setting select {
		width: 100%;
		max-width: 200px;
		padding: 8px 12px;
		border: 1px solid var(--border-color);
		border-radius: 6px;
		font-size: 14px;
		background: var(--input-bg);
		color: var(--text-primary);
		cursor: pointer;
	}

	.setting select:focus {
		border-color: var(--accent-color);
		outline: none;
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
