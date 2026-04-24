.PHONY: help chmod-scripts \
	package-macos-arm64-app package-macos-arm64-dmg package-macos-arm64 \
	package-macos-x86_64-app package-macos-x86_64-dmg package-macos-x86_64 \
	package-macos-universal-dmg package-macos-universal \
	package-macos-all-app package-macos-all-dmg package-macos-all

APP_NAME := Timon

help:
	@echo "Available targets:"
	@echo "  make package-macos-arm64-app      Build macOS arm64 .app.zip"
	@echo "  make package-macos-arm64-dmg      Build macOS arm64 .dmg"
	@echo "  make package-macos-arm64          Build macOS arm64 .app.zip and .dmg"
	@echo "  make package-macos-x86_64-app     Build macOS x86_64 .app.zip"
	@echo "  make package-macos-x86_64-dmg     Build macOS x86_64 .dmg"
	@echo "  make package-macos-x86_64         Build macOS x86_64 .app.zip and .dmg"
	@echo "  make package-macos-universal-dmg  Build macOS universal .app.zip and .dmg"
	@echo "  make package-macos-all-app        Build all macOS .app.zip packages"
	@echo "  make package-macos-all-dmg        Build all macOS .dmg packages"
	@echo "  make package-macos-all            Build all macOS packages"

chmod-scripts:
	chmod +x ./scripts/ci/*.sh

package-macos-arm64-app: chmod-scripts
	OUTPUTS=app TARGET_TRIPLE=aarch64-apple-darwin ARCHIVE_PREFIX=timon-macos-aarch64 ./scripts/ci/package-macos.sh

package-macos-arm64-dmg: chmod-scripts
	OUTPUTS=dmg TARGET_TRIPLE=aarch64-apple-darwin ARCHIVE_PREFIX=timon-macos-aarch64 ./scripts/ci/package-macos.sh

package-macos-arm64: chmod-scripts
	TARGET_TRIPLE=aarch64-apple-darwin ARCHIVE_PREFIX=timon-macos-aarch64 ./scripts/ci/package-macos.sh

package-macos-x86_64-app: chmod-scripts
	OUTPUTS=app TARGET_TRIPLE=x86_64-apple-darwin ARCHIVE_PREFIX=timon-macos-x86_64 ./scripts/ci/package-macos.sh

package-macos-x86_64-dmg: chmod-scripts
	OUTPUTS=dmg TARGET_TRIPLE=x86_64-apple-darwin ARCHIVE_PREFIX=timon-macos-x86_64 ./scripts/ci/package-macos.sh

package-macos-x86_64: chmod-scripts
	TARGET_TRIPLE=x86_64-apple-darwin ARCHIVE_PREFIX=timon-macos-x86_64 ./scripts/ci/package-macos.sh

package-macos-universal-dmg: chmod-scripts
	ARM_TRIPLE=aarch64-apple-darwin X64_TRIPLE=x86_64-apple-darwin ARCHIVE_PREFIX=timon-macos-universal ./scripts/ci/package-macos-universal-dmg.sh

package-macos-universal: package-macos-universal-dmg

package-macos-all-app: package-macos-arm64-app package-macos-x86_64-app

package-macos-all-dmg: package-macos-arm64-dmg package-macos-x86_64-dmg package-macos-universal-dmg

package-macos-all: package-macos-arm64 package-macos-x86_64 package-macos-universal-dmg
