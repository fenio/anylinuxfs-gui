import { writable, derived } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import type { MountInfo } from '../types';
import { getMountStatus } from '../api';
import { Events, Timeouts } from '../constants';
import { logError, logAction } from '../logger';

interface StatusState {
	info: MountInfo;
	loading: boolean;
	error: string | null;
}

const defaultInfo: MountInfo = {
	mounted: false,
	device: null,
	mount_point: null,
	filesystem: null,
	vm_running: false,
	ram_mb: null,
	vcpus: null,
	orphaned_instance: false
};

// Longer polling interval since we now have push events
const FALLBACK_POLL_INTERVAL = 10000; // 10 seconds

function createStatusStore() {
	const { subscribe, set, update } = writable<StatusState>({
		info: defaultInfo,
		loading: false,
		error: null
	});

	let pollInterval: ReturnType<typeof setInterval> | null = null;
	let unlisten: (() => void) | null = null;

	return {
		subscribe,
		async refresh() {
			update((s) => ({ ...s, loading: true }));
			try {
				const info = await getMountStatus();
				update((s) => ({ ...s, info, loading: false, error: null }));
			} catch (e) {
				logError('status.refresh', e);
				update((s) => ({ ...s, error: String(e), loading: false }));
			}
		},
		async startListening() {
			// Stop any existing listeners
			this.stopListening();

			// Initial refresh
			this.refresh();

			// Listen for status change events (push updates)
			try {
				unlisten = await listen(Events.STATUS_CHANGED, () => {
					logAction('Status changed event received');
					this.refresh();
				});
			} catch (e) {
				logError('status.startListening', e);
			}

			// Fallback polling at longer interval for orphan detection
			pollInterval = setInterval(() => this.refresh(), FALLBACK_POLL_INTERVAL);
		},
		stopListening() {
			if (unlisten) {
				unlisten();
				unlisten = null;
			}
			if (pollInterval) {
				clearInterval(pollInterval);
				pollInterval = null;
			}
		},
		// Legacy methods for compatibility
		startPolling(intervalMs: number = Timeouts.STATUS_POLL_INTERVAL) {
			this.startListening();
		},
		stopPolling() {
			this.stopListening();
		}
	};
}

export const status = createStatusStore();

export const isMounted = derived(status, ($status) => $status.info.mounted);

export const mountPoint = derived(status, ($status) => $status.info.mount_point);
