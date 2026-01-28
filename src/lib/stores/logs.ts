import { writable } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import { getLogContent, startLogStream } from '../api';
import { Events, Limits } from '../constants';
import { logError } from '../logger';

interface LogsState {
	lines: string[];
	loading: boolean;
	error: string | null;
	following: boolean;
}

function createLogsStore() {
	const { subscribe, set, update } = writable<LogsState>({
		lines: [],
		loading: false,
		error: null,
		following: true
	});

	let unlisten: (() => void) | null = null;

	return {
		subscribe,
		async load() {
			update((s) => ({ ...s, loading: true, error: null }));
			try {
				const lines = await getLogContent(Limits.DEFAULT_LOG_LINES);
				update((s) => ({ ...s, lines, loading: false }));
			} catch (e) {
				logError('logs.load', e);
				update((s) => ({ ...s, error: String(e), loading: false }));
			}
		},
		async startStreaming() {
			try {
				// Start the backend log watcher
				await startLogStream();

				// Listen for log events
				unlisten = await listen<string>(Events.LOG_LINE, (event) => {
					update((s) => {
						const newLines = [...s.lines, event.payload];
						// Keep only last MAX_LINES
						if (newLines.length > Limits.MAX_LOG_LINES) {
							newLines.splice(0, newLines.length - Limits.MAX_LOG_LINES);
						}
						return { ...s, lines: newLines };
					});
				});
			} catch (e) {
				logError('logs.startStreaming', e);
				update((s) => ({ ...s, error: String(e) }));
			}
		},
		stopStreaming() {
			if (unlisten) {
				unlisten();
				unlisten = null;
			}
		},
		setFollowing(following: boolean) {
			update((s) => ({ ...s, following }));
		},
		clear() {
			update((s) => ({ ...s, lines: [] }));
		}
	};
}

export const logs = createLogsStore();
