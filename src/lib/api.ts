import { invoke } from '@tauri-apps/api/core';
import type { DiskListResult, MountInfo, AppConfig, CliStatus } from './types';

export async function checkCli(): Promise<CliStatus> {
	return await invoke<CliStatus>('check_cli');
}

export async function listDisks(useSudo: boolean = false): Promise<DiskListResult> {
	return await invoke<DiskListResult>('list_disks', { useSudo });
}

export async function mountDisk(device: string, passphrase?: string): Promise<string> {
	return await invoke<string>('mount_disk', { device, passphrase: passphrase || null });
}

export async function unmountDisk(): Promise<string> {
	return await invoke<string>('unmount_disk');
}

export async function forceCleanup(): Promise<string> {
	return await invoke<string>('force_cleanup');
}

export async function getMountStatus(): Promise<MountInfo> {
	return await invoke<MountInfo>('get_mount_status');
}

export async function getLogContent(lines?: number): Promise<string[]> {
	return await invoke<string[]>('get_log_content', { lines: lines || null });
}

export async function startLogStream(): Promise<void> {
	return await invoke<void>('start_log_stream');
}

export async function getConfig(): Promise<AppConfig> {
	return await invoke<AppConfig>('get_config');
}

export async function updateConfig(
	ramMb?: number,
	vcpus?: number,
	logLevel?: string
): Promise<void> {
	return await invoke<void>('update_config', {
		ramMb: ramMb ?? null,
		vcpus: vcpus ?? null,
		logLevel: logLevel ?? null
	});
}
