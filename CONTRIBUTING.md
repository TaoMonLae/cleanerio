# Contributing to Cleanerio

Thanks for wanting to help. Bug reports, fixes, new cleanup rules and design
work are all welcome.

## Licensing of contributions

Cleanerio is released under the [MIT License](LICENSE). By opening a pull
request you agree that your contribution is licensed under the same MIT
License, and you confirm that you wrote it or otherwise have the right to
submit it under that license. There is no separate contributor agreement to
sign.

If your change adds a dependency, its license must allow use in an MIT
project (MIT, Apache-2.0, BSD, ISC, Zlib and similar are fine; GPL-only or
AGPL code is not). Run `python3 tools/notices.py` so that
`THIRD-PARTY-NOTICES.md` stays current.

Do not submit code, icons or screenshots copied from other cleaner apps.

## Getting set up

You need a Mac, a Linux desktop or a Windows PC, and a recent Rust toolchain (<https://rustup.rs>).

```sh
git clone https://github.com/TaoMonLae/cleanerio.git
cd cleanerio
cargo run            # start the app
cargo test           # run the tests
```

`./package_mac.sh` builds the `.app` and the disk image; `./package_linux.sh` builds the Linux packages and `.\package_windows.ps1` the Windows ones. Signing needs your
own Developer ID; without one the script still builds an unsigned app.

## Rules the app must keep

Cleanerio touches people's files, so a few rules are not negotiable:

1. Nothing is deleted directly. Files go to the Trash, where they can be put back.
2. Nothing leaves the computer. No analytics, and no network calls except the app updater,
   which runs only when the person asks and only fetches version numbers.
3. Protected locations (see `is_protected` in `src/scan.rs`) are never offered for removal.
4. Every removal is confirmed by the person first, with the size shown.
5. Nothing runs with administrator rights.

A pull request that weakens one of these will not be merged. A change to what
gets scanned or removed needs a test in `src/scan.rs`.

## Pull requests

- Keep each pull request to one topic.
- Run `cargo fmt`, `cargo build` (no new warnings) and `cargo test` first.
- For interface changes, attach before and after screenshots.
- Say what you tested and on which macOS version or Linux distribution.
- Mac-only and Linux-only behaviour is chosen at run time (`linux::ON`, `windows::ON`), so both sides
  compile everywhere. Keep it that way: a change must build on both systems.

## Reporting bugs

Open an issue with your macOS version, Mac model (Apple silicon or Intel),
what you did, what you expected and what happened. For anything that could
put someone's files at risk, follow [SECURITY.md](SECURITY.md) instead.
