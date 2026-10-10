export type FilesystemFamily = 'ext' | 'xfs' | 'btrfs' | 'f2fs' | 'reiserfs' | 'ntfs' | 'exfat' | 'fat' | 'zfs' | 'unknown';

export function filesystemFamily(filesystem: string): FilesystemFamily {
	const fs = filesystem.toLowerCase();
	if (/^ext[234]$/.test(fs)) return 'ext';
	if (fs === 'zfs' || fs === 'zfs_member') return 'zfs';
	if (['fat16', 'fat32', 'vfat', 'ms-dos'].includes(fs)) return 'fat';
	if (['xfs', 'btrfs', 'f2fs', 'reiserfs', 'ntfs', 'exfat'].includes(fs)) return fs as FilesystemFamily;
	return 'unknown';
}

export function defaultMountOptions(fs: FilesystemFamily): string[] {
	if (fs === 'zfs' || fs === 'unknown') return [];
	if (fs === 'btrfs') return ['noatime', 'nodiratime', 'compress=zstd'];
	return ['noatime', 'nodiratime'];
}

export function mountOptionError(fs: FilesystemFamily, readOnly: boolean, options: string): string | null {
	const parts = options.split(',').map((option) => option.trim()).filter(Boolean);
	if ((readOnly || parts.includes('ro')) && parts.includes('rw')) {
		return 'Choose either read-only (ro) or read-write (rw), not both.';
	}
	if (fs === 'zfs' && parts.some((option) => option !== 'ro' && option !== 'rw')) {
		return 'ZFS uses pool import and dataset properties, not ordinary Linux mount options. Only ro/rw is supported here; configure other ZFS properties through the CLI.';
	}
	if (fs !== 'unknown' && fs !== 'btrfs' && parts.some((option) => /^(subvol|subvolid|compress|compress-force)=/.test(option))) {
		return 'Subvolume and compress/compress-force mount options are Btrfs-specific. Remove them for this filesystem.';
	}
	return null;
}

export function suggestedMountOptions(fs: FilesystemFamily, stored: unknown): string[] {
	if (!Array.isArray(stored)) return defaultMountOptions(fs);
	return [...new Set(stored.filter((option): option is string => typeof option === 'string' && !!option.trim()))]
		.filter((option) => option !== 'nobarrier' && !mountOptionError(fs, false, option));
}
