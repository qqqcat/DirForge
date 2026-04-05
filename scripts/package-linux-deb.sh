#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist/linux"
PKG_NAME="dirotter"
APP_NAME="DirOtter"
BIN_NAME="dirotter-linux"
ARCH="${DIROTTER_DEB_ARCH:-amd64}"
VERSION="${DIROTTER_VERSION:-0.1.0}"
MAINTAINER="${DIROTTER_MAINTAINER:-DirOtter Team <team@dirotter.local>}"
DESCRIPTION="${DIROTTER_DESCRIPTION:-DirOtter disk analyzer and cleanup assistant}"

if ! command -v dpkg-deb >/dev/null 2>&1; then
  echo "dpkg-deb is required to build .deb packages."
  exit 1
fi

mkdir -p "${DIST_DIR}"

pushd "${ROOT_DIR}" >/dev/null
cargo build -p dirotter-linux --release
popd >/dev/null

BUILD_ROOT="${DIST_DIR}/${PKG_NAME}_${VERSION}_${ARCH}"
rm -rf "${BUILD_ROOT}"
mkdir -p \
  "${BUILD_ROOT}/DEBIAN" \
  "${BUILD_ROOT}/usr/bin" \
  "${BUILD_ROOT}/usr/share/applications"

cp "${ROOT_DIR}/target/release/${BIN_NAME}" "${BUILD_ROOT}/usr/bin/${PKG_NAME}"
chmod 0755 "${BUILD_ROOT}/usr/bin/${PKG_NAME}"

cat > "${BUILD_ROOT}/DEBIAN/control" <<CONTROL
Package: ${PKG_NAME}
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${ARCH}
Maintainer: ${MAINTAINER}
Description: ${DESCRIPTION}
CONTROL

cat > "${BUILD_ROOT}/usr/share/applications/${PKG_NAME}.desktop" <<DESKTOP
[Desktop Entry]
Name=${APP_NAME}
Comment=Disk analyzer and cleanup assistant
Exec=${PKG_NAME}
Terminal=false
Type=Application
Categories=Utility;System;
DESKTOP

DEB_PATH="${DIST_DIR}/${PKG_NAME}_${VERSION}_${ARCH}.deb"
rm -f "${DEB_PATH}"
dpkg-deb --build "${BUILD_ROOT}" "${DEB_PATH}" >/dev/null

echo "Linux .deb package created: ${DEB_PATH}"
