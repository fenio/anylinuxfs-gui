import { trace, debug, info, warn, error } from '@tauri-apps/plugin-log';
import { isPermissionGranted, sendNotification } from '@tauri-apps/plugin-notification';
import { getCurrentWindow } from '@tauri-apps/api/window';

// Re-export log functions for convenience
export { trace, debug, info, warn, error };

// Helper to log errors with context
export function logError(context: string, err: unknown): void {
	const message = err instanceof Error ? err.message : String(err);
	error(`[${context}] ${message}`);
}

// Send a notification when the window is hidden (tray mode)
export async function notifyIfHidden(title: string, body: string): Promise<void> {
	try {
		const visible = await getCurrentWindow().isVisible();
		if (visible) return;
		const granted = await isPermissionGranted();
		if (granted) {
			sendNotification({ title, body });
		}
	} catch {
		// Silently ignore notification failures
	}
}

// Helper to log actions
export function logAction(action: string, details?: Record<string, unknown>): void {
	if (details) {
		info(`${action}: ${JSON.stringify(details)}`);
	} else {
		info(action);
	}
}
