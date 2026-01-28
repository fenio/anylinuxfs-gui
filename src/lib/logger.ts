import { trace, debug, info, warn, error } from '@tauri-apps/plugin-log';

// Re-export log functions for convenience
export { trace, debug, info, warn, error };

// Helper to log errors with context
export function logError(context: string, err: unknown): void {
	const message = err instanceof Error ? err.message : String(err);
	error(`[${context}] ${message}`);
}

// Helper to log actions
export function logAction(action: string, details?: Record<string, unknown>): void {
	if (details) {
		info(`${action}: ${JSON.stringify(details)}`);
	} else {
		info(action);
	}
}
