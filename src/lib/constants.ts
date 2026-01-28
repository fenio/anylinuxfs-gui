// Tauri event names
export const Events = {
	LOG_LINE: 'log-line',
	SHELL_OUTPUT: 'shell-output',
	SHELL_EXIT: 'shell-exit',
	DISKS_CHANGED: 'disks-changed'
} as const;

// Timeouts (in milliseconds)
export const Timeouts = {
	UNMOUNT_CLEANUP_DELAY: 500,
	RECENT_UNMOUNT_CLEAR: 3000,
	STATUS_POLL_INTERVAL: 2000,
	LOG_POLL_INTERVAL: 1000
} as const;

// Limits
export const Limits = {
	MAX_LOG_LINES: 1000,
	DEFAULT_LOG_LINES: 500
} as const;

// Device validation
const DEVICE_PATH_REGEX = /^\/dev\/[a-zA-Z0-9_-]+[a-zA-Z0-9_\-s]*$/;

export function isValidDevicePath(device: string): boolean {
	return DEVICE_PATH_REGEX.test(device) && device.length <= 64;
}

export function validateDevicePath(device: string): string | null {
	if (!device) {
		return 'Device path is required';
	}
	if (!device.startsWith('/dev/')) {
		return 'Device path must start with /dev/';
	}
	if (!isValidDevicePath(device)) {
		return 'Invalid device path format';
	}
	return null;
}
