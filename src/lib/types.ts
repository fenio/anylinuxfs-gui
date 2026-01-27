export interface Partition {
	device: string;
	size: string;
	filesystem: string;
	label: string | null;
	encrypted: boolean;
	mounted_by_system: boolean;
	system_mount_point: string | null;
	supported: boolean;
	support_note: string | null;
}

export interface Disk {
	device: string;
	size: string;
	model: string | null;
	partitions: Partition[];
}

export interface DiskListResult {
	disks: Disk[];
	has_supported_partitions: boolean;
	used_admin_mode: boolean;
}

export interface MountInfo {
	mounted: boolean;
	device: string | null;
	mount_point: string | null;
	filesystem: string | null;
	vm_running: boolean;
	ram_mb: number | null;
	vcpus: number | null;
	orphaned_instance: boolean;
}

export interface AppConfig {
	ram_mb: number | null;
	vcpus: number | null;
	log_level: string | null;
}

export interface CliStatus {
	available: boolean;
	path: string;
}
