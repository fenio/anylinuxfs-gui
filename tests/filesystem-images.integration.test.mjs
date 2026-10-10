// Opt-in macOS/anylinuxfs integration tests. Only newly created disposable
// images are formatted. Never accept a physical device from the environment.
import assert from 'node:assert/strict';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdtemp, open, mkdir, readdir, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import test from 'node:test';

const exec = promisify(execFile);
const enabled = process.env.ALFS_FILESYSTEM_INTEGRATION === '1';
const selected = new Set((process.env.ALFS_TEST_FILESYSTEMS || 'ext4,xfs,btrfs').split(','));
const cli = process.env.ANYLINUXFS_BIN || 'anylinuxfs';
const imageBytes = 512 * 1024 * 1024;
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const run = async (program, args) => (await exec(program, args, { timeout: 120_000, maxBuffer: 8 * 1024 * 1024 })).stdout;
const alfs = (args) => run('sudo', ['-n', cli, ...args]);

async function waitForExit(active) {
	let timer;
	try {
		return await Promise.race([
			active.finished,
			new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('Mount process did not exit after unmount')), 30_000); }),
		]);
	} finally { clearTimeout(timer); }
}

function formatCommand(fs) {
	const common = `set -eu
[ "$(blockdev --getsize64 /dev/vda)" -eq ${imageBytes} ] || exit 9
printf 'label: gpt\\nstart=2048,size=1044480,type=linux\\n' | sfdisk /dev/vda
`;
	const mkfs = {
		ext4: 'mkfs.ext4 -F /dev/vda1',
		xfs: 'mkfs.xfs -f /dev/vda1',
		btrfs: 'mkfs.btrfs -f /dev/vda1',
		exfat: 'mkfs.exfat /dev/vda1',
		ntfs: 'mkfs.ntfs -F /dev/vda1',
		f2fs: 'mkfs.f2fs -f /dev/vda1',
	};
	if (fs === 'zfs') {
		return `${common}modprobe zfs
zpool create -f -o cachefile=none -m /mnt/alfs_fixture alfs_fixture /dev/vda1
zfs create alfs_fixture/data
chmod 777 /mnt/alfs_fixture/data
printf 'filesystem fixture' > /mnt/alfs_fixture/data/alfs-fixture-seed.txt
zpool export alfs_fixture`;
	}
	assert.ok(mkfs[fs], 'Only the fixed fixture matrix may be formatted');
	return `${common}${mkfs[fs]}
mkdir -p /mnt/alfs_fixture
mount /dev/vda1 /mnt/alfs_fixture
chmod 777 /mnt/alfs_fixture
printf 'filesystem fixture' > /mnt/alfs_fixture/alfs-fixture-seed.txt
umount /mnt/alfs_fixture`;
}

async function findSeed(path, depth = 0) {
	if (depth > 8) return null;
	for (const entry of await readdir(path, { withFileTypes: true })) {
		const child = join(path, entry.name);
		if (entry.isFile() && entry.name === 'alfs-fixture-seed.txt') return child;
		if (entry.isDirectory()) {
			const found = await findSeed(child, depth + 1);
			if (found) return found;
		}
	}
	return null;
}

async function mountFixture(device, mountPoint, readOnly) {
	const args = ['-n', cli, 'mount', device, mountPoint, '-w', 'false'];
	if (readOnly) args.push('-o', 'ro');
	const child = spawn('sudo', args, { stdio: ['ignore', 'pipe', 'pipe'] });
	let output = '';
	child.stdout.on('data', (data) => output += data);
	child.stderr.on('data', (data) => output += data);
	let failure;
	child.on('error', (error) => failure = error);
	const finished = new Promise((resolve) => child.on('close', (code) => resolve(code)));
	let closed = false;
	finished.then(() => closed = true);
	try {
		for (let attempt = 0; attempt < 120; attempt++) {
			if (failure) throw failure;
			const status = await run(cli, ['status']);
			const line = status.split('\n').find((line) => line.split(' on ')[0] === device);
			if (line) {
				assert.ok(line.includes(` on ${mountPoint} (`), line);
				return { child, finished };
			}
			assert.ok(!closed, `Mount exited before appearing in status: ${output}`);
			await pause(500);
		}
		throw new Error(`Mount timeout: ${output}`);
	} catch (error) {
		// Stop only our attached fixture; never issue an unscoped stop.
		await run(cli, ['stop', device]).catch(() => {});
		child.kill('SIGTERM');
		throw error;
	}
}

