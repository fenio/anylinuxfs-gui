// Tauri event names
export const Events = {
	LOG_LINES: 'log-lines', // Batched log lines for better performance
	SHELL_OUTPUT: 'shell-output',
	SHELL_EXIT: 'shell-exit',
	DISKS_CHANGED: 'disks-changed',
	STATUS_CHANGED: 'status-changed'
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
const DEVICE_PATH_REGEX = /^\/dev\/[a-zA-Z0-9_-]+$/;
const RAID_PATH_REGEX = /^raid:[a-zA-Z0-9:_-]+$/;
const LVM_PATH_REGEX = /^lvm:[a-zA-Z0-9:_-]+$/;

export function isValidDevicePath(device: string): boolean {
	if (device.length > 128) return false;
	return DEVICE_PATH_REGEX.test(device) || RAID_PATH_REGEX.test(device) || LVM_PATH_REGEX.test(device);
}

export function validateDevicePath(device: string): string | null {
	if (!device) {
		return 'Device path is required';
	}
	if (!device.startsWith('/dev/') && !device.startsWith('raid:') && !device.startsWith('lvm:')) {
		return 'Device path must start with /dev/, raid:, or lvm:';
	}
	if (!isValidDevicePath(device)) {
		return 'Invalid device path format';
	}
	return null;
}
