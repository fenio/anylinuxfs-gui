// Typed error handling for Tauri API calls

export interface ApiError {
	code: string;
	message: string;
	details?: string;
}

export type ApiResult<T> = { ok: true; data: T } | { ok: false; error: ApiError };

// Error codes for different failure scenarios
export const ErrorCodes = {
	UNKNOWN: 'UNKNOWN',
	NETWORK: 'NETWORK',
	TIMEOUT: 'TIMEOUT',
	VALIDATION: 'VALIDATION',
	NOT_FOUND: 'NOT_FOUND',
	PERMISSION: 'PERMISSION',
	MOUNT_FAILED: 'MOUNT_FAILED',
	UNMOUNT_FAILED: 'UNMOUNT_FAILED',
	CLI_NOT_FOUND: 'CLI_NOT_FOUND',
	TASK_ERROR: 'TASK_ERROR'
} as const;

export type ErrorCode = (typeof ErrorCodes)[keyof typeof ErrorCodes];

// Parse Tauri error strings into structured errors
export function parseError(err: unknown): ApiError {
	const message = err instanceof Error ? err.message : String(err);

	// Detect specific error types from message content
	if (message.includes('CLI not found') || message.includes('not found in PATH')) {
		return { code: ErrorCodes.CLI_NOT_FOUND, message: 'anylinuxfs CLI not found', details: message };
	}

	if (message.includes('Task error')) {
		return { code: ErrorCodes.TASK_ERROR, message: 'Background task failed', details: message };
	}

	if (message.includes('Mount failed') || message.includes('wrong fs type')) {
		return { code: ErrorCodes.MOUNT_FAILED, message: 'Failed to mount filesystem', details: message };
	}

	if (message.includes('Unmount failed') || message.includes('not mounted')) {
		return { code: ErrorCodes.UNMOUNT_FAILED, message: 'Failed to unmount filesystem', details: message };
	}

	if (message.includes('permission denied') || message.includes('sudo')) {
		return { code: ErrorCodes.PERMISSION, message: 'Permission denied', details: message };
	}

	if (message.includes('timeout') || message.includes('timed out')) {
		return { code: ErrorCodes.TIMEOUT, message: 'Operation timed out', details: message };
	}

	if (message.includes('Invalid') || message.includes('validation')) {
		return { code: ErrorCodes.VALIDATION, message: 'Invalid input', details: message };
	}

	return { code: ErrorCodes.UNKNOWN, message, details: undefined };
}

// Wrap an async operation with error handling
export async function wrapAsync<T>(fn: () => Promise<T>): Promise<ApiResult<T>> {
	try {
		const data = await fn();
		return { ok: true, data };
	} catch (err) {
		return { ok: false, error: parseError(err) };
	}
}

// Helper to extract error message for display
export function getErrorMessage(error: ApiError): string {
	if (error.details && error.details !== error.message) {
		return `${error.message}: ${error.details}`;
	}
	return error.message;
}

// Type guard to check if result is an error
export function isError<T>(result: ApiResult<T>): result is { ok: false; error: ApiError } {
	return !result.ok;
}
