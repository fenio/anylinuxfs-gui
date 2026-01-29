import { writable } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import { getLogContent, startLogStream } from '../api';
import { Events, Limits } from '../constants';
import { logError } from '../logger';

// Pre-computed log line with flags
export interface LogLine {
	text: string;
	isError: boolean;
	isWarn: boolean;
}

interface LogsState {
	lines: LogLine[];
	loading: boolean;
	error: string | null;
	following: boolean;
}

// Pre-compute error/warn flags for a line
function processLine(text: string): LogLine {
	return {
		text,
		isError: text.includes('ERROR'),
		isWarn: text.includes('WARN')
	};
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
				const rawLines = await getLogContent(Limits.DEFAULT_LOG_LINES);
				const lines = rawLines.map(processLine);
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

				// Listen for batched log events (more efficient than per-line)
				unlisten = await listen<string[]>(Events.LOG_LINES, (event) => {
					update((s) => {
						const newLines = [...s.lines];
						for (const line of event.payload) {
							newLines.push(processLine(line));
						}
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
