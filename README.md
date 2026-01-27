# anylinuxfs GUI

A macOS GUI application for [anylinuxfs](https://github.com/nohajc/anylinuxfs) - mount Linux filesystems (ext4, btrfs, XFS, etc.) on macOS.

## Features

- Browse and mount Linux disk partitions
- Support for encrypted drives (LUKS/BitLocker)
- Real-time mount status monitoring
- Log viewer with follow mode
- VM configuration (RAM, vCPUs, log level)
- Native macOS look and feel (light/dark mode)

## Screenshots

<img width="1089" height="906" alt="image" src="https://github.com/user-attachments/assets/a50c98fe-bf7e-414c-9ef0-016bcf1101bb" />
<img width="1089" height="906" alt="image" src="https://github.com/user-attachments/assets/036ef98d-5b44-45a4-baff-40bb41532878" />


## Requirements

- macOS (Apple Silicon)
- [anylinuxfs CLI](https://github.com/nohajc/anylinuxfs) installed via Homebrew:
  ```
  brew install nohajc/anylinuxfs/anylinuxfs
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
