<script lang="ts">
	import type { BtrfsSubvolumeList, Partition } from '#lib/types';
	import { getMountStatus, listBtrfsSubvolumes, unmountDisk } from '#lib/api';
	import { parseError } from '#lib/errors';
	import { disks } from '#lib/stores/disks';
	import { elevation } from '#lib/stores/elevation';
	import { status } from '#lib/stores/status';
	import PassphraseDialog from './PassphraseDialog.svelte';

	interface Props {
		partition: Partition;
		options: string;
		disabled: boolean;
		mounted: boolean;
		onSelect: (option: string, readOnly: boolean) => void;
		onBusyChange: (busy: boolean) => void;
	}

	let { partition, options, disabled, mounted, onSelect, onBusyChange }: Props = $props();
	let data = $state<BtrfsSubvolumeList | null>(null);
	let busy = $state(false);
	let error = $state<string | null>(null);
	let showUnlock = $state(false);
	let submitting = $state(false);
	let unlockError = $state<string | null>(null);
	let selected = $derived(options.split(',').map((part) => part.trim()).find((part) => /^(subvol|subvolid)=/.test(part)) || '');
	let knownSelection = $derived(!selected || selected === 'subvolid=5' || !!data?.subvolumes.some((subvolume) => selected === `subvolid=${subvolume.id}`));

	function setBusy(value: boolean) {
		busy = value;
		onBusyChange(value);
	}

	function select(event: Event) {
		const option = (event.target as HTMLSelectElement).value;
		const id = option ? Number(option.slice('subvolid='.length)) : data?.default_id;
		onSelect(option, data?.subvolumes.find((subvolume) => subvolume.id === id)?.read_only || false);
	}

	async function readSubvolumes(temporary: boolean) {
		try {
			data = await listBtrfsSubvolumes(partition.device);
		} catch (e) {
			error = parseError(e).message;
		} finally {
			if (temporary) {
				try {
					await unmountDisk(partition.device);
				} catch (e) {
					error = `${error ? error + ' ' : ''}The temporary read-only mount could not be unmounted: ${parseError(e).message}. Unmount it before mounting your selection.`;
				}
			}
			status.refresh();
			setBusy(false);
		}
	}

	async function mountForDiscovery(passphrase?: string, keyFile?: string) {
		const result = await disks.mount(partition.device, passphrase, true, 'nologreplay,subvolid=5', false, keyFile);
		if (result === 'success') {
			showUnlock = false;
			await readSubvolumes(true);
		} else if (result === 'encryption_required') {
			unlockError = passphrase ? 'Incorrect passphrase. Please try again.' : null;
			showUnlock = true;
		} else if (result === 'error' && showUnlock) {
			unlockError = $disks.error || 'Unable to unlock the filesystem.';
		} else {
			showUnlock = false;
			error = result === 'error' ? $disks.error : 'Discovery cancelled.';
			setBusy(false);
		}
	}

	async function discover() {
		if (busy || disabled) return;
		error = null;
		unlockError = null;
		setBusy(true);
		try {
			// Check live status rather than a possibly stale polling snapshot. Never
			// unmount an existing user mount after reading its subvolume metadata.
			const mounts = await getMountStatus();
			if (mounts.some((mount) => mount.device === partition.device)) {
				await readSubvolumes(false);
			} else if (partition.encrypted) {
				showUnlock = true;
			} else {
				await mountForDiscovery();
			}
		} catch (e) {
			error = parseError(e).message;
			setBusy(false);
		}
	}

	async function unlock(passphrase?: string, keyFile?: string) {
		submitting = true;
		try {
			await mountForDiscovery(passphrase, keyFile);
		} catch (e) {
			unlockError = parseError(e).message;
		} finally {
			submitting = false;
		}
	}

	function cancelUnlock() {
		showUnlock = false;
		setBusy(false);
	}
</script>

<div class="btrfs-picker">
	<label>
		<span>Btrfs subvolume</span>
		<select value={selected} onchange={select} disabled={disabled || busy || mounted}>
			<option value="">Filesystem default{data ? ` (ID ${data.default_id})` : ''}</option>
			<option value="subvolid=5">Top-level filesystem (ID 5)</option>
			{#if !knownSelection}<option value={selected}>Custom: {selected}</option>{/if}
			{#each data?.subvolumes || [] as subvolume (subvolume.id)}
				<option value={`subvolid=${subvolume.id}`}>
					{subvolume.path} (ID {subvolume.id}){subvolume.snapshot ? ' — snapshot' : ''}{subvolume.read_only ? ' — read-only' : ''}
				</option>
			{/each}
		</select>
	</label>
	<button type="button" onclick={discover} disabled={disabled || busy}>
		{busy ? 'Discovering…' : data ? 'Refresh subvolumes' : 'Discover subvolumes'}
	</button>
	<p class="hint">{mounted ? 'Discovery leaves your current mount unchanged. Unmount before choosing another subvolume.' : 'Discovery temporarily mounts the top level read-only, without replaying the journal, then unmounts it. Encrypted drives require unlocking.'}</p>
	{#if data}<p class="hint">{data.subvolumes.length} subvolumes found. Read-only subvolumes are selected with RO enabled. Selections use IDs, including for paths with spaces.</p>{/if}
	{#if error}<p class="error" role="alert">{error}</p>{/if}
</div>

{#if showUnlock}
	<PassphraseDialog device={partition.device} description="This temporarily mounts the filesystem read-only to discover Btrfs subvolumes, then unmounts it." errorMessage={unlockError} submitting={submitting} onSubmit={unlock} onCancel={cancelUnlock} />
{/if}

<style>
	.btrfs-picker { padding: 12px 16px; border-top: 1px solid var(--border-color); }
	label { display: flex; flex-direction: column; gap: 6px; font-size: 13px; }
	select { width: 100%; padding: 8px; background: var(--input-bg); color: var(--text-primary); border: 1px solid var(--border-color); border-radius: 6px; }
	button { margin-top: 8px; padding: 6px 12px; background: var(--bg-secondary); color: var(--text-primary); border: 1px solid var(--border-color); border-radius: 6px; cursor: pointer; }
	button:disabled { opacity: 0.5; cursor: default; }
	.hint { font-size: 12px; color: var(--text-secondary); margin: 8px 0 0; }
	.error { font-size: 12px; color: var(--error-color, #ef4444); margin: 8px 0 0; }
</style>
