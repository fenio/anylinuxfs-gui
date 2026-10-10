<script lang="ts">
	import { selectKeyFile } from '#lib/api';
	import { elevation } from '#lib/stores/elevation';
	import { parseError } from '#lib/errors';
	interface Props {
		device: string;
		description?: string;
		errorMessage?: string | null;
		submitting?: boolean;
		onSubmit: (passphrase?: string, keyFile?: string) => void;
		onCancel: () => void;
	}

	let { device, description, errorMessage = null, submitting = false, onSubmit, onCancel }: Props = $props();

	let passphrase = $state('');
	let method = $state<'passphrase' | 'key_file'>('passphrase');
	let keyFile = $state('');
	let selecting = $state(false);
	let pickerError = $state<string | null>(null);
	let terminalPassphrase = $derived(method === 'passphrase' && $elevation.policy.mode === 'interactive_terminal');
	let canSubmit = $derived(!submitting && !selecting && (method === 'key_file' ? !!keyFile : terminalPassphrase || !!passphrase.trim()));

	async function chooseKeyFile() {
		selecting = true;
		pickerError = null;
		try {
			const selected = await selectKeyFile();
			if (selected) keyFile = selected;
		} catch (error) {
			pickerError = parseError(error).message;
		} finally {
			selecting = false;
		}
	}
	let showPassphrase = $state(false);
	let inputEl: HTMLInputElement | undefined = $state();

	$effect(() => {
		if (inputEl) {
			inputEl.focus();
		}
	});

	function handleSubmit(e: Event) {
		e.preventDefault();
		if (canSubmit) {
			onSubmit(method === 'passphrase' && !terminalPassphrase ? passphrase : undefined, method === 'key_file' ? keyFile : undefined);
		}
	}

	function handleCancel() {
		if (submitting || selecting) return;
		passphrase = '';
		keyFile = '';
		showPassphrase = false;
		onCancel();
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			handleCancel();
		}
	}
</script>

<div class="overlay" role="dialog" aria-modal="true" tabindex="-1" onkeydown={handleKeydown}>
	<div class="dialog">
		<div class="dialog-header">
			<h3>Unlock Encrypted Partition</h3>
		</div>
		<div class="dialog-body">
			<p class="device-info">
				Choose how to unlock <code>{device}</code>.
			</p>
			{#if description}<p class="device-info">{description}</p>{/if}
			{#if errorMessage}
				<p class="passphrase-error" role="alert">{errorMessage}</p>
			{/if}
			<form onsubmit={handleSubmit}>
				<label for="unlock-method">Unlock method</label>
				<select id="unlock-method" bind:value={method} disabled={submitting || selecting} onchange={() => { passphrase = ''; pickerError = null; }}>
					<option value="passphrase">Passphrase or recovery key</option>
					<option value="key_file">Key file</option>
				</select>
				{#if method === 'key_file'}
					<label for="key-file">Key file</label>
					<div class="input-wrapper">
						<input id="key-file" value={keyFile} readonly placeholder="No file selected" title={keyFile} />
						<button type="button" class="btn-secondary" onclick={chooseKeyFile} disabled={submitting || selecting}>Browse…</button>
					</div>
					{#if pickerError}<p class="passphrase-error" role="alert">{pickerError}</p>{/if}
				{:else if terminalPassphrase}
					<p>Enter the disk passphrase in Terminal after clicking Mount.</p>
				{:else}
				<label for="passphrase">Passphrase or recovery key</label>
				<div class="input-wrapper">
					<input
						bind:this={inputEl}
						id="passphrase"
						type={showPassphrase ? 'text' : 'password'}
						bind:value={passphrase}
						placeholder="Enter passphrase or recovery key"
						autocomplete="off"
						autocorrect="off"
						spellcheck="false"
						disabled={submitting}
					/>
					<button
						type="button"
						class="toggle-visibility"
						onclick={() => (showPassphrase = !showPassphrase)}
						title={showPassphrase ? 'Hide passphrase' : 'Show passphrase'}
					>
						{showPassphrase ? 'Hide' : 'Show'}
					</button>
				</div>
				{/if}
			</form>
		</div>
		<div class="dialog-footer">
			<button class="btn-secondary" onclick={handleCancel} disabled={submitting || selecting}>Cancel</button>
			<button
				class="btn-primary"
				onclick={handleSubmit}
				disabled={!canSubmit}
			>
				{submitting ? 'Mounting...' : 'Mount'}
			</button>
		</div>
	</div>
</div>

<style>
	select {
		width: 100%;
		margin-bottom: 16px;
		padding: 10px;
		background: var(--input-bg);
		color: var(--text-primary);
		border: 1px solid var(--border-color);
		border-radius: 6px;
	}
	.overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.4);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
	}

	.dialog {
		width: 400px;
		background: var(--card-bg);
		border-radius: 12px;
		box-shadow: 0 10px 40px rgba(0, 0, 0, 0.2);
		overflow: hidden;
	}

	.dialog-header {
		padding: 16px 20px;
		border-bottom: 1px solid var(--border-color);
	}

	.dialog-header h3 {
		margin: 0;
		font-size: 16px;
		font-weight: 600;
		color: var(--text-primary);
	}

	.dialog-body {
		padding: 20px;
	}

	.device-info {
		margin: 0 0 16px;
		font-size: 14px;
		color: var(--text-secondary);
	}

	.passphrase-error {
		margin: 0 0 12px;
		padding: 8px 12px;
		font-size: 13px;
		color: var(--error-color);
		background: var(--error-bg);
		border: 1px solid var(--error-border);
		border-radius: 6px;
	}

	.device-info code {
		font-family: monospace;
		background: var(--badge-bg);
		padding: 2px 6px;
		border-radius: 4px;
		color: var(--text-primary);
	}

	label {
		display: block;
		font-size: 13px;
		font-weight: 500;
		color: var(--text-primary);
		margin-bottom: 6px;
	}

	.input-wrapper {
		display: flex;
		gap: 8px;
	}

	input {
		flex: 1;
		padding: 10px 12px;
		border: 1px solid var(--border-color);
		border-radius: 6px;
		font-size: 14px;
		background: var(--input-bg);
		color: var(--text-primary);
		outline: none;
	}

	input:focus {
		border-color: var(--accent-color);
		box-shadow: 0 0 0 3px var(--accent-shadow);
	}

	.toggle-visibility {
		padding: 8px 12px;
		border: 1px solid var(--border-color);
		border-radius: 6px;
		background: var(--button-secondary-bg);
		color: var(--text-secondary);
		font-size: 13px;
		cursor: pointer;
	}

	.toggle-visibility:hover {
		background: var(--button-secondary-hover);
	}

	.dialog-footer {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
		padding: 16px 20px;
		border-top: 1px solid var(--border-color);
		background: var(--neutral-bg);
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

	.btn-secondary:hover {
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

	.btn-primary:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