for (const fs of ['ext4', 'xfs', 'btrfs', 'zfs', 'exfat', 'ntfs', 'f2fs']) {
	test(`${fs}: mount, persisted read/write, read-only rejection, unmount`, {
		skip: !enabled || !selected.has(fs), timeout: 360_000,
	}, async () => {
		assert.equal(process.platform, 'darwin', 'This harness uses macOS hdiutil');
		await run('sudo', ['-n', '-v']); // Require pre-authorized sudo; never prompt.
		const dir = await mkdtemp(join(process.env.ALFS_TEST_TMPDIR || tmpdir(), 'alfs-gui-fixture-'));
		const mountPoint = join(dir, 'mount (fixture)');
		let attached;
		let attachAttempted = false;
		let device;
		let active;
		try {
			const image = join(dir, `${fs}.img`);
			const file = await open(image, 'wx');
			try { await file.truncate(imageBytes); } finally { await file.close(); }
			await alfs(['shell', '-c', formatCommand(fs), image]);
			attachAttempted = true;
			const output = await run('hdiutil', ['attach', '-nomount', '-imagekey', 'diskimage-class=CRawDiskImage', image]);
			attached = output.match(/^\s*(\/dev\/disk\d+)(?=\s)/m)?.[1];
			assert.ok(attached, output);
			device = `${attached}s1`;
			assert.ok(output.split(/\s+/).includes(device), output);
			await mkdir(mountPoint);
			active = await mountFixture(device, mountPoint, false);
			const seed = await findSeed(mountPoint);
			assert.ok(seed, 'The fixture must be readable through the host mount');
			assert.equal(await readFile(seed, 'utf8'), 'filesystem fixture');
			await writeFile(join(dirname(seed), 'alfs-written.txt'), 'persisted through unmount');
			await run(cli, ['unmount', device]);
			assert.equal(await waitForExit(active), 0);
			active = null;
			assert.ok(!(await run(cli, ['status'])).split('\n').some((line) => line.split(' on ')[0] === device));
			active = await mountFixture(device, mountPoint, true);
			const readonlySeed = await findSeed(mountPoint);
			assert.ok(readonlySeed);
			assert.equal(await readFile(join(dirname(readonlySeed), 'alfs-written.txt'), 'utf8'), 'persisted through unmount');
			await assert.rejects(writeFile(join(dirname(readonlySeed), 'alfs-must-not-write.txt'), 'no'), (error) => ['EROFS', 'EACCES', 'EPERM'].includes(error.code));
			await run(cli, ['unmount', device]);
			assert.equal(await waitForExit(active), 0);
			active = null;
			assert.ok(!(await run(cli, ['status'])).split('\n').some((line) => line.split(' on ')[0] === device));
		} finally {
			if (device && active) {
				await run(cli, ['unmount', device]).catch(() => {});
				await run(cli, ['stop', device]).catch(() => {});
				active.child.kill('SIGTERM');
			}
			if (attached) await run('hdiutil', ['detach', attached]);
			if (!attachAttempted || attached) {
				const mounts = await run('mount', []);
				assert.ok(!mounts.includes(` on ${mountPoint} (`), `Preserving ${dir}: a fixture mount is still present`);
				await rm(dir, { recursive: true, force: true }); // Only our mkdtemp directory, after confirmed detach.
			} else {
				console.error(`Preserving fixture at ${dir}: attachment could not be identified for safe cleanup.`);
			}
		}
	});
}
