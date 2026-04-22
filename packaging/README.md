# Packaging

This repository ships packaging scaffolding for:

- macOS
  - `aarch64-apple-darwin`
  - `x86_64-apple-darwin`
  - outputs: `.app.zip`, `.pkg`
- Linux
  - `x86_64-unknown-linux-gnu`
  - `aarch64-unknown-linux-gnu`
  - outputs: `.deb`, `.rpm`, `.AppImage`, `.flatpak`
- Windows
  - `x86_64-pc-windows-msvc`
  - output: `.zip`

## CI scripts

- `scripts/ci/package-macos.sh`
  Builds a single macOS target and packages it as `.app.zip` and `.pkg`.
- `scripts/ci/package-linux.sh`
  Builds one Linux target and packages it as `.deb`, `.rpm`, `.AppImage`, and `.flatpak`.
- `scripts/ci/build-windows.ps1`
  Builds and archives the Windows executable.

## GitHub Actions

- `.github/workflows/build.yml`
  Runs build-only packaging jobs on push / pull request / manual dispatch.
- `.github/workflows/release.yml`
  Publishes artifacts to GitHub Releases when a `v*` tag is pushed.

## Notes

- Linux packaging depends on system tools like `flatpak-builder`, `rpm`, and `patchelf`.
- AppImage is currently produced with `appimagetool`.
- Flatpak uses `org.freedesktop.Platform` / `Sdk` runtime `24.08`.
- The current packaging setup is unsigned. macOS notarization and Windows signing are not configured yet.
