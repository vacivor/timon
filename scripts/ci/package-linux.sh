#!/usr/bin/env bash
set -euo pipefail

APP_NAME="${APP_NAME:-Timon}"
APP_ID="${APP_ID:-io.vacivor.timon}"
BINARY_NAME="${BINARY_NAME:-timon}"
PROFILE="${PROFILE:-release}"
TARGET_TRIPLE="${TARGET_TRIPLE:?TARGET_TRIPLE is required}"
DIST_DIR="${DIST_DIR:-dist}"
VERSION="${VERSION:-$(sed -n 's/^version = \"\\(.*\\)\"$/\\1/p' Cargo.toml | head -n 1)}"
ARCHIVE_PREFIX="${ARCHIVE_PREFIX:-timon-linux}"

cargo build --locked --profile "${PROFILE}" --target "${TARGET_TRIPLE}"

case "${TARGET_TRIPLE}" in
  x86_64-*) APPIMAGE_ARCH="x86_64"; FLATPAK_ARCH="x86_64" ;;
  aarch64-*|arm64-*) APPIMAGE_ARCH="aarch64"; FLATPAK_ARCH="aarch64" ;;
  *) echo "Unsupported Linux target: ${TARGET_TRIPLE}" >&2; exit 1 ;;
esac

BINARY_PATH="target/${TARGET_TRIPLE}/${PROFILE}/${BINARY_NAME}"
APPDIR="${DIST_DIR}/AppDir"
FLATPAK_SRC="${DIST_DIR}/flatpak-src"
TOOLS_DIR="${DIST_DIR}/tools"
FLATPAK_BRANCH="${FLATPAK_BRANCH:-stable}"

rm -rf "${APPDIR}" "${FLATPAK_SRC}"
mkdir -p \
  "${APPDIR}/usr/bin" \
  "${APPDIR}/usr/share/applications" \
  "${APPDIR}/usr/share/icons/hicolor/scalable/apps" \
  "${APPDIR}/usr/share/metainfo" \
  "${FLATPAK_SRC}" \
  "${TOOLS_DIR}"

cp "${BINARY_PATH}" "${APPDIR}/usr/bin/${BINARY_NAME}"
cp packaging/linux/${APP_ID}.desktop "${APPDIR}/usr/share/applications/${APP_ID}.desktop"
cp packaging/linux/${APP_ID}.svg "${APPDIR}/usr/share/icons/hicolor/scalable/apps/${APP_ID}.svg"
cp packaging/linux/${APP_ID}.metainfo.xml "${APPDIR}/usr/share/metainfo/${APP_ID}.metainfo.xml"
cp packaging/linux/${APP_ID}.desktop "${APPDIR}/${APP_ID}.desktop"
cp packaging/linux/${APP_ID}.svg "${APPDIR}/${APP_ID}.svg"

cat > "${APPDIR}/AppRun" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "${HERE}/usr/bin/timon" "$@"
EOF
chmod +x "${APPDIR}/AppRun"

mkdir -p target/release
cp "${BINARY_PATH}" "target/release/${BINARY_NAME}"

cp "${BINARY_PATH}" "${FLATPAK_SRC}/timon"
cp packaging/linux/${APP_ID}.desktop "${FLATPAK_SRC}/${APP_ID}.desktop"
cp packaging/linux/${APP_ID}.svg "${FLATPAK_SRC}/${APP_ID}.svg"
cp packaging/linux/${APP_ID}.metainfo.xml "${FLATPAK_SRC}/${APP_ID}.metainfo.xml"
cp packaging/linux/${APP_ID}.flatpak.yml "${FLATPAK_SRC}/${APP_ID}.flatpak.yml"

cargo deb --no-build --target "${TARGET_TRIPLE}" --output "${DIST_DIR}/${ARCHIVE_PREFIX}.deb"
cargo generate-rpm --target "${TARGET_TRIPLE}" -o "${DIST_DIR}/${ARCHIVE_PREFIX}.rpm"

APPIMAGETOOL="${TOOLS_DIR}/appimagetool-${APPIMAGE_ARCH}.AppImage"
if [[ ! -x "${APPIMAGETOOL}" ]]; then
  curl -L \
    "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${APPIMAGE_ARCH}.AppImage" \
    -o "${APPIMAGETOOL}"
  chmod +x "${APPIMAGETOOL}"
fi

ARCH="${APPIMAGE_ARCH}" APPIMAGE_EXTRACT_AND_RUN=1 \
  "${APPIMAGETOOL}" "${APPDIR}" "${DIST_DIR}/${ARCHIVE_PREFIX}.AppImage"

flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install -y flathub org.freedesktop.Platform//24.08 org.freedesktop.Sdk//24.08

flatpak-builder \
  --force-clean \
  --arch="${FLATPAK_ARCH}" \
  --default-branch="${FLATPAK_BRANCH}" \
  --repo="${DIST_DIR}/flatpak-repo" \
  "${DIST_DIR}/flatpak-build" \
  "${FLATPAK_SRC}/${APP_ID}.flatpak.yml"

flatpak build-bundle \
  "${DIST_DIR}/flatpak-repo" \
  "${DIST_DIR}/${ARCHIVE_PREFIX}.flatpak" \
  "${APP_ID}" \
  "${FLATPAK_BRANCH}"
