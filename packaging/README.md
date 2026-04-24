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

- macOS `.app` bundles are signed by default with ad-hoc signing (`SIGN_IDENTITY=-`).
- Set `SIGN_IDENTITY` to use a specific local or imported code-signing identity for `.app` bundles.
- Set `PKG_SIGN_IDENTITY` to sign `.pkg` installers with `productbuild --sign`.
- GitHub Actions can import a `.p12` signing certificate when these secrets are configured:
  - `MACOS_CERTIFICATE_BASE64`: base64-encoded `.p12` file.
  - `MACOS_CERTIFICATE_PASSWORD`: password for the `.p12` file.
  - `MACOS_KEYCHAIN_PASSWORD`: temporary CI keychain password.
- GitHub Actions reads signing identities from repository variables or secrets:
  - `MACOS_SIGN_IDENTITY`: app signing identity. Defaults to `-` for ad-hoc signing.
  - `MACOS_PKG_SIGN_IDENTITY`: installer signing identity. Empty means the `.pkg` is not signed.
- macOS notarization is not configured yet.
