import assert from 'node:assert/strict';
import test from 'node:test';
import { filesystemFamily, defaultMountOptions, mountOptionError, suggestedMountOptions } from '../src/lib/filesystems.ts';

test('identify filesystems without guessing the inner type of encrypted volumes', () => {
	assert.equal(filesystemFamily('XFS'), 'xfs');
	assert.equal(filesystemFamily('zfs_member'), 'zfs');
	assert.equal(filesystemFamily('ext4'), 'ext');
	assert.equal(filesystemFamily('crypto_LUKS'), 'unknown');
	assert.equal(filesystemFamily('Linux Filesystem'), 'unknown');
});

test('Btrfs compression is never suggested for other filesystems; no nobarrier defaults', () => {
	for (const fs of ['ext', 'xfs', 'f2fs', 'ntfs', 'exfat', 'fat', 'zfs', 'unknown']) {
		assert.ok(defaultMountOptions(fs).every((option) => !option.includes('compress') && option !== 'nobarrier'));
	}
	assert.ok(defaultMountOptions('btrfs').includes('compress=zstd'));
	assert.deepEqual(defaultMountOptions('zfs'), []);
	assert.deepEqual(defaultMountOptions('unknown'), []);
});

test('migration filters incompatible suggestions but tolerates malformed storage', () => {
	assert.deepEqual(suggestedMountOptions('xfs', ['noatime', 'compress-force=zstd:5', 'nobarrier', 'noatime', 42]), ['noatime']);
	assert.deepEqual(suggestedMountOptions('zfs', ['noatime', 'ro']), ['ro']);
	assert.deepEqual(suggestedMountOptions('btrfs', {}), defaultMountOptions('btrfs'));
});

test('reject ignored ZFS options and known incompatible options, not unknown advanced options', () => {
	assert.match(mountOptionError('zfs', false, 'noatime'), /ZFS/);
	assert.equal(mountOptionError('zfs', true, ''), null);
	assert.equal(mountOptionError('zfs', false, 'rw'), null);
	assert.match(mountOptionError('xfs', false, 'subvolid=256'), /Btrfs/);
	assert.equal(mountOptionError('btrfs', false, 'subvolid=256,compress=zstd'), null);
	assert.equal(mountOptionError('xfs', false, 'nouuid'), null);
	assert.equal(mountOptionError('unknown', false, 'subvolid=256'), null);
});

test('read-only/read-write conflicts are rejected for every filesystem', () => {
	for (const fs of ['ext', 'xfs', 'zfs', 'btrfs', 'unknown']) {
		assert.match(mountOptionError(fs, true, 'rw'), /not both/);
		assert.match(mountOptionError(fs, false, 'ro,rw'), /not both/);
	}
});
