# Cleanerio

A Mac cleaner and maintenance app written in Rust with egui.

## Features

- Smart Scan: junk, threats and speed in one pass
- Cleanup: caches, logs, developer leftovers, old installers, Trash
- Protection: checks login agents, apps and running programs against a threat list
- Performance: maintenance tasks, login items, heavy apps
- Applications: uninstall with leftover files
- My Files: large and old files, duplicates, downloads
- Space Map, Cloud Files, Quick Tools, Activity

Everything removed goes to the Trash after a confirmation. Protected folders are never moved.

## Build

Requires Rust (https://rustup.rs) on macOS.

```
cargo test
cargo run
```

## Package

```
./package_mac.sh
```

Produces `dist/Cleanerio.app` and a disk image. The script signs with a Developer ID
certificate when one is in the keychain; see the notes at the top of the script for notarization.

The rules the app uses (sizes, ages, thresholds) are listed in `MECHANICS.md`.
