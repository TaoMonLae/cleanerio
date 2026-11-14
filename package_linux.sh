#!/bin/bash
# Builds Cleanerio for Linux and packages it three ways, all in dist/:
#   cleanerio-<version>-linux-<arch>.tar.gz   the program with an install script
#   cleanerio_<version>_<arch>.deb            for Debian, Ubuntu, Mint and relatives
#   Cleanerio-<version>-<arch>.AppImage       one file that runs on most distributions
# Run it on Linux:   ./package_linux.sh
#
# Older systems: with cargo-zigbuild installed (pip install ziglang cargo-zigbuild) the
#               program is built against glibc 2.31, so it also runs on Ubuntu 20.04 and
#               Debian 11. Without it, the program needs a system at least as new as this one.
# AppImage:     made when appimagetool is on the PATH (or its path is in APPIMAGETOOL).
set -euo pipefail
cd "$(dirname "$0")"

[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
if ! command -v cargo >/dev/null; then
  echo "Rust isn't installed. Install it from https://rustup.rs and run this again."
  exit 1
fi

VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
ARCH=$(uname -m)
case "$ARCH" in
  x86_64) DEB_ARCH=amd64 ;;
  aarch64) DEB_ARCH=arm64 ;;
  *) DEB_ARCH=$ARCH ;;
esac
TARGET_DIR="${CARGO_TARGET_DIR:-target}"

echo "Building Cleanerio $VERSION for Linux ($ARCH)…"
if command -v cargo-zigbuild >/dev/null; then
  cargo zigbuild --release --target "$ARCH-unknown-linux-gnu.2.31"
  BIN="$TARGET_DIR/$ARCH-unknown-linux-gnu/release/cleanerio"
  GLIBC=2.31
else
  # built against this system's C library, so it needs one at least as new
  GLIBC=$(ldd --version | head -1 | grep -o '[0-9]*\.[0-9]*$')
  echo "cargo-zigbuild isn't installed: this build needs glibc $GLIBC or later."
  cargo build --release
  BIN="$TARGET_DIR/release/cleanerio"
fi

rm -rf dist/linux
mkdir -p dist/linux

# ---- the files every package shares, laid out as they sit under /usr
ROOT="dist/linux/root"
install -Dm755 "$BIN" "$ROOT/usr/bin/cleanerio"
strip "$ROOT/usr/bin/cleanerio" 2>/dev/null || true
install -Dm644 assets/linux/cleanerio.desktop "$ROOT/usr/share/applications/cleanerio.desktop"
install -Dm644 assets/icon-256.png "$ROOT/usr/share/icons/hicolor/256x256/apps/cleanerio.png"
install -Dm644 assets/icon.svg "$ROOT/usr/share/icons/hicolor/scalable/apps/cleanerio.svg"
install -Dm644 LICENSE "$ROOT/usr/share/doc/cleanerio/copyright"
install -Dm644 THIRD-PARTY-NOTICES.md "$ROOT/usr/share/doc/cleanerio/THIRD-PARTY-NOTICES.md"

# ---- tar.gz with an install script (installs for this user only, no password needed)
TAR_NAME="cleanerio-$VERSION-linux-$ARCH"
TAR_DIR="dist/linux/$TAR_NAME"
mkdir -p "$TAR_DIR"
cp "$ROOT/usr/bin/cleanerio" assets/linux/cleanerio.desktop assets/icon-256.png LICENSE THIRD-PARTY-NOTICES.md "$TAR_DIR/"
cat > "$TAR_DIR/install.sh" <<'INSTALL'
#!/bin/sh
# Installs Cleanerio for your account only. Undo with: ./install.sh remove
set -e
cd "$(dirname "$0")"
BIN="$HOME/.local/bin"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
if [ "${1:-}" = "remove" ]; then
  rm -f "$BIN/cleanerio" "$DATA/applications/cleanerio.desktop" "$DATA/icons/hicolor/256x256/apps/cleanerio.png"
  echo "Cleanerio was removed."
  exit 0
fi
mkdir -p "$BIN" "$DATA/applications" "$DATA/icons/hicolor/256x256/apps"
install -m755 cleanerio "$BIN/cleanerio"
install -m644 icon-256.png "$DATA/icons/hicolor/256x256/apps/cleanerio.png"
sed "s|^Exec=cleanerio|Exec=$BIN/cleanerio|" cleanerio.desktop > "$DATA/applications/cleanerio.desktop"
update-desktop-database "$DATA/applications" 2>/dev/null || true
echo "Cleanerio is installed. Find it in your app menu, or run: $BIN/cleanerio"
INSTALL
chmod +x "$TAR_DIR/install.sh"
tar -C dist/linux -czf "dist/$TAR_NAME.tar.gz" "$TAR_NAME"
echo "Made dist/$TAR_NAME.tar.gz"

# ---- .deb
if command -v dpkg-deb >/dev/null; then
  DEB_DIR="dist/linux/deb"
  mkdir -p "$DEB_DIR/DEBIAN"
  cp -r "$ROOT/usr" "$DEB_DIR/"
  SIZE_KB=$(du -sk "$DEB_DIR/usr" | cut -f1)
  cat > "$DEB_DIR/DEBIAN/control" <<CONTROL
Package: cleanerio
Version: $VERSION
Section: utils
Priority: optional
Architecture: $DEB_ARCH
Installed-Size: $SIZE_KB
Depends: libc6 (>= $GLIBC), libgcc-s1
Recommends: libxkbcommon0, libwayland-client0 | libx11-6, libegl1 | libvulkan1, zenity
Maintainer: Min Htet Min Aung <TaoMonLae@users.noreply.github.com>
Homepage: https://github.com/TaoMonLae/cleanerio
Description: Cleaner that only ever moves things to the Trash
 Cleanerio finds caches, logs, old installers, large files, duplicates and
 unused apps, and shows what fills your disk. Nothing is deleted directly:
 files go to the Trash, and nothing leaves your computer.
CONTROL
  DEB="dist/cleanerio_${VERSION}_${DEB_ARCH}.deb"
  dpkg-deb --build --root-owner-group "$DEB_DIR" "$DEB" >/dev/null
  echo "Made $DEB"
else
  echo "dpkg-deb isn't installed, so no .deb was made."
fi

# ---- AppImage
TOOL="${APPIMAGETOOL:-$(command -v appimagetool || true)}"
if [ -n "$TOOL" ]; then
  APPDIR="dist/linux/Cleanerio.AppDir"
  mkdir -p "$APPDIR"
  cp -r "$ROOT/usr" "$APPDIR/"
  cp assets/linux/cleanerio.desktop "$APPDIR/cleanerio.desktop"
  cp assets/icon-256.png "$APPDIR/cleanerio.png"
  cp assets/icon-256.png "$APPDIR/.DirIcon"
  ln -sf usr/bin/cleanerio "$APPDIR/AppRun"
  IMG="dist/Cleanerio-$VERSION-$ARCH.AppImage"
  ARCH=$ARCH "$TOOL" "$APPDIR" "$IMG"
  chmod +x "$IMG"
  echo "Made $IMG"
else
  echo "appimagetool isn't installed, so no AppImage was made."
  echo "Get it from https://github.com/AppImage/appimagetool/releases and run this again."
fi

echo
echo "Checksums:"
(cd dist && sha256sum "$TAR_NAME.tar.gz" cleanerio_*.deb Cleanerio-*.AppImage 2>/dev/null || true)
