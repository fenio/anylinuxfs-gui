import { writable, derived } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import type { MountInfo } from '../types';
import { getMountStatus } from '../api';
import { Events } from '../constants';
import { logError, logAction } from '../logger';
import { parseError } from '../errors';

interface StatusState {
	mounts: MountInfo[];
	loading: boolean;
	error: string | null;
}

// Longer polling interval since we now have push events
const FALLBACK_POLL_INTERVAL = 10000; // 10 seconds

// Debounce window to merge rapid refresh calls
const DEBOUNCE_MS = 500;

function createStatusStore() {
	const { subscribe, set, update } = writable<StatusState>({
		mounts: [],
		loading: false,
		error: null
	});

	let pollInterval: ReturnType<typeof setInterval> | null = null;
	let unlisten: (() => void) | null = null;
	let debounceTimeout: ReturnType<typeof setTimeout> | null = null;
	let refreshInProgress = false;

	async function doRefresh() {
		if (refreshInProgress) return;
		refreshInProgress = true;
		update((s) => ({ ...s, loading: true }));
		try {
			const mounts = await getMountStatus();
			update((s) => ({ ...s, mounts, loading: false, error: null }));
		} catch (e) {
			logError('status.refresh', e);
			update((s) => ({ ...s, error: parseError(e).message, loading: false }));
		} finally {
			refreshInProgress = false;
		}
	}

	return {
		subscribe,
		refresh() {
			// Debounce: cancel pending refresh and schedule new one
			if (debounceTimeout) {
				clearTimeout(debounceTimeout);
			}
			debounceTimeout = setTimeout(() => {
				debounceTimeout = null;
				doRefresh();
			}, DEBOUNCE_MS);
		},
		// Immediate refresh bypassing debounce (for initial load)
		refreshNow() {
			if (debounceTimeout) {
				clearTimeout(debounceTimeout);
				debounceTimeout = null;
			}
			doRefresh();
		},
		async startListening() {
			// Stop any existing listeners
			this.stopListening();

			// Initial refresh (immediate, no debounce)
			this.refreshNow();

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
			if (debounceTimeout) {
				clearTimeout(debounceTimeout);
				debounceTimeout = null;
			}
			if (unlisten) {
				unlisten();
				unlisten = null;
			}
			if (pollInterval) {
				clearInterval(pollInterval);
				pollInterval = null;
			}
		}
	};
}

export const status = createStatusStore();

export const isMounted = derived(status, ($status) => $status.mounts.length > 0);

export const mountedDevices = derived(status, ($status) =>
	new Set($status.mounts.map((m) => m.device))
);
