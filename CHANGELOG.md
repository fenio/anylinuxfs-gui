# Changelog

All notable changes to this project will be documented in this file.

## [0.1.8] - 2025-01-27

### Added
- Eject button to safely power down and remove disks

### Fixed
- Disk detection now merges native and MS fallback results
- Linux-only cards now properly detected (not filtered by -m flag)
- Cards with broken GUID tables still work via MS fallback
- "Linux Filesystem" partition type now recognized as supported
- Disk watcher now polls for physical disk changes (catches Linux-only disks)

## [0.1.6] - 2025-01-27

### Fixed
- Ad-hoc code signing to reduce Gatekeeper issues
- Homebrew cask now auto-removes quarantine attribute on install

## [0.1.5] - 2025-01-27

### Added
- Embedded VM shell with xterm.js terminal emulator
- VM Images management page (install/uninstall alpine, freebsd images)
- Alpine packages management page (add/remove custom apk packages)
- First-run detection with setup info banner
- Free-form RAM/vCPU input (no longer constrained to dropdown options)

### Changed
- Settings now reads config from CLI (shows actual values including defaults)
- Shell auto-starts when navigating to Shell page (if no filesystem mounted)
- Improved CLI not found message

### Fixed
- Config parsing for unquoted string values from CLI output
- Init banner now disappears after VM initialization completes

## [0.1.1] - 2025-01-27

### Security
- Enabled Content Security Policy (CSP) to protect against script injection
- Disabled devtools in production builds

### Added
- Auto-refresh disk list when volumes are mounted/unmounted
- CLI path discovery (searches PATH, ANYLINUXFS_PATH env var, common locations)
- Config validation for RAM, vCPUs, and log level settings
- Watcher thread shutdown mechanism to prevent thread accumulation

### Changed
- Increased mount verification timeout from 2.5s to 10s
- Reduced tokio dependency features (smaller binary)
- Build script now cross-platform (Node.js instead of macOS-specific sed)

### Fixed
- Compiler warnings (unused imports and variables)
- Unused CSS selectors in MountStatus component
- Disk list race condition on eject (1.5s settle time)

## [0.1.0] - 2025-01-24

### Added
- Initial release
- Mount/unmount Linux filesystems (ext4, btrfs, XFS, etc.) on macOS
- Support for encrypted drives (LUKS/BitLocker)
- Real-time mount status monitoring
- Log viewer with file watching
- VM configuration (RAM, vCPUs, log level)
- Admin mode for enhanced disk detection
- Force cleanup for orphaned VM instances
