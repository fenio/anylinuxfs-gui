<script lang="ts">
	import { onMount } from 'svelte';
	import { listImages, installImage, uninstallImage, type VmImage } from '$lib/api';
	import { wrapAsync, parseError } from '$lib/errors';

	let images = $state<VmImage[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let processingImage = $state<string | null>(null);

	async function loadImages() {
		loading = true;
		error = null;
		const result = await wrapAsync(() => listImages());
		if (result.ok) {
			images = result.data;
		} else {
			error = result.error.message;
		}
		loading = false;
	}

	onMount(() => {
		loadImages();
	});

	async function handleInstall(name: string) {
		processingImage = name;
		error = null;
		try {
			await installImage(name);
			await loadImages();
		} catch (e) {
			error = parseError(e).message;
		}
		processingImage = null;
	}

	async function handleUninstall(name: string) {
		processingImage = name;
		error = null;
		try {
			await uninstallImage(name);
			await loadImages();
		} catch (e) {
			error = parseError(e).message;
		}
		processingImage = null;
	}
</script>

<svelte:head>
	<title>Images - anylinuxfs</title>
</svelte:head>

<div class="images-page">
	<div class="header">
		<h2>VM Images</h2>
		<button class="btn-secondary" onclick={loadImages} disabled={loading}>
			{loading ? 'Loading...' : 'Refresh'}
		</button>
	</div>

	{#if error}
		<div class="error-banner" role="alert">
			<span>{error}</span>
			<button onclick={() => (error = null)}>Dismiss</button>
		</div>
	{/if}

	{#if loading && images.length === 0}
		<div class="loading">Loading images...</div>
	{:else if images.length === 0}
		<div class="empty">No images available.</div>
	{:else}
		<div class="images-list">
			{#each images as image}
				<div class="image-card" class:installed={image.installed}>
					<div class="image-info">
						<span class="image-name">{image.name}</span>
						{#if image.installed}
							<span class="status-badge">Installed</span>
						{/if}
					</div>
					<div class="image-actions">
						{#if processingImage === image.name}
							<span class="processing">Processing...</span>
						{:else if image.installed}
							<button
								class="btn-danger"
								onclick={() => handleUninstall(image.name)}
								disabled={processingImage !== null}
							>
								Uninstall
							</button>
						{:else}
							<button
								class="btn-primary"
								onclick={() => handleInstall(image.name)}
								disabled={processingImage !== null}
							>
								Install
							</button>
						{/if}
					</div>
				</div>
			{/each}
		</div>
	{/if}

	<div class="info-section">
		<p>VM images are used by anylinuxfs to mount different filesystem types.</p>
		<p><strong>alpine-latest</strong> - Default Linux image for ext2/3/4, btrfs, XFS, etc.</p>
		<p><strong>freebsd-*</strong> - FreeBSD image required for ZFS support.</p>
	</div>
</div>

<style>
	.images-page {
		height: 100%;
		display: flex;
		flex-direction: column;
	}

	.header {
		display: flex;
		align-items: center;
		justify-content: space-between;
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

	.loading,
	.empty {
		padding: 24px;
		text-align: center;
		color: var(--text-secondary);
	}

	.images-list {
		flex: 1;
		overflow-y: auto;
	}

	.image-card {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 14px 16px;
		background: var(--card-bg);
		border: 1px solid var(--border-color);
		border-radius: 8px;
		margin-bottom: 10px;
	}

	.image-card.installed {
		border-color: var(--success-border);
		background: var(--success-bg);
	}

	.image-info {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.image-name {
		font-size: 14px;
		font-weight: 500;
		color: var(--text-primary);
		font-family: monospace;
	}

	.status-badge {
		padding: 2px 8px;
		background: var(--success-color);
		color: white;
		border-radius: 10px;
		font-size: 11px;
		font-weight: 500;
	}

	.image-actions {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.processing {
		font-size: 13px;
		color: var(--text-secondary);
		font-style: italic;
	}

	.btn-primary,
	.btn-secondary,
	.btn-danger {
		padding: 6px 14px;
		border-radius: 6px;
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
	}

	.btn-primary {
		border: none;
		background: var(--accent-color);
		color: white;
	}

	.btn-primary:hover:not(:disabled) {
		background: var(--accent-hover);
	}

	.btn-secondary {
		border: 1px solid var(--border-color);
		background: var(--button-secondary-bg);
		color: var(--text-primary);
	}

	.btn-secondary:hover:not(:disabled) {
		background: var(--button-secondary-hover);
	}

	.btn-danger {
		border: none;
		background: var(--danger-color);
		color: white;
	}

	.btn-danger:hover:not(:disabled) {
		background: var(--danger-hover);
	}

	.btn-primary:disabled,
	.btn-secondary:disabled,
	.btn-danger:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.info-section {
		margin-top: 20px;
		padding: 16px;
		background: var(--neutral-bg);
		border: 1px solid var(--border-color);
		border-radius: 8px;
	}

	.info-section p {
		margin: 0 0 8px;
		font-size: 13px;
		color: var(--text-secondary);
	}

	.info-section p:last-child {
		margin-bottom: 0;
	}

	.info-section strong {
		color: var(--text-primary);
		font-family: monospace;
	}
</style>
