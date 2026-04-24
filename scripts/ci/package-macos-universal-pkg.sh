#!/usr/bin/env bash
set -euo pipefail

APP_NAME="${APP_NAME:-Timon}"
BINARY_NAME="${BINARY_NAME:-timon}"
BUNDLE_ID="${BUNDLE_ID:-io.vacivor.timon}"
PROFILE="${PROFILE:-release}"
DIST_DIR="${DIST_DIR:-dist}"
ARM_TRIPLE="${ARM_TRIPLE:-aarch64-apple-darwin}"
X64_TRIPLE="${X64_TRIPLE:-x86_64-apple-darwin}"
VERSION="${VERSION:-$(sed -n 's/^version = \"\\(.*\\)\"$/\\1/p' Cargo.toml | head -n 1)}"
ARCHIVE_PREFIX="${ARCHIVE_PREFIX:-timon-macos-universal}"
SIGN_IDENTITY="${SIGN_IDENTITY:--}"
PKG_SIGN_IDENTITY="${PKG_SIGN_IDENTITY:-}"

cargo build --locked --profile "${PROFILE}" --target "${ARM_TRIPLE}"
cargo build --locked --profile "${PROFILE}" --target "${X64_TRIPLE}"

BUILD_ROOT="${DIST_DIR}/macos/universal"
APP_DIR="${BUILD_ROOT}/${APP_NAME}.app"
CONTENTS_DIR="${APP_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"

rm -rf "${BUILD_ROOT}"
mkdir -p "${MACOS_DIR}" "${CONTENTS_DIR}/Resources"

lipo -create \
  "target/${ARM_TRIPLE}/${PROFILE}/${BINARY_NAME}" \
  "target/${X64_TRIPLE}/${PROFILE}/${BINARY_NAME}" \
  -output "${MACOS_DIR}/${APP_NAME}"

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

productbuild_args=()

if [[ -n "${PKG_SIGN_IDENTITY}" ]]; then
  productbuild_args+=(--sign "${PKG_SIGN_IDENTITY}")
fi

productbuild_args+=(--component "${APP_DIR}" "/Applications")
productbuild "${productbuild_args[@]}" "${DIST_DIR}/${ARCHIVE_PREFIX}.pkg"
