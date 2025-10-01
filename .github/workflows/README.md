# GitHub Actions Workflows

This directory contains GitHub Actions workflows for building and releasing Red Siren across multiple platforms.

## Workflows

### 1. Desktop Build (`desktop-build.yml`)
Builds desktop applications for macOS (ARM64 & x64), Linux (x64), and Windows (x64).

**Triggers:**
- Push to `main` branch
- Pull requests
- Manual workflow dispatch

**Artifacts:** Desktop bundles for each platform uploaded as workflow artifacts.

### 2. Mobile Build (`mobile-build.yml`)
Builds mobile applications for Android and iOS.

**Triggers:**
- Push to `main` branch
- Pull requests
- Manual workflow dispatch

**Artifacts:** 
- Android: APK and AAB files
- iOS: App bundles

### 3. Release (`release.yml`)
Creates official releases and publishes artifacts to GitHub Releases. Also handles App Store Connect publishing for iOS.

**Triggers:**
- Push tags matching `v*` pattern (e.g., `v1.0.0`)
- Manual workflow dispatch

**Features:**
- Creates GitHub Release
- Builds and uploads artifacts for all platforms
- Signs Android releases (if configured)
- Signs iOS releases and uploads to App Store Connect (if configured)

### 4. PR Preview (`pr-preview.yml`)
Generates preview builds for pull requests with 30-day retention.

**Triggers:**
- Pull request opened, synchronized, or reopened

**Features:**
- Builds all desktop and mobile platforms
- Uploads preview artifacts
- Posts a comment on the PR with download links

## Required Secrets

To fully utilize these workflows, configure the following secrets in your repository settings:

### Android Release Signing (Optional but Recommended)
- `ANDROID_KEYSTORE_BASE64`: Base64-encoded keystore file
  ```bash
  base64 -i your-keystore.jks | pbcopy  # macOS
  base64 -w 0 your-keystore.jks  # Linux
  ```
- `ANDROID_KEYSTORE_PASSWORD`: Password for the keystore
- `ANDROID_KEY_ALIAS`: Key alias in the keystore
- `ANDROID_KEY_PASSWORD`: Password for the key

### iOS Release Signing & Distribution (Optional but Recommended)
- `APPLE_CERTIFICATE`: Base64-encoded P12 certificate file
  ```bash
  base64 -i certificate.p12 | pbcopy  # macOS
  base64 -w 0 certificate.p12  # Linux
  ```
- `APPLE_CERTIFICATE_PASSWORD`: Password for the P12 certificate
- `APPLE_SIGNING_IDENTITY`: Developer signing identity (e.g., "Apple Development: Your Name (TEAMID)")
- `APPLE_PROVISIONING_PROFILE`: Base64-encoded provisioning profile

### App Store Connect Upload (Required for App Store Distribution)
- `APPLE_API_KEY`: App Store Connect API Key ID
- `APPLE_API_ISSUER`: App Store Connect API Issuer ID

## Creating a Release

To create a new release:

1. Tag your commit with a version number:
   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

2. The release workflow will automatically:
   - Build all platform variants
   - Create a GitHub Release
   - Upload all artifacts
   - Publish to App Store Connect (if iOS secrets are configured)

## Local Development

These workflows use standard Tauri commands. To build locally:

```bash
# Desktop
cargo tauri build

# iOS
cargo tauri ios build

# Android
cargo tauri android build
```

## Dependencies

The workflows automatically install:
- Rust (nightly toolchain as specified in `rust-toolchain.toml`)
- Node.js (LTS version)
- Trunk (for WASM builds)
- Platform-specific dependencies (Android SDK/NDK, Xcode tools, etc.)

## Troubleshooting

### Build Failures
1. Check that `rust-toolchain.toml` specifies the correct targets
2. Verify that `tauri.conf.json` and platform-specific configs are valid
3. Ensure all required secrets are set for release builds

### iOS Build Issues
- Verify signing certificates and provisioning profiles are valid
- Check that the bundle identifier matches your App Store Connect configuration
- Ensure App Store Connect API credentials have proper permissions

### Android Build Issues
- Verify NDK version compatibility
- Check that minSdkVersion in `tauri.android.conf.json` matches requirements
- For release builds, ensure keystore and signing credentials are correctly configured

## Notes

- Preview artifacts are retained for 30 days
- Release artifacts are permanently stored in GitHub Releases
- Mobile builds require more time and resources than desktop builds
- iOS builds require macOS runners
- Android builds can run on Ubuntu runners
