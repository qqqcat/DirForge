#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist/linux"
BIN_NAME="dirotter-linux"
APP_NAME="DirOtter"
VERSION="${DIROTTER_VERSION:-0.1.0}"

mkdir -p "${DIST_DIR}"

pushd "${ROOT_DIR}" >/dev/null
cargo build -p dirotter-linux --release
popd >/dev/null

STAGE_DIR="${DIST_DIR}/${APP_NAME}-${VERSION}-linux-x86_64"
rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}/bin" "${STAGE_DIR}/share/applications"

cp "${ROOT_DIR}/target/release/${BIN_NAME}" "${STAGE_DIR}/bin/${APP_NAME}"
cat > "${STAGE_DIR}/share/applications/dirotter.desktop" <<DESKTOP
[Desktop Entry]
Name=DirOtter
Comment=Disk analyzer and cleanup assistant
Exec=${APP_NAME}
Terminal=false
Type=Application
Categories=Utility;System;
DESKTOP

TARBALL="${DIST_DIR}/${APP_NAME}-${VERSION}-linux-x86_64.tar.gz"
rm -f "${TARBALL}"

tar -C "${DIST_DIR}" -czf "${TARBALL}" "$(basename "${STAGE_DIR}")"

echo "Linux package created: ${TARBALL}"
