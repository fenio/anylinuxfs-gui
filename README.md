# anylinuxfs GUI

A macOS GUI application for [anylinuxfs](https://github.com/nohajc/anylinuxfs) - mount Linux filesystems (ext4, btrfs, XFS, etc.) on macOS.

## Features

- **Disk Management** - Browse and mount Linux partitions (ext2/3/4, btrfs, XFS, ZFS, etc.)
- **Safe Eject** - Properly unmount and eject external drives with one click
- **Encrypted Drives** - Support for LUKS and BitLocker encrypted volumes
- **Embedded VM Shell** - Interactive terminal with image selector (Alpine Linux or FreeBSD)
- **Custom Actions** - Create and manage mount/unmount hooks with environment variables
- **Image Management** - Install/uninstall VM images (Alpine Linux, FreeBSD for ZFS)
- **Package Management** - Add/remove custom Alpine packages to extend VM capabilities
- **Real-time Monitoring** - Live mount status and log viewer with follow mode
- **VM Configuration** - Customize RAM, vCPUs, and log verbosity
- **Auto-refresh** - Disk list updates automatically when drives are connected/ejected
- **Native macOS** - Light/dark mode support, Apple Silicon optimized

## Screenshots

<img width="1125" height="955" alt="image" src="https://github.com/user-attachments/assets/9327ee67-ebc6-4078-9a82-55bbd80a58a0" />
<img width="1125" height="955" alt="image" src="https://github.com/user-attachments/assets/4ac07f74-9571-4a20-b352-aba596bc7b47" />
<img width="1125" height="955" alt="image" src="https://github.com/user-attachments/assets/5b9dde2b-f887-41fe-aa3e-163fcd8d8711" />
<img width="1125" height="955" alt="image" src="https://github.com/user-attachments/assets/b1460151-6151-4588-b6aa-83344e0e69b5" />
<img width="1125" height="955" alt="image" src="https://github.com/user-attachments/assets/85c13af8-d94e-4e23-9b43-c6d480904d36" />
<img width="1125" height="955" alt="image" src="https://github.com/user-attachments/assets/0e2c0d45-0e82-4b20-9fb7-d91c80a73aaf" />


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

If you get "damaged" or Gatekeeper warnings, run:
```bash
xattr -cr /Applications/anylinuxfs-gui.app
```

### Manual

Download the latest DMG from [Releases](../../releases), open it, and drag the app to Applications.

**Important:** The app is not notarized by Apple. After installation, remove the quarantine attribute:

```bash
xattr -cr /Applications/anylinuxfs-gui.app
```

Then you can open the app normally.

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
