<script lang="ts">
	import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
	import { onMount } from 'svelte';

	let autoLaunch = $state(false);
	let autoLaunchLoading = $state(false);

	onMount(async () => {
		autoLaunch = await isEnabled();
	});

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

</style>
