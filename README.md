# anylinuxfs GUI

A macOS GUI application for [anylinuxfs](https://github.com/containers/anylinuxfs) - mount Linux filesystems (ext4, btrfs, XFS, etc.) on macOS.

## Features

- Browse and mount Linux disk partitions
- Support for encrypted drives (LUKS/BitLocker)
- Real-time mount status monitoring
- Log viewer with follow mode
- VM configuration (RAM, vCPUs, log level)
- Native macOS look and feel (light/dark mode)

## Requirements

- macOS (Apple Silicon)
- [anylinuxfs CLI](https://github.com/containers/anylinuxfs) installed via Homebrew:
  ```
  brew install anylinuxfs
  ```

## Installation

### Homebrew (recommended)

```bash
brew install fenio/tap/anylinuxfs-gui
```

### Manual

Download the latest DMG from [Releases](../../releases), open it, and drag the app to Applications.

On first launch, right-click the app and select "Open" to bypass Gatekeeper (the app is not notarized).

## Building from Source

### Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://rustup.rs/)
- [Tauri CLI](https://tauri.app/)

### Build

```bash
npm install
npm run tauri build
```

The built app will be at `src-tauri/target/release/bundle/macos/anylinuxfs-gui.app`

## License

GPL-3.0 - see [LICENSE](LICENSE)
