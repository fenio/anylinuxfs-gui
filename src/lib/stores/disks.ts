import { writable, derived } from 'svelte/store';
import type { Disk, DiskListResult } from '../types';
import { listDisks, mountDisk, unmountDisk } from '../api';
import { Timeouts, validateDevicePath } from '../constants';
import { logAction, logError } from '../logger';

interface DisksState {
	disks: Disk[];
	loading: boolean;
	error: string | null;
	mountingDevice: string | null;
	adminMode: boolean;
	hasSupportedPartitions: boolean;
	recentUnmount: boolean;
	currentMountId: number;
}

function createDisksStore() {
	const { subscribe, set, update } = writable<DisksState>({
		disks: [],
		loading: false,
		error: null,
		mountingDevice: null,
		adminMode: false,
		hasSupportedPartitions: true,
		recentUnmount: false,
		currentMountId: 0
	});

	let unmountTimeout: ReturnType<typeof setTimeout> | null = null;

	return {
		subscribe,
		async refresh(useSudo?: boolean) {
			update((s) => ({ ...s, loading: true, error: null }));
			try {
				// Use provided value or fall back to current adminMode
				let adminMode: boolean;
				const unsubscribe = subscribe((s) => (adminMode = useSudo ?? s.adminMode));
				unsubscribe();
				const result = await listDisks(adminMode!);
				update((s) => ({
					...s,
					disks: result.disks,
					hasSupportedPartitions: result.has_supported_partitions,
					loading: false
				}));
			} catch (e) {
				update((s) => ({ ...s, error: String(e), loading: false }));
			}
		},
		setAdminMode(enabled: boolean) {
			update((s) => ({ ...s, adminMode: enabled }));
		},
		async mount(device: string, passphrase?: string) {
			// Validate device path
			const validationError = validateDevicePath(device);
			if (validationError) {
				logError('mount', new Error(validationError));
				update((s) => ({ ...s, error: validationError }));
				return false;
			}

			const mountId = Date.now();
			logAction('Mount started', { device });
			update((s) => ({ ...s, mountingDevice: device, error: null, recentUnmount: false, currentMountId: mountId }));
			try {
				await mountDisk(device, passphrase);
				logAction('Mount completed', { device });
				update((s) => ({ ...s, mountingDevice: null }));
				return true;
			} catch (e) {
				logError('mount', e);
				// Don't show error if:
				// - Unmount was requested while mounting
				// - Mount was already detected as successful (mountingDevice was cleared)
				// - A different mount operation started
				update((s) => ({
					...s,
					error: (s.recentUnmount || s.mountingDevice === null || s.currentMountId !== mountId) ? null : String(e),
					mountingDevice: s.currentMountId === mountId ? null : s.mountingDevice
				}));
				return false;
			}
		},
		async unmount() {
			// Set recentUnmount to suppress stale mount errors and orphan warnings
			if (unmountTimeout) clearTimeout(unmountTimeout);
			logAction('Unmount started');
			update((s) => ({ ...s, mountingDevice: 'unmounting', error: null, recentUnmount: true }));
			try {
				await unmountDisk();
				// Small delay to let socket file clean up
				await new Promise((r) => setTimeout(r, Timeouts.UNMOUNT_CLEANUP_DELAY));
				logAction('Unmount completed');
				update((s) => ({ ...s, mountingDevice: null }));
				// Clear recentUnmount after timeout
				unmountTimeout = setTimeout(() => {
					update((s) => ({ ...s, recentUnmount: false }));
				}, Timeouts.RECENT_UNMOUNT_CLEAR);
				return true;
			} catch (e) {
				logError('unmount', e);
				update((s) => ({ ...s, error: String(e), mountingDevice: null }));
				unmountTimeout = setTimeout(() => {
					update((s) => ({ ...s, recentUnmount: false }));
				}, Timeouts.RECENT_UNMOUNT_CLEAR);
				return false;
			}
		},
		clearError() {
			update((s) => ({ ...s, error: null }));
		},
		clearMounting() {
			update((s) => ({ ...s, mountingDevice: null }));
		}
	};
}

export const disks = createDisksStore();

export const diskCount = derived(disks, ($disks) => $disks.disks.length);

export const partitionCount = derived(disks, ($disks) =>
	$disks.disks.reduce((acc, disk) => acc + disk.partitions.length, 0)
);
