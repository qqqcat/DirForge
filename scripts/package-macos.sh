#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist/macos"
APP_NAME="DirOtter"
BIN_NAME="dirotter-macos"
BUNDLE_ID="com.dirotter.app"
VERSION="${DIROTTER_VERSION:-0.1.0}"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This packaging script must run on macOS (Darwin)."
  exit 1
fi

mkdir -p "${DIST_DIR}"

pushd "${ROOT_DIR}" >/dev/null
cargo build -p dirotter-macos --release
popd >/dev/null

APP_DIR="${DIST_DIR}/${APP_NAME}.app"
CONTENTS_DIR="${APP_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"

rm -rf "${APP_DIR}"
mkdir -p "${MACOS_DIR}"

cp "${ROOT_DIR}/target/release/${BIN_NAME}" "${MACOS_DIR}/${APP_NAME}"

cat > "${CONTENTS_DIR}/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleDisplayName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleExecutable</key>
  <string>${APP_NAME}</string>
  <key>CFBundleIdentifier</key>
  <string>${BUNDLE_ID}</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
</dict>
</plist>
PLIST

ZIP_PATH="${DIST_DIR}/${APP_NAME}-${VERSION}-macos.zip"
rm -f "${ZIP_PATH}"

pushd "${DIST_DIR}" >/dev/null
/usr/bin/zip -r "${ZIP_PATH}" "${APP_NAME}.app" >/dev/null
popd >/dev/null

echo "macOS app bundle created: ${APP_DIR}"
echo "macOS zip package created: ${ZIP_PATH}"
