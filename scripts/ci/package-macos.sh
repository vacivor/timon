#!/usr/bin/env bash
set -euo pipefail

APP_NAME="${APP_NAME:-Timon}"
BINARY_NAME="${BINARY_NAME:-Timon}"
BUNDLE_ID="${BUNDLE_ID:-io.vacivor.timon}"
PROFILE="${PROFILE:-release}"
DIST_DIR="${DIST_DIR:-dist}"
TARGET_TRIPLE="${TARGET_TRIPLE:?TARGET_TRIPLE is required}"
VERSION="${VERSION:-$(sed -n 's/^version = \"\\(.*\\)\"$/\\1/p' Cargo.toml | head -n 1)}"
ARCHIVE_PREFIX="${ARCHIVE_PREFIX:-timon-macos}"
OUTPUTS="${OUTPUTS:-app,dmg}"
SIGN_IDENTITY="${SIGN_IDENTITY:--}"

has_output() {
  case ",${OUTPUTS}," in
    *,"$1",*) return 0 ;;
    *) return 1 ;;
  esac
}

cargo build --locked --profile "${PROFILE}" --target "${TARGET_TRIPLE}"

BUILD_ROOT="${DIST_DIR}/macos/${TARGET_TRIPLE}"
APP_DIR="${BUILD_ROOT}/${APP_NAME}.app"
CONTENTS_DIR="${APP_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"

rm -rf "${BUILD_ROOT}"
mkdir -p "${MACOS_DIR}" "${CONTENTS_DIR}/Resources"

cp "target/${TARGET_TRIPLE}/${PROFILE}/${BINARY_NAME}" "${MACOS_DIR}/${APP_NAME}"
chmod +x "${MACOS_DIR}/${APP_NAME}"

cat > "${CONTENTS_DIR}/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>${APP_NAME}</string>
  <key>CFBundleIdentifier</key>
  <string>${BUNDLE_ID}</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>${APP_NAME}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${VERSION}</string>
  <key>CFBundleVersion</key>
  <string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF

mkdir -p "${DIST_DIR}"

if [[ -n "${SIGN_IDENTITY}" ]]; then
  codesign \
    --force \
    --deep \
    --options runtime \
    --sign "${SIGN_IDENTITY}" \
    "${APP_DIR}"

  codesign --verify --deep --strict "${APP_DIR}"
fi

if has_output app; then
  rm -f "${DIST_DIR}/${ARCHIVE_PREFIX}.app.zip"
  ditto -c -k --sequesterRsrc --keepParent "${APP_DIR}" "${DIST_DIR}/${ARCHIVE_PREFIX}.app.zip"
fi

if has_output dmg; then
  DMG_STAGING="${BUILD_ROOT}/dmg"
  DMG_PATH="${DIST_DIR}/${ARCHIVE_PREFIX}.dmg"

  rm -rf "${DMG_STAGING}"
  mkdir -p "${DMG_STAGING}"
  ditto "${APP_DIR}" "${DMG_STAGING}/${APP_NAME}.app"
  ln -s /Applications "${DMG_STAGING}/Applications"

  rm -f "${DMG_PATH}"
  hdiutil create \
    -volname "${APP_NAME}" \
    -srcfolder "${DMG_STAGING}" \
    -ov \
    -format UDZO \
    "${DMG_PATH}"
fi
