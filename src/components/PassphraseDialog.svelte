<script lang="ts">
	interface Props {
		device: string;
		onSubmit: (passphrase: string) => void;
		onCancel: () => void;
	}

	let { device, onSubmit, onCancel }: Props = $props();

	let passphrase = $state('');
	let showPassphrase = $state(false);
	let inputEl: HTMLInputElement | undefined = $state();

	$effect(() => {
		if (inputEl) {
			inputEl.focus();
		}
	});

	function handleSubmit(e: Event) {
		e.preventDefault();
		if (passphrase.trim()) {
			onSubmit(passphrase);
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			onCancel();
		}
	}
</script>

<div class="overlay" role="dialog" aria-modal="true" tabindex="-1" onkeydown={handleKeydown}>
	<div class="dialog">
		<div class="dialog-header">
			<h3>Enter Passphrase</h3>
		</div>
		<div class="dialog-body">
			<p class="device-info">
				The partition <code>{device}</code> is encrypted.
			</p>
			<form onsubmit={handleSubmit}>
				<label for="passphrase">Passphrase</label>
				<div class="input-wrapper">
					<input
						bind:this={inputEl}
						id="passphrase"
						type={showPassphrase ? 'text' : 'password'}
						bind:value={passphrase}
						placeholder="Enter encryption passphrase"
						autocomplete="off"
						autocorrect="off"
						spellcheck="false"
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
			</form>
		</div>
		<div class="dialog-footer">
			<button class="btn-secondary" onclick={onCancel}>Cancel</button>
			<button
				class="btn-primary"
				onclick={handleSubmit}
				disabled={!passphrase.trim()}
			>
				Mount
			</button>
		</div>
	</div>
</div>

<style>
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
