import { writable, derived, get } from 'svelte/store';
import type { Disk, DiskListResult } from '../types';
import { cancelElevationOperation, listDisks, mountDisk, unmountDisk } from '../api';
import { Timeouts, validateDevicePath } from '../constants';
import { logAction, logError, notifyIfHidden } from '../logger';
import { parseError } from '../errors';
import { elevation } from './elevation';

interface DisksState {
	disks: Disk[];
	loading: boolean;
	error: string | null;
	mountingDevices: Set<string>;
	cancellableMounts: Set<string>;
	mountingMessages: Map<string, string>;
	adminMode: boolean;
	hasSupportedPartitions: boolean;
	recentUnmount: boolean;
}

function createDisksStore() {
	// Track adminMode locally to avoid subscribe/unsubscribe overhead
	let currentAdminMode = false;

	const { subscribe, set, update } = writable<DisksState>({
		disks: [],
		loading: false,
		error: null,
		mountingDevices: new Set(),
		cancellableMounts: new Set(),
		mountingMessages: new Map(),
		adminMode: false,
		hasSupportedPartitions: true,
		recentUnmount: false
	});

	let unmountTimeout: ReturnType<typeof setTimeout> | null = null;
	let refreshPromise: Promise<void> | null = null;

	return {
		subscribe,
		async refresh(useSudo?: boolean, silent?: boolean) {
			// Disk-watcher and user refreshes share one backend operation. Coalesce
			// overlapping requests so a silent refresh cannot clear the loading state
			// of a visible Terminal request or launch a duplicate Terminal session.
			if (refreshPromise) return refreshPromise;

			refreshPromise = (async () => {
				update((s) => ({ ...s, loading: true, error: null }));
				try {
					const adminMode = useSudo ?? currentAdminMode;
					const result = await listDisks(adminMode, silent ?? false);
					update((s) => ({
						...s,
						disks: result.disks,
						hasSupportedPartitions: result.has_supported_partitions,
						loading: false
					}));
				} catch (e) {
					const rawError = String(e);
					if (silent && rawError.includes('ALFS_SILENT_AUTH_EXPIRED')) {
						if (get(elevation).policy.mode === 'interactive_terminal') {
							// Interactive elevation requires visible interaction. Keep the
							// current list until the user explicitly clicks Refresh.
							update((s) => ({ ...s, loading: false }));
							return;
						}
						// Credentials expired during auto-refresh — disable admin mode quietly
						currentAdminMode = false;
						update((s) => ({
							...s,
							adminMode: false,
							loading: false,
							error: 'Admin credentials expired. Re-enable Admin mode to authenticate again.'
						}));
						return;
					}
					update((s) => ({ ...s, error: parseError(e).message, loading: false }));
				}
			})();

			try {
				await refreshPromise;
			} finally {
				refreshPromise = null;
			}
		},
		setAdminMode(enabled: boolean) {
			currentAdminMode = enabled;
			update((s) => ({ ...s, adminMode: enabled }));
		},
		async mount(device: string, passphrase?: string, readOnly?: boolean, extraOptions?: string, ignorePermissions?: boolean): Promise<'success' | 'encryption_required' | 'cancelled' | 'error'> {
			// Reject if this specific device is already being mounted
			const current = get({ subscribe });
			if (current.mountingDevices.has(device)) return 'error';

			// Validate device path
			const validationError = validateDevicePath(device);
			if (validationError) {
				logError('mount', new Error(validationError));
				update((s) => ({ ...s, error: validationError }));
				return 'error';
			}

			logAction('Mount started', { device });
			const elevationMode = get(elevation).policy.mode;
			update((s) => {
				const devices = new Set(s.mountingDevices);
				const cancellableMounts = new Set(s.cancellableMounts);
				const messages = new Map(s.mountingMessages);
				devices.add(device);
				if (elevationMode === 'interactive_terminal') cancellableMounts.add(device);
				messages.set(
					device,
					elevationMode === 'interactive_terminal'
						? 'Waiting for administrator approval and disk passphrase in Terminal…'
						: 'Mounting…'
				);
				return {
					...s,
					mountingDevices: devices,
					cancellableMounts,
					mountingMessages: messages,
					error: null,
					recentUnmount: false
				};
			});
			try {
				const result = await mountDisk(device, passphrase, readOnly, extraOptions, ignorePermissions);
				if (result.outcome === 'mounted') {
					logAction('Mount completed', { device });
					notifyIfHidden('Mount Complete', `${device} mounted successfully.`);
					return 'success';
				}
				if (result.outcome === 'encryption_required') {
					logAction('Encryption detected, passphrase needed', { device });
					return 'encryption_required';
				}
				if (result.outcome === 'cancelled') {
					logAction('Mount cancelled', { device });
					return 'cancelled';
				}

				const errorMessage = result.message || 'Mount failed';
				logError('mount', new Error(errorMessage));
				notifyIfHidden('Mount Failed', errorMessage);
				update((s) => ({ ...s, error: errorMessage }));
				return 'error';
			} catch (e) {
				logError('mount', e);
				const errorMessage = parseError(e).message;
				notifyIfHidden('Mount Failed', errorMessage);
				update((s) => ({ ...s, error: errorMessage }));
				return 'error';
			} finally {
				update((s) => {
					const devices = new Set(s.mountingDevices);
					const cancellableMounts = new Set(s.cancellableMounts);
					const messages = new Map(s.mountingMessages);
					devices.delete(device);
					cancellableMounts.delete(device);
					messages.delete(device);
					return { ...s, mountingDevices: devices, cancellableMounts, mountingMessages: messages };
				});
			}
		},
		async cancelMount(device: string): Promise<void> {
			update((s) => {
				if (!s.mountingDevices.has(device)) return s;
				const messages = new Map(s.mountingMessages);
				messages.set(device, 'Cancelling mount and requesting cleanup…');
				return { ...s, mountingMessages: messages };
			});
			try {
				await cancelElevationOperation(device);
			} catch (error) {
				const errorMessage = parseError(error).message;
				logError('cancelMount', error);
				update((s) => ({ ...s, error: errorMessage }));
			}
		},
		async unmount(device?: string) {
			logAction('Unmount started', { device: device || 'all' });
			// Set recentUnmount to suppress stale mount errors
			if (unmountTimeout) clearTimeout(unmountTimeout);
			update((s) => ({ ...s, error: null, recentUnmount: true }));
			try {
				await unmountDisk(device);
				// Small delay to let socket file clean up
				await new Promise((r) => setTimeout(r, Timeouts.UNMOUNT_CLEANUP_DELAY));
				logAction('Unmount completed', { device: device || 'all' });
				notifyIfHidden('Unmount Complete', 'Filesystem unmounted successfully.');
				// Clear recentUnmount after timeout
				unmountTimeout = setTimeout(() => {
					update((s) => ({ ...s, recentUnmount: false }));
				}, Timeouts.RECENT_UNMOUNT_CLEAR);
				return true;
			} catch (e) {
				logError('unmount', e);
				const errorMessage = parseError(e).message;
				notifyIfHidden('Unmount Failed', errorMessage);
				update((s) => ({ ...s, error: errorMessage }));
				unmountTimeout = setTimeout(() => {
					update((s) => ({ ...s, recentUnmount: false }));
				}, Timeouts.RECENT_UNMOUNT_CLEAR);
				return false;
			}
		},
		clearError() {
			update((s) => ({ ...s, error: null }));
		},
		cleanup() {
			if (unmountTimeout) {
				clearTimeout(unmountTimeout);
				unmountTimeout = null;
			}
		}
	};
}

export const disks = createDisksStore();

export const diskCount = derived(disks, ($disks) => $disks.disks.length);

export const partitionCount = derived(disks, ($disks) =>
	$disks.disks.reduce((acc, disk) => acc + disk.partitions.length, 0)
);
