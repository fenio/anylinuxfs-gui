<script lang="ts">
	import { onMount } from 'svelte';
	import {
		listCustomActions,
		createCustomAction,
		updateCustomAction,
		deleteCustomAction,
		type CustomAction
	} from '$lib/api';

	let actions = $state<CustomAction[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let showForm = $state(false);
	let editingAction = $state<CustomAction | null>(null);

	// Form fields
	let formName = $state('');
	let formDescription = $state('');
	let formBeforeMount = $state('');
	let formAfterMount = $state('');
	let formBeforeUnmount = $state('');
	let formEnvironment = $state('');
	let formCaptureEnvironment = $state('');
	let formOverrideNfsExport = $state('');
	let formRequiredOs = $state('');
	let formSubmitting = $state(false);
	let deleteConfirm = $state<string | null>(null);

	async function loadActions() {
		loading = true;
		error = null;
		try {
			actions = await listCustomActions();
		} catch (e) {
			error = String(e);
		}
		loading = false;
	}

	onMount(() => {
		loadActions();
	});

	function resetForm() {
		formName = '';
		formDescription = '';
		formBeforeMount = '';
		formAfterMount = '';
		formBeforeUnmount = '';
		formEnvironment = '';
		formCaptureEnvironment = '';
		formOverrideNfsExport = '';
		formRequiredOs = '';
	}

	function openCreateForm() {
		resetForm();
		editingAction = null;
		showForm = true;
	}

	function openEditForm(action: CustomAction) {
		formName = action.name;
		formDescription = action.description;
		formBeforeMount = action.before_mount;
		formAfterMount = action.after_mount;
		formBeforeUnmount = action.before_unmount;
		formEnvironment = action.environment.join('\n');
		formCaptureEnvironment = action.capture_environment.join('\n');
		formOverrideNfsExport = action.override_nfs_export;
		formRequiredOs = action.required_os;
		editingAction = action;
		showForm = true;
	}

	function openCopyForm(action: CustomAction) {
		formName = action.name + '-copy';
		formDescription = action.description;
		formBeforeMount = action.before_mount;
		formAfterMount = action.after_mount;
		formBeforeUnmount = action.before_unmount;
		formEnvironment = action.environment.join('\n');
		formCaptureEnvironment = action.capture_environment.join('\n');
		formOverrideNfsExport = action.override_nfs_export;
		formRequiredOs = action.required_os;
		editingAction = null; // null means create mode
		showForm = true;
	}

	function closeForm() {
		showForm = false;
		editingAction = null;
		resetForm();
	}

	async function handleSubmit() {
		if (!formName.trim()) {
			error = 'Action name is required';
			return;
		}

		formSubmitting = true;
		error = null;

		const actionData = {
			name: formName.trim(),
			description: formDescription.trim(),
			before_mount: formBeforeMount,
			after_mount: formAfterMount,
			before_unmount: formBeforeUnmount,
			environment: formEnvironment
				.split('\n')
				.map((s) => s.trim())
				.filter((s) => s),
			capture_environment: formCaptureEnvironment
				.split('\n')
				.map((s) => s.trim())
				.filter((s) => s),
			override_nfs_export: formOverrideNfsExport.trim(),
			required_os: formRequiredOs.trim()
		};

		try {
			if (editingAction) {
				await updateCustomAction(actionData);
			} else {
				await createCustomAction(actionData);
			}
			closeForm();
			await loadActions();
		} catch (e) {
			error = String(e);
		}

		formSubmitting = false;
	}

	function confirmDelete(name: string) {
		deleteConfirm = name;
	}

	function cancelDelete() {
		deleteConfirm = null;
	}

	async function handleDelete() {
		if (!deleteConfirm) return;

		const name = deleteConfirm;
		deleteConfirm = null;
		error = null;

		try {
			await deleteCustomAction(name);
			await loadActions();
		} catch (e) {
			error = String(e);
		}
	}
</script>

<svelte:head>
	<title>Actions - anylinuxfs</title>
</svelte:head>

<div class="actions-page">
	<div class="header">
		<h2>Custom Actions</h2>
		<button class="btn-primary" onclick={openCreateForm}>New Action</button>
	</div>

	{#if error}
		<div class="error-banner">
			<span>{error}</span>
			<button onclick={() => (error = null)}>Dismiss</button>
		</div>
	{/if}

	{#if loading && actions.length === 0}
		<div class="loading">Loading actions...</div>
	{:else if actions.length === 0}
		<div class="empty">
			<p>No custom actions defined.</p>
			<p class="hint">
				Custom actions let you run scripts before/after mount operations and customize the NFS
				export.
			</p>
		</div>
	{:else}
		<div class="actions-list">
			{#each actions as action}
				<div class="action-card" class:upstream={action.is_upstream}>
					<div class="action-info">
						<div class="action-header">
							<span class="action-name">{action.name}</span>
							{#if action.is_upstream}
								<span class="badge upstream-badge">Upstream (read-only)</span>
							{:else}
								<span class="badge user-badge">User</span>
							{/if}
						</div>
						{#if action.description}
							<p class="action-description">{action.description}</p>
						{/if}
						<div class="action-details">
							{#if action.before_mount}
								<span class="detail-tag">before_mount</span>
							{/if}
							{#if action.after_mount}
								<span class="detail-tag">after_mount</span>
							{/if}
							{#if action.before_unmount}
								<span class="detail-tag">before_unmount</span>
							{/if}
							{#if action.environment.length > 0}
								<span class="detail-tag">environment</span>
							{/if}
							{#if action.override_nfs_export}
								<span class="detail-tag">nfs_export</span>
							{/if}
							{#if action.required_os}
								<span class="detail-tag">os: {action.required_os}</span>
							{/if}
						</div>
					</div>
					<div class="action-actions">
						{#if action.is_upstream}
							<button class="btn-secondary" onclick={() => openCopyForm(action)}>Copy</button>
						{:else}
							<button class="btn-secondary" onclick={() => openEditForm(action)}>Edit</button>
							{#if deleteConfirm === action.name}
								<button class="btn-danger" onclick={handleDelete}>Confirm</button>
								<button class="btn-secondary" onclick={cancelDelete}>Cancel</button>
							{:else}
								<button class="btn-danger" onclick={() => confirmDelete(action.name)}>Delete</button>
							{/if}
						{/if}
					</div>
				</div>
			{/each}
		</div>
	{/if}

	<div class="info-section">
		<p>
			<strong>Custom actions</strong> allow you to run shell scripts at different stages of the mount/unmount
			process.
		</p>
		<p>
			<strong>Upstream actions</strong> are defined in <code>/opt/homebrew/etc/anylinuxfs.toml</code>
			and cannot be modified.
		</p>
		<p>
			<strong>User actions</strong> are stored in <code>~/.anylinuxfs/config.toml</code>.
		</p>
	</div>
</div>

{#if showForm}
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<div class="modal-overlay" onclick={closeForm} role="presentation">
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
			<div class="modal-header">
				<h3>{editingAction ? 'Edit Action' : 'New Action'}</h3>
				<button class="modal-close" onclick={closeForm}>&times;</button>
			</div>
			<form class="modal-body" onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
				<div class="form-group">
					<label for="name">Name <span class="required">*</span></label>
					<input
						type="text"
						id="name"
						bind:value={formName}
						disabled={!!editingAction}
						placeholder="my-action"
						required
						autocomplete="off"
						spellcheck="false"
					/>
					{#if editingAction}
						<span class="field-hint">Name cannot be changed after creation</span>
					{/if}
				</div>

				<div class="form-group">
					<label for="description">Description</label>
					<input
						type="text"
						id="description"
						bind:value={formDescription}
						placeholder="Optional description of what this action does"
						autocomplete="off"
						spellcheck="false"
					/>
				</div>

				<div class="form-group">
					<label for="before_mount">Before Mount Script</label>
					<textarea
						id="before_mount"
						bind:value={formBeforeMount}
						rows="3"
						placeholder="Commands to run before mounting"
						autocomplete="off"
						spellcheck="false"
					></textarea>
				</div>

				<div class="form-group">
					<label for="after_mount">After Mount Script</label>
					<textarea
						id="after_mount"
						bind:value={formAfterMount}
						rows="3"
						placeholder="Commands to run after mounting"
						autocomplete="off"
						spellcheck="false"
					></textarea>
				</div>

				<div class="form-group">
					<label for="before_unmount">Before Unmount Script</label>
					<textarea
						id="before_unmount"
						bind:value={formBeforeUnmount}
						rows="3"
						placeholder="Commands to run before unmounting"
						autocomplete="off"
						spellcheck="false"
					></textarea>
				</div>

				<div class="form-group">
					<label for="environment">Environment Variables</label>
					<textarea
						id="environment"
						bind:value={formEnvironment}
						rows="2"
						placeholder="KEY=VALUE (one per line)"
						autocomplete="off"
						spellcheck="false"
					></textarea>
					<span class="field-hint">Variables to pass to the mount scripts</span>
				</div>

				<div class="form-group">
					<label for="capture_environment">Capture Environment</label>
					<textarea
						id="capture_environment"
						bind:value={formCaptureEnvironment}
						rows="2"
						placeholder="VARIABLE_NAME (one per line)"
						autocomplete="off"
						spellcheck="false"
					></textarea>
					<span class="field-hint">Host environment variables to capture and pass through</span>
				</div>

				<div class="form-group">
					<label for="override_nfs_export">Override NFS Export</label>
					<input
						type="text"
						id="override_nfs_export"
						bind:value={formOverrideNfsExport}
						placeholder="Custom NFS export path"
						autocomplete="off"
						spellcheck="false"
					/>
				</div>

				<div class="form-group">
					<label for="required_os">Required OS</label>
					<select id="required_os" bind:value={formRequiredOs}>
						<option value="">Any</option>
						<option value="linux">Linux</option>
						<option value="freebsd">FreeBSD</option>
					</select>
					<span class="field-hint">Restrict this action to a specific VM image</span>
				</div>

				<div class="form-actions">
					<button type="button" class="btn-secondary" onclick={closeForm}>Cancel</button>
					<button type="submit" class="btn-primary" disabled={formSubmitting}>
						{formSubmitting ? 'Saving...' : editingAction ? 'Save Changes' : 'Create Action'}
					</button>
				</div>
			</form>
		</div>
	</div>
{/if}

<style>
	.actions-page {
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

	.empty p {
		margin: 0 0 8px;
	}

	.empty .hint {
		font-size: 13px;
	}

	.actions-list {
		flex: 1;
		overflow-y: auto;
	}

	.action-card {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		padding: 14px 16px;
		background: var(--card-bg);
		border: 1px solid var(--border-color);
		border-radius: 8px;
		margin-bottom: 10px;
	}

	.action-card.upstream {
		background: var(--neutral-bg);
	}

	.action-info {
		flex: 1;
		min-width: 0;
	}

	.action-header {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-bottom: 4px;
	}

	.action-name {
		font-size: 14px;
		font-weight: 600;
		color: var(--text-primary);
		font-family: monospace;
	}

	.badge {
		padding: 2px 8px;
		border-radius: 10px;
		font-size: 10px;
		font-weight: 500;
	}

	.upstream-badge {
		background: var(--disabled-bg);
		color: var(--disabled-text);
	}

	.user-badge {
		background: var(--accent-color);
		color: white;
	}

	.action-description {
		font-size: 13px;
		color: var(--text-secondary);
		margin: 4px 0 8px;
	}

	.action-details {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}

	.detail-tag {
		padding: 2px 6px;
		background: var(--hover-bg);
		border-radius: 4px;
		font-size: 11px;
		color: var(--text-secondary);
		font-family: monospace;
	}

	.action-actions {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-left: 16px;
	}

	.readonly-label {
		font-size: 12px;
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
	}

	.info-section code {
		background: var(--hover-bg);
		padding: 2px 6px;
		border-radius: 4px;
		font-size: 12px;
	}

	/* Modal styles */
	.modal-overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.5);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
	}

	.modal {
		background: var(--card-bg);
		border-radius: 12px;
		width: 500px;
		max-width: 90vw;
		max-height: 85vh;
		overflow: hidden;
		display: flex;
		flex-direction: column;
		box-shadow: 0 20px 50px rgba(0, 0, 0, 0.3);
	}

	.modal-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 16px 20px;
		border-bottom: 1px solid var(--border-color);
	}

	.modal-header h3 {
		margin: 0;
		font-size: 16px;
		font-weight: 600;
		color: var(--text-primary);
	}

	.modal-close {
		background: none;
		border: none;
		font-size: 24px;
		color: var(--text-secondary);
		cursor: pointer;
		padding: 0;
		line-height: 1;
	}

	.modal-close:hover {
		color: var(--text-primary);
	}

	.modal-body {
		padding: 20px;
		overflow-y: auto;
	}

	.form-group {
		margin-bottom: 16px;
	}

	.form-group label {
		display: block;
		font-size: 13px;
		font-weight: 500;
		color: var(--text-primary);
		margin-bottom: 6px;
	}

	.required {
		color: var(--danger-color);
		font-weight: 600;
	}

	.form-group input,
	.form-group textarea,
	.form-group select {
		width: 100%;
		padding: 8px 12px;
		border: 1px solid var(--border-color);
		border-radius: 6px;
		background: var(--input-bg, var(--card-bg));
		color: var(--text-primary);
		font-size: 13px;
		font-family: inherit;
	}

	.form-group textarea {
		resize: vertical;
		font-family: monospace;
	}

	.form-group input:focus,
	.form-group textarea:focus,
	.form-group select:focus {
		outline: none;
		border-color: var(--accent-color);
	}

	.form-group input:disabled {
		background: var(--neutral-bg);
		color: var(--text-secondary);
	}

	.field-hint {
		display: block;
		font-size: 11px;
		color: var(--text-secondary);
		margin-top: 4px;
	}

	.form-actions {
		display: flex;
		justify-content: flex-end;
		gap: 10px;
		margin-top: 20px;
		padding-top: 16px;
		border-top: 1px solid var(--border-color);
	}
</style>
