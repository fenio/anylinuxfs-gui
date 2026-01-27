import { writable, derived } from 'svelte/store';
import type { MountInfo } from '../types';
import { getMountStatus } from '../api';

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

function createStatusStore() {
	const { subscribe, set, update } = writable<StatusState>({
		info: defaultInfo,
		loading: false,
		error: null
	});

	let pollInterval: ReturnType<typeof setInterval> | null = null;

	return {
		subscribe,
		async refresh() {
			update((s) => ({ ...s, loading: true }));
			try {
				const info = await getMountStatus();
				update((s) => ({ ...s, info, loading: false, error: null }));
			} catch (e) {
				update((s) => ({ ...s, error: String(e), loading: false }));
			}
		},
		startPolling(intervalMs: number = 2000) {
			this.stopPolling();
			this.refresh();
			pollInterval = setInterval(() => this.refresh(), intervalMs);
		},
		stopPolling() {
			if (pollInterval) {
				clearInterval(pollInterval);
				pollInterval = null;
			}
		}
	};
}

export const status = createStatusStore();

export const isMounted = derived(status, ($status) => $status.info.mounted);

export const mountPoint = derived(status, ($status) => $status.info.mount_point);
