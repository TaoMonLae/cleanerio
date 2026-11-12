#!/bin/bash
# Builds Cleanerio and packages it as a signed Mac app (dist/Cleanerio.app) and a disk image
# (dist/Cleanerio-<version>.dmg). Run it on a Mac:   ./package_mac.sh
#
# Signing:      uses the "Developer ID Application" certificate in your keychain.
#               Several certificates? Pick one:  SIGN_IDENTITY="Developer ID Application: Name (TEAMID)" ./package_mac.sh
# Notarizing:   happens when a notarytool keychain profile named "cleanerio-notary" exists
#               (or the name in NOTARY_PROFILE). Create it once with:
#               xcrun notarytool store-credentials cleanerio-notary --apple-id YOU@EXAMPLE.COM --team-id TEAMID
set -euo pipefail
cd "$(dirname "$0")"

[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
if ! command -v cargo >/dev/null; then
  echo "Rust isn't installed. Install it from https://rustup.rs and run this again."
  exit 1
fi

VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
APP="dist/Cleanerio.app"
DMG="dist/Cleanerio-$VERSION.dmg"
STAGE="dist/dmg"
NOTARY_PROFILE="${NOTARY_PROFILE:-cleanerio-notary}"

# ---- which certificate to sign with
IDENTITY="${SIGN_IDENTITY:-}"
if [ -z "$IDENTITY" ]; then
  IDENTITY=$(security find-identity -v -p codesigning 2>/dev/null | grep "Developer ID Application" | head -1 | sed 's/.*"\(.*\)".*/\1/' || true)
fi

echo "Building Cleanerio $VERSION (release)…"
cargo build --release

echo "Assembling ${APP}…"
rm -rf "$APP" "$STAGE" "$DMG"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/cleanerio "$APP/Contents/MacOS/Cleanerio"
cp assets/Cleanerio.icns "$APP/Contents/Resources/Cleanerio.icns"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Cleanerio</string>
  <key>CFBundleDisplayName</key><string>Cleanerio</string>
  <key>CFBundleIdentifier</key><string>com.taomonlae.cleanerio</string>
  <key>CFBundleExecutable</key><string>Cleanerio</string>
  <key>CFBundleIconFile</key><string>Cleanerio</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSAppleEventsUsageDescription</key><string>Cleanerio asks Finder to move files to the Trash and empty it, and reads your login items.</string>
</dict>
</plist>
PLIST

# ---- sign the app
if [ -n "$IDENTITY" ]; then
  echo "Signing with: $IDENTITY"
  codesign --force --options runtime --timestamp \
    --entitlements assets/entitlements.plist --sign "$IDENTITY" "$APP"
else
  echo "No \"Developer ID Application\" certificate found in your keychain."
  echo "Signing for this Mac only. To sign for other Macs, create the certificate in"
  echo "Xcode › Settings › Accounts › Manage Certificates › + › Developer ID Application,"
  echo "then run this script again."
  codesign --force --deep --sign - "$APP"
fi
codesign --verify --deep --strict "$APP"

# ---- disk image
echo "Making ${DMG}…"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "Cleanerio" -srcfolder "$STAGE" -ov -format UDZO "$DMG" >/dev/null
rm -rf "$STAGE"

# ---- sign and notarize the disk image
NOTARIZED="no"
if [ -n "$IDENTITY" ]; then
  codesign --force --timestamp --sign "$IDENTITY" "$DMG"
  if xcrun notarytool history --keychain-profile "$NOTARY_PROFILE" >/dev/null 2>&1; then
    echo "Sending to Apple for notarization (usually a few minutes)…"
    xcrun notarytool submit "$DMG" --keychain-profile "$NOTARY_PROFILE" --wait
    xcrun stapler staple "$DMG"
    NOTARIZED="yes"
  fi
fi

echo
echo "Done."
echo "  App:        $(pwd)/$APP"
echo "  Disk image: $(pwd)/$DMG"
if [ -z "$IDENTITY" ]; then
  echo "  Signed:     for this Mac only (no Developer ID certificate found)"
elif [ "$NOTARIZED" = "yes" ]; then
  echo "  Signed:     Developer ID, notarized and stapled. Opens without warnings on other Macs."
else
  echo "  Signed:     Developer ID, NOT notarized. Other Macs will still warn on first open."
  echo "  To notarize, save your credentials once, then run this script again:"
  echo "    xcrun notarytool store-credentials $NOTARY_PROFILE --apple-id YOU@EXAMPLE.COM --team-id TEAMID"
fi
echo "Open the app with:  open \"$APP\""
