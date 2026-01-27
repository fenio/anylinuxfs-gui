# Changelog

All notable changes to this project will be documented in this file.

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
