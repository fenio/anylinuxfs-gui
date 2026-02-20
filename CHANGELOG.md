# Changelog

All notable changes to this project will be documented in this file.

## [0.5.1] - 2026-02-18

### Added
- Extra mount options with split button UI and quick-chip buttons (noatime, nodiratime, nobarrier, compress-force)
- Collapsible error details for mount failures

## [0.5.0] - 2026-02-15

### Added
- Read-only mount option with RO checkbox on disk cards

### Changed
- Improved passphrase clearing, operation rate limiting, and graceful shutdown
- Optimized diskutil batching for faster disk detection

## [0.4.7] - 2026-02-13

### Changed
- Simplified sudo auth: try native PAM first (handles Touch ID), fall back to askpass dialog

## [0.4.6] - 2026-02-13

### Fixed
- Preserve ALFS_PASSPHRASE through sudo for encrypted volume mounting

## [0.4.5] - 2026-02-13

### Changed
- Improved RAID, LVM, and LUKS handling with better detection and error messages

## [0.4.0] - 2026-02-12

### Added
- RAID and LVM volume support in GUI (visible in admin mode)

### Fixed
- Svelte-check errors in shell and actions pages

## [0.3.4] - 2026-02-10

### Added
- Reinit-pending banner when VM image update is needed after CLI upgrade

## [0.3.3] - 2026-02-09

### Changed
- Updated adapter-static and dependencies

### Fixed
- Fetch full git history for changelog generation in CI

## [0.3.2] - 2026-02-01

### Fixed
- Use dynamic image list for shell dropdown instead of hardcoded values
- Use cargo install for git-cliff on macOS CI runner

### Changed
- Removed list command merging workaround

## [0.3.1] - 2026-01-29

### Added
- GUI and CLI version display in sidebar
- Automated changelog generation with git-cliff

### Fixed
- Improved cache cleanup, error sanitization, and log batching
- Added input validation and improved error handling
- Use piped stdin instead of null for command execution
- Security hardening for temp files, device validation, and cache

## [0.3.0] - 2026-01-29

### Added
- Push events for real-time status updates (reduced polling)
- Virtualized log viewer with follow mode
- Typed error handling module
- Frontend logging via tauri-plugin-log
- Constants module for timeouts, limits, and event names

### Changed
- Performance optimizations for disk listing and status checks
- Command result caching with TTL-based eviction
- Fixed hardcoded colors to use CSS custom properties

### Security
- Added Content Security Policy improvements

## [0.2.0] - 2026-01-28

### Added
- Custom actions management page (create, edit, delete mount/unmount hooks)
- VM image selector on Shell page (Alpine/FreeBSD)

### Changed
- Eject button now only shown for external drives
- Unmount before eject for safer disk removal
- Shell no longer auto-starts when navigating to Shell page

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
