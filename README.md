<p align="center">
  <img src="assets/icon-256.png" width="128" height="128" alt="Cleanerio app icon">
</p>

<h1 align="center">Cleanerio</h1>

<p align="center">
  A cleaner and maintenance app for Mac and Linux, written in Rust.<br>
  Find junk, large files, duplicates and unused apps, and see what fills your disk.
</p>

<p align="center">
  <a href="https://github.com/TaoMonLae/cleanerio/releases/latest">Download the latest release</a>
</p>

![Smart Scan](docs/screenshots/smart-scan.jpg)

## What it does

| Section | What you get |
|---|---|
| **Smart Scan** | One pass for junk, threats and slow-downs |
| **Cleanup** | Caches, logs, developer leftovers, mail downloads, old installers, Trash |
| **Protection** | Checks login agents, apps, browser extensions and running programs against a threat list |
| **Performance** | Maintenance tasks, login items, background items, apps using the most memory |
| **Applications** | Uninstall apps together with the files they leave behind |
| **My Files** | Large and old files, duplicates, downloads |
| **Space Map** | A bubble map of what fills each folder |
| **Cloud Files** | Free local copies of iCloud Drive files |
| **Quick Tools** | One-click fixes |
| **Activity** | Mac health and a history of what was cleaned |

## Screenshots

| Cleanup | My Files |
|---|---|
| ![Cleanup](docs/screenshots/cleanup.jpg) | ![My Files](docs/screenshots/my-files.jpg) |

| Space Map | Performance |
|---|---|
| ![Space Map](docs/screenshots/space-map.jpg) | ![Performance](docs/screenshots/performance.jpg) |

![Quick Tools](docs/screenshots/quick-tools.jpg)

The screenshots use sample files.

## Safe by design

- Everything you remove goes to the Trash after a confirmation. Only "Empty Trash" deletes for good.
- Protected folders (your home folder, Library, Documents, /Applications, /System and the like) can never be moved to the Trash.
- Caches of apps that are running are left out until you quit the app.
- Threats matched by name only, and app leftovers matched by name only, are never ticked for you.
- "Last opened" comes from the date macOS records, not from file access times.

## Install

### Mac

1. Download `Cleanerio-<version>.dmg` from the [releases page](https://github.com/TaoMonLae/cleanerio/releases).
2. Open it and drag Cleanerio to Applications.
3. Grant **Full Disk Access** in System Settings › Privacy & Security, so every folder can be scanned.

Requires an Apple Silicon Mac and macOS 11 or later.

### Linux

Pick one from the [releases page](https://github.com/TaoMonLae/cleanerio/releases):

- **Debian, Ubuntu, Mint:** `sudo apt install ./cleanerio_<version>_amd64.deb`
- **Any distribution:** unpack `cleanerio-<version>-linux-x86_64.tar.gz` and run `./install.sh`.
  It installs for your account only and needs no password. `./install.sh remove` undoes it.

Requires a 64-bit Intel or AMD computer, glibc 2.31 or later (Ubuntu 20.04, Debian 11,
Fedora 32 and newer) and a desktop with X11 or Wayland.

What differs on Linux:

- Cleanup looks in `~/.cache`, log folders, package caches of coding tools, package files in
  Downloads, and the desktop's Trash.
- Applications lists what is in your app menu. Apps you dropped into your home folder (AppImages)
  can be uninstalled. Apps owned by apt, dnf, Flatpak or Snap are shown with their size and the
  files they keep in your home folder; remove the app itself with its package manager.
- Protection checks startup apps, your background services, browser extensions and running
  programs. The bundled threat database lists Mac malware, so on Linux its main use is spotting
  startup entries that point at temporary folders or missing programs.
- Maintenance tasks that need an administrator ask through your desktop's password window (`pkexec`).

## Build from source

Requires [Rust](https://rustup.rs).

```
cargo test
cargo run
```

On a Mac, build the app bundle and disk image with:

```
./package_mac.sh
```

This produces `dist/Cleanerio.app` and `dist/Cleanerio-<version>.dmg`. The script signs with a
Developer ID certificate when one is in your keychain, and notarizes when a `notarytool`
keychain profile is set up. See the notes at the top of the script.

On Linux, build the packages with:

```
./package_linux.sh
```

This produces a `.tar.gz` and a `.deb` in `dist/`, and an AppImage when `appimagetool` is
installed. See the notes at the top of the script.

## Project layout

| Path | Contents |
|---|---|
| `src/scan.rs` | Everything that looks at the disk, and the Trash actions |
| `src/threats.rs` | The threat check |
| `src/pages.rs`, `src/app.rs` | The screens and the window |
| `src/linux.rs` | The Linux counterparts of the Mac-only parts |
| `src/art.rs`, `src/ui_kit.rs` | Drawing helpers |
| `assets/` | App icon and signing entitlements |
| `MECHANICS.md` | The rules the app uses: sizes, ages, thresholds |

Built with [egui](https://github.com/emilk/egui).

## Contributing

Contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) first; it lists the safety
rules the app must keep. Everyone taking part is expected to follow the
[Code of Conduct](CODE_OF_CONDUCT.md). Report security problems as described in
[SECURITY.md](SECURITY.md).

## License

Cleanerio is free software under the [MIT License](LICENSE). Copyright (c) 2026 Min Htet Min Aung.
You may use, copy, change and share it, including commercially, as long as the copyright and
license notice stay with it.

The crates and fonts it is built with keep their own licenses, listed in
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
