import { writable } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import { getLogContent, startLogStream, listLogFiles, type LogFileInfo } from '../api';
import { Events, Limits } from '../constants';
import { logError } from '../logger';
import { parseError } from '../errors';

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
	logFiles: LogFileInfo[];
	selectedFile: string | null; // null = all files
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
		following: true,
		logFiles: [],
		selectedFile: null
	});

	let unlisten: (() => void) | null = null;

	return {
		subscribe,
		async loadFiles() {
			try {
				const files = await listLogFiles();
				update((s) => ({ ...s, logFiles: files }));
			} catch (e) {
				logError('logs.loadFiles', e);
			}
		},
		async load() {
			update((s) => ({ ...s, loading: true, error: null }));
			try {
				let selectedFile: string | null = null;
				update((s) => { selectedFile = s.selectedFile; return s; });
				const rawLines = await getLogContent(Limits.DEFAULT_LOG_LINES, selectedFile || undefined);
				const lines = rawLines.map(processLine);
				update((s) => ({ ...s, lines, loading: false }));
			} catch (e) {
				logError('logs.load', e);
				update((s) => ({ ...s, error: parseError(e).message, loading: false }));
			}
		},
		selectFile(filePath: string | null) {
			update((s) => ({ ...s, selectedFile: filePath, lines: [] }));
			this.load();
		},
		async startStreaming() {
			try {
				// Start the backend log watcher
				await startLogStream();

				// Listen for batched log events (more efficient than per-line)
				unlisten = await listen<string[]>(Events.LOG_LINES, (event) => {
					update((s) => {
						// Only append streaming lines when viewing all files
						if (s.selectedFile !== null) return s;
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
				update((s) => ({ ...s, error: parseError(e).message }));
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
