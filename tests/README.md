# Filesystem compatibility tests

`npm test` runs option-validation unit tests. Disposable-image integration tests
are **skipped unless explicitly enabled**; ordinary CI does not format or mount disks.
Rust regression tests run with `cd src-tauri && cargo test --locked`.

## Opt-in image tests (macOS)

Requirements:

- macOS, Node 24+, `hdiutil`, and an initialized anylinuxfs CLI with multi-instance support.
- Administrator access. Run `sudo -v` yourself first; tests use `sudo -n` and never prompt.
- The Linux VM needs `sfdisk`/`blockdev` (util-linux) and the formatters for selected filesystems.
  The default matrix needs e2fsprogs, xfsprogs, and btrfs-progs. Optional cases need
  exfatprogs, ntfs-3g, f2fs-tools, or working ZFS tooling/kernel modules.
- Review your anylinuxfs configuration/custom actions first: tests use the normal
  CLI mount flow, which may run configured actions. Tests do not install packages,
  alter configuration, or attempt filesystem repairs.

```sh
sudo -v
npm run test:filesystems
```

The default matrix is **ext4, XFS, Btrfs**. Select additional installed filesystems explicitly:

```sh
ALFS_TEST_FILESYSTEMS=ext4,xfs,btrfs,zfs,exfat,ntfs,f2fs npm run test:filesystems
```

Optional `ANYLINUXFS_BIN` selects a CLI executable and `ALFS_TEST_TMPDIR` selects
a temporary-directory parent. There is deliberately **no input-device option**.

Each test creates its own sparse 512 MiB image, partitions/formats only that image
inside a VM, attaches it through hdiutil, and mounts its partition through anylinuxfs.
It verifies readable fixture data, a persisted write across unmount/remount,
read-only write rejection, and exact-device disappearance after unmount. ZFS puts
the fixture in a child dataset. Mount paths include spaces and parentheses.
Cleanup targets only that test's attached device and temporary directory; a failed
detach or a remaining host mount prevents deletion of the image. An unidentified
attachment is also preserved for manual cleanup. No unscoped `stop` is issued.

These are **CLI/VM integration tests**, not GUI automation. They do not establish
password/key-file behavior, Interactive Terminal UX, every filesystem feature,
multi-device pools, or recovery from damaged disks. Unit tests cover the GUI's
option policy and Rust status/error classification separately. Encrypted volumes
and both elevation modes still need dedicated end-to-end verification.
