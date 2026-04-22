#!/usr/bin/env bash
set -euo pipefail

BINARY_NAME="${BINARY_NAME:-timon}"
PROFILE="${PROFILE:-release}"
TARGET_TRIPLE="${TARGET_TRIPLE:-$(rustc -vV | sed -n 's/^host: //p')}"
DIST_DIR="${DIST_DIR:-dist}"
ARCHIVE_BASENAME="${ARCHIVE_BASENAME:-${BINARY_NAME}-${TARGET_TRIPLE}}"

cargo build --locked --profile "${PROFILE}" --target "${TARGET_TRIPLE}"

BUILD_DIR="target/${TARGET_TRIPLE}/${PROFILE}"
STAGE_DIR="${DIST_DIR}/${ARCHIVE_BASENAME}"

rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}"

cp "${BUILD_DIR}/${BINARY_NAME}" "${STAGE_DIR}/"

tar -C "${DIST_DIR}" -czf "${DIST_DIR}/${ARCHIVE_BASENAME}.tar.gz" "${ARCHIVE_BASENAME}"
