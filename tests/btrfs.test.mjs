import assert from 'node:assert/strict';
import test from 'node:test';
import { withBtrfsSubvolume } from '../src/lib/btrfs.ts';

test('selecting by ID removes competing path and ID options', () => {
	assert.equal(withBtrfsSubvolume('compress=zstd,subvol=@home,subvolid=256', 'subvolid=257', false), 'compress=zstd,subvolid=257');
});

test('filesystem default removes explicit selections without changing other options', () => {
	assert.equal(withBtrfsSubvolume('ro,subvol=@', '', false), 'ro');
});

test('read-only snapshots remove rw and do not duplicate ro', () => {
	assert.equal(withBtrfsSubvolume('rw,noatime', 'subvolid=300', true), 'noatime,subvolid=300,ro');
	assert.equal(withBtrfsSubvolume('ro,rw', 'subvolid=300', true), 'ro,subvolid=300');
});

test('selecting top-level preserves a user-requested read-only mount', () => {
	assert.equal(withBtrfsSubvolume('ro,subvolid=256', 'subvolid=5', false), 'ro,subvolid=5');
});
