<script lang="ts">
	import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
	import { onMount } from 'svelte';
	import { elevation } from '$lib/stores/elevation';
	import { disks } from '$lib/stores/disks';
	import type { ElevationMode } from '$lib/types';

	let autoLaunch = $state(false);
	let autoLaunchLoading = $state(false);

	onMount(async () => {
		autoLaunch = await isEnabled();
	});

	async function changeElevationMode(e: Event) {
		const mode = (e.target as HTMLSelectElement).value as ElevationMode;
		await elevation.setMode(mode);
	}

	async function toggleAutoLaunch() {
		autoLaunchLoading = true;
		try {
			if (autoLaunch) {
				await disable();
			} else {
				await enable();
			}
			autoLaunch = await isEnabled();
		} catch {
			// Revert on failure
			autoLaunch = !autoLaunch;
		}
		autoLaunchLoading = false;
	}
</script>

<div class="preferences-panel">
	<div class="header">
		<h2>Preferences</h2>
	</div>

	<div class="setting-group">
		<h3>Administrator authentication</h3>
		<p class="description">Choose how privileged disk commands request approval.</p>

		<div class="setting">
			<label for="elevation-mode">Elevation method</label>
			<select
				id="elevation-mode"
				value={$elevation.policy.mode}
				onchange={changeElevationMode}
				disabled={$elevation.loading || $elevation.saving || $disks.loading || $disks.mountingDevices.size > 0}
			>
				<option value="native">Native sudo (password or Touch ID)</option>
				<option value="interactive_terminal">Interactive Terminal (managed Macs)</option>
			</select>
			{#if $elevation.error}
				<span class="hint">{$elevation.error}</span>
			{/if}
			{#if $elevation.policy.mode === 'interactive_terminal'}
				<p class="guide-text">
					Admin scans and mounts open a Terminal tab so interactive privilege-management software can show its approval workflow.
					For encrypted disks, enter the LUKS passphrase in Terminal when <code>anylinuxfs</code> asks for it.
				</p>
				<span class="hint">Automatic disk changes do not open Terminal; click Refresh while Admin mode is enabled.</span>
			{:else}
				<span class="hint">Best for native macOS administrator accounts.</span>
			{/if}
		</div>
	</div>

	<div class="setting-group">
		<h3>Startup</h3>
		<p class="description">Control how anylinuxfs launches.</p>

		<div class="setting">
			<label class="toggle-row">
				<input
					type="checkbox"
					checked={autoLaunch}
					onchange={toggleAutoLaunch}
					disabled={autoLaunchLoading}
				/>
				<span>Launch at login</span>
			</label>
			<span class="hint">Automatically start anylinuxfs when you log in.</span>
		</div>
	</div>

	<div class="setting-group">
		<h3>Touch ID</h3>
		<p class="description">Use Touch ID instead of typing your password for admin actions.</p>

		<div class="setting">
			<p class="guide-text">
				macOS can use Touch ID to authenticate <code>sudo</code> commands.
				Once enabled, anylinuxfs will automatically use Touch ID when mounting disks.
			</p>
			<p class="guide-text">To enable, run these commands in Terminal:</p>
			<div class="code-block">
				<code>sudo cp /etc/pam.d/sudo_local.template /etc/pam.d/sudo_local</code>
				<code>sudo nano /etc/pam.d/sudo_local</code>
			</div>
			<p class="guide-text">
				Then uncomment the line containing <code>pam_tid.so</code> (remove the <code>#</code>)
				and save with <kbd>Ctrl+O</kbd>, <kbd>Enter</kbd>, <kbd>Ctrl+X</kbd>.
			</p>
			<span class="hint">This setting persists across macOS updates (Sonoma and later).</span>
		</div>
	</div>
</div>

<style>
	.preferences-panel {
		height: 100%;
		display: flex;
		flex-direction: column;
	}

	.toggle-row {
		display: flex;
		align-items: center;
		gap: 8px;
		cursor: pointer;
		margin-bottom: 2px;
	}

	.toggle-row input {
		cursor: pointer;
	}

	.toggle-row input:disabled {
		cursor: not-allowed;
	}

	.guide-text {
		margin: 4px 0;
		line-height: 1.5;
	}

	.guide-text code, .guide-text kbd {
		background: var(--input-bg);
		padding: 1px 5px;
		border-radius: 3px;
		font-size: 0.9em;
	}

	.code-block {
		background: var(--input-bg);
		border-radius: 6px;
		padding: 10px 12px;
		margin: 8px 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.code-block code {
		font-size: 0.85em;
		white-space: pre-wrap;
		word-break: break-all;
	}

</style>
