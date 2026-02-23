import { invoke } from '@tauri-apps/api/core';
import type { DiskListResult, MountInfo, AppConfig, CliStatus } from './types';

export async function checkCli(): Promise<CliStatus> {
	return await invoke<CliStatus>('check_cli');
}

export async function listDisks(useSudo: boolean = false, silent: boolean = false): Promise<DiskListResult> {
	return await invoke<DiskListResult>('list_disks', { useSudo, silent });
}

export async function mountDisk(device: string, passphrase?: string, readOnly?: boolean, extraOptions?: string): Promise<string> {
	return await invoke<string>('mount_disk', { device, passphrase: passphrase || null, readOnly: readOnly || false, extraOptions: extraOptions || null });
}

export async function unmountDisk(): Promise<string> {
	return await invoke<string>('unmount_disk');
}

export async function ejectDisk(device: string): Promise<string> {
	return await invoke<string>('eject_disk', { device });
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

export async function startDiskWatcher(): Promise<void> {
	return await invoke<void>('start_disk_watcher');
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

export async function startShell(image?: string): Promise<void> {
	return await invoke<void>('start_shell', { image: image || null });
}

export async function writeShell(data: string): Promise<void> {
	return await invoke<void>('write_shell', { data });
}

export async function resizeShell(rows: number, cols: number): Promise<void> {
	return await invoke<void>('resize_shell', { rows, cols });
}

export async function stopShell(): Promise<void> {
	return await invoke<void>('stop_shell');
}

export interface VmImage {
	name: string;
	installed: boolean;
}

export async function listImages(): Promise<VmImage[]> {
	return await invoke<VmImage[]>('list_images');
}

export async function installImage(name: string): Promise<void> {
	return await invoke<void>('install_image', { name });
}

export async function uninstallImage(name: string): Promise<void> {
	return await invoke<void>('uninstall_image', { name });
}

export async function listPackages(): Promise<string[]> {
	return await invoke<string[]>('list_packages');
}

export async function addPackages(packages: string[]): Promise<void> {
	return await invoke<void>('add_packages', { packages });
}

export async function removePackages(packages: string[]): Promise<void> {
	return await invoke<void>('remove_packages', { packages });
}

export interface CustomAction {
	name: string;
	description: string;
	before_mount: string;
	after_mount: string;
	before_unmount: string;
	environment: string[];
	capture_environment: string[];
	override_nfs_export: string;
	required_os: string;
	is_upstream: boolean;
}

export async function listCustomActions(): Promise<CustomAction[]> {
	return await invoke<CustomAction[]>('list_custom_actions');
}

export async function createCustomAction(action: Omit<CustomAction, 'is_upstream'>): Promise<void> {
	return await invoke<void>('create_custom_action', {
		name: action.name,
		description: action.description,
		beforeMount: action.before_mount,
		afterMount: action.after_mount,
		beforeUnmount: action.before_unmount,
		environment: action.environment,
		captureEnvironment: action.capture_environment,
		overrideNfsExport: action.override_nfs_export,
		requiredOs: action.required_os
	});
}

export async function updateCustomAction(action: Omit<CustomAction, 'is_upstream'>): Promise<void> {
	return await invoke<void>('update_custom_action', {
		name: action.name,
		description: action.description,
		beforeMount: action.before_mount,
		afterMount: action.after_mount,
		beforeUnmount: action.before_unmount,
		environment: action.environment,
		captureEnvironment: action.capture_environment,
		overrideNfsExport: action.override_nfs_export,
		requiredOs: action.required_os
	});
}

export async function deleteCustomAction(name: string): Promise<void> {
	return await invoke<void>('delete_custom_action', { name });
}

export async function setTrayUnmountEnabled(enabled: boolean): Promise<void> {
	return await invoke<void>('set_tray_unmount_enabled', { enabled });
}
