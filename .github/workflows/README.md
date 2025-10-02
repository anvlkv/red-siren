# GitHub Actions Workflows

This directory contains GitHub Actions workflows for building and releasing Red Siren across multiple platforms.

## Workflows

### 1. Build and Release (`main.yml`)
Main production workflow that builds and publishes releases when code is pushed to the main branch.

**Triggers:**
- Push to `main` branch
- Manual workflow dispatch

**Features:**
- **Desktop builds:** Uses official `tauri-apps/tauri-action` for macOS (ARM64 & x64), Linux (x64), and Windows (x64)
- **Mobile builds:** Custom build steps for Android and iOS
- **Release publishing:** Creates GitHub releases with all platform artifacts
- **App Store integration:** Automatically uploads iOS builds to App Store Connect (when configured)

### 2. PR Preview (`pr-preview.yml`)
Generates preview builds for pull requests with limited platform support for faster feedback.

**Triggers:**
- Pull request opened, synchronized, or reopened

**Features:**
- **Limited platforms:** Only Ubuntu desktop and Android mobile (for speed)
- **Draft releases:** Creates draft releases marked as pre-release
- **PR comments:** Posts download links directly in PR comments
- **Short retention:** 7-day artifact retention for cleanup

## Build Matrix

| Platform | Main Branch | PR Preview |
|----------|-------------|------------|
| macOS ARM64 | ✅ | ❌ |
| macOS x64 | ✅ | ❌ |
| Ubuntu x64 | ✅ | ✅ |
| Windows x64 | ✅ | ❌ |
| Android | ✅ | ✅ |
| iOS | ✅ | ❌ |

## Required Secrets

Configure these secrets in your repository settings for full functionality:

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
- `APPLE_CERTIFICATE_PASSWORD`: Password for the P12 certificate
- `APPLE_SIGNING_IDENTITY`: Developer signing identity
- `APPLE_PROVISIONING_PROFILE`: Base64-encoded provisioning profile

### App Store Connect Upload (Required for App Store Distribution)
- `APPLE_API_KEY`: App Store Connect API Key ID
- `APPLE_API_ISSUER`: App Store Connect API Issuer ID

## Release Process

### Automatic Releases
Every push to the `main` branch automatically:
1. Builds all desktop platforms using Tauri action
2. Builds mobile platforms using custom steps
3. Creates a GitHub release with version `v{run_number}`
4. Uploads all platform artifacts
5. Publishes iOS to App Store Connect (if configured)

### PR Previews
Every pull request automatically:
1. Builds Ubuntu desktop and Android mobile only
2. Creates a draft pre-release with tag `preview-pr-{number}-{run}`
3. Posts download links in PR comments
4. Cleans up artifacts after 7 days

## Local Development

Build commands for local development:

```bash
# Desktop (all platforms)
cargo tauri build

# iOS
cargo tauri ios init
cargo tauri ios build

# Android
cargo tauri android init
cargo tauri android build --apk
```

## Dependencies

The workflows automatically install:
- Rust (stable toolchain with required targets)
- Node.js (LTS version)
- Trunk (for WASM builds)
- Platform-specific dependencies (Android SDK/NDK, Xcode tools, etc.)
- Tauri CLI v2.x

## Troubleshooting

### Build Failures
1. Check that all required Rust targets are available
2. Verify `tauri.conf.json` and platform configs are valid
3. Ensure secrets are properly configured for release builds

### Mobile Build Issues
- **Android:** Verify NDK version and signing credentials
- **iOS:** Check certificates, provisioning profiles, and App Store Connect API access

### Release Issues
- Releases are created automatically on main branch pushes
- PR previews are marked as draft and pre-release
- Check repository permissions if release creation fails

## Architecture

This workflow setup follows the **MAYA DRY KISS** principle:
- **Simple:** Only 2 workflows with clear responsibilities
- **Efficient:** Desktop uses official Tauri action, mobile uses optimized custom steps
- **Focused:** Main builds everything, PR previews build only what's needed for testing