export interface Partition {
	device: string;
	size: string;
	filesystem: string;
	label: string | null;
	uuid: string | null;
	encrypted: boolean;
	mounted_by_system: boolean;
	system_mount_point: string | null;
	supported: boolean;
	support_note: string | null;
}

export type DiskType = 'normal' | 'raid' | 'lvm';

export interface Disk {
	device: string;
	size: string;
	model: string | null;
	is_external: boolean;
	disk_type: DiskType;
	partitions: Partition[];
}

export interface DiskListResult {
	disks: Disk[];
	has_supported_partitions: boolean;
	used_admin_mode: boolean;
}

export interface MountInfo {
	device: string;
	mount_point: string;
	filesystem: string | null;
	ram_mb: number | null;
	vcpus: number | null;
}

export interface AppConfig {
	ram_mb: number | null;
	vcpus: number | null;
	log_level: string | null;
}

export interface CliStatus {
	available: boolean;
	path: string;
	initialized: boolean;
	reinit_pending: boolean;
	cli_version: string | null;
	gui_version: string;
}
