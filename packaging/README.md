# Packaging

This repository ships packaging scaffolding for:

- macOS
  - `aarch64-apple-darwin`
  - `x86_64-apple-darwin`
  - outputs: `.app.zip`, `.pkg`

## CI scripts

- `scripts/ci/package-macos.sh`
  Builds a single macOS target and can package `.app.zip`, `.pkg`, or both.
- `Makefile`
  The preferred local and CI entrypoint. GitHub Actions calls split targets for `app` and `pkg`, such as `make package-macos-arm64-app`, `make package-macos-arm64-pkg`, and `make package-macos-universal-pkg`.

## GitHub Actions

- `.github/workflows/build.yml`
  Currently runs macOS-only packaging jobs on push / pull request / manual dispatch.
- `.github/workflows/release.yml`
  Currently publishes macOS-only artifacts to GitHub Releases when a `v*` tag is pushed.

## Notes

- The current packaging setup is unsigned. macOS notarization is not configured yet.
