import { writable, derived, get } from 'svelte/store';
import type { Disk, DiskListResult } from '../types';
import { listDisks, mountDisk, unmountDisk } from '../api';
import { Timeouts, validateDevicePath } from '../constants';
import { logAction, logError, notifyIfHidden } from '../logger';
import { parseError } from '../errors';

interface DisksState {
	disks: Disk[];
	loading: boolean;
	error: string | null;
	mountingDevices: Set<string>;
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
		adminMode: false,
		hasSupportedPartitions: true,
		recentUnmount: false
	});

	let unmountTimeout: ReturnType<typeof setTimeout> | null = null;

	return {
		subscribe,
		async refresh(useSudo?: boolean, silent?: boolean) {
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
		},
		setAdminMode(enabled: boolean) {
			currentAdminMode = enabled;
			update((s) => ({ ...s, adminMode: enabled }));
		},
		async mount(device: string, passphrase?: string, readOnly?: boolean, extraOptions?: string): Promise<'success' | 'encryption_required' | 'error'> {
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
			update((s) => {
				const devices = new Set(s.mountingDevices);
				devices.add(device);
				return { ...s, mountingDevices: devices, error: null, recentUnmount: false };
			});
			try {
				await mountDisk(device, passphrase, readOnly, extraOptions);
				logAction('Mount completed', { device });
				update((s) => {
					const devices = new Set(s.mountingDevices);
					devices.delete(device);
					return { ...s, mountingDevices: devices };
				});
				notifyIfHidden('Mount Complete', `${device} mounted successfully.`);
				return 'success';
			} catch (e) {
				const rawError = String(e);

				// Detect encryption-required error from backend
				if (rawError.includes('ENCRYPTION_REQUIRED')) {
					logAction('Encryption detected, passphrase needed', { device });
					update((s) => {
						const devices = new Set(s.mountingDevices);
						devices.delete(device);
						return { ...s, mountingDevices: devices };
					});
					return 'encryption_required';
				}

				logError('mount', e);
				const errorMessage = parseError(e).message;
				notifyIfHidden('Mount Failed', errorMessage);
				update((s) => {
					const devices = new Set(s.mountingDevices);
					devices.delete(device);
					return { ...s, error: errorMessage, mountingDevices: devices };
				});
				return 'error';
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
