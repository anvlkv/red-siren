# Tauri v2 GitHub Workflows with Matrix Builds Guide

This comprehensive guide provides exact GitHub Actions workflow configurations for building Tauri v2 applications across desktop and mobile platforms using matrix builds.

## Prerequisites

Before setting up GitHub workflows, ensure your development environment meets the following requirements:

### General Requirements
- **Rust** - Latest stable version
- **Node.js** - LTS version
- **Tauri CLI** - v2.x

### Desktop Development
- **Windows**: Microsoft C++ Build Tools, WebView2 Runtime
- **Linux**: WebKit2GTK development libraries
- **macOS**: Xcode command line tools

### Mobile Development
- **Android**: 
  - Java Development Kit (JDK) 17+
  - Android SDK (API level 24+)
  - Android NDK
  - Set `ANDROID_HOME` and `NDK_HOME` environment variables
- **iOS**: 
  - Xcode (latest stable)
  - iOS development targets: `aarch64-apple-ios`, `x86_64-apple-ios`, `aarch64-apple-ios-sim`

## Desktop Matrix Build Workflow

Create `.github/workflows/desktop-build.yml`:

```yaml
name: 'Desktop Build'

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  build-desktop:
    permissions:
      contents: write
    strategy:
      fail-fast: false
      matrix:
        include:
          # macOS builds
          - platform: 'macos-latest'
            args: '--target aarch64-apple-darwin'
            target: 'aarch64-apple-darwin'
          - platform: 'macos-latest'
            args: '--target x86_64-apple-darwin'
            target: 'x86_64-apple-darwin'
          # Linux build
          - platform: 'ubuntu-22.04'
            args: ''
            target: 'x86_64-unknown-linux-gnu'
          # Windows build
          - platform: 'windows-latest'
            args: ''
            target: 'x86_64-pc-windows-msvc'

    runs-on: ${{ matrix.platform }}
    
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Install dependencies (Ubuntu only)
        if: matrix.platform == 'ubuntu-22.04'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.0-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 'lts/*'
          cache: 'npm'

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Rust cache
        uses: swatinem/rust-cache@v2
        with:
          workspaces: './src-tauri -> target'

      - name: Install frontend dependencies
        run: npm ci

      - name: Build Tauri app
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          args: ${{ matrix.args }}
          # Remove these if you don't want to create releases
          tagName: app-v__VERSION__
          releaseName: 'Desktop App v__VERSION__'
          releaseBody: 'See the assets to download and install this app'
          releaseDraft: true
          prerelease: false
```

## Mobile Matrix Build Workflow

**Note**: As of Tauri v2.0 stable release (October 2024), the official `tauri-action` does not yet support mobile builds. Mobile apps must be built manually using the Tauri CLI until official GitHub Actions support is added.

Create `.github/workflows/mobile-build.yml`:

```yaml
name: 'Mobile Build'

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  build-mobile:
    strategy:
      fail-fast: false
      matrix:
        include:
          # Android build
          - platform: 'ubuntu-latest'
            target: 'android'
            setup-android: true
          # iOS build  
          - platform: 'macos-latest'
            target: 'ios'
            setup-ios: true

    runs-on: ${{ matrix.platform }}
    
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 'lts/*'
          cache: 'npm'

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Setup Android SDK
        if: matrix.setup-android
        uses: android-actions/setup-android@v3
        with:
          api-level: 31
          target: default
          arch: x86_64

      - name: Setup Android environment
        if: matrix.setup-android
        run: |
          echo "ANDROID_HOME=$ANDROID_SDK_ROOT" >> $GITHUB_ENV
          echo "NDK_HOME=$ANDROID_SDK_ROOT/ndk/25.1.8937393" >> $GITHUB_ENV
          rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android

      - name: Setup iOS environment
        if: matrix.setup-ios
        run: |
          rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim

      - name: Install frontend dependencies
        run: npm ci

      - name: Initialize mobile platform
        run: |
          if [ "${{ matrix.target }}" = "android" ]; then
            npm run tauri android init
          elif [ "${{ matrix.target }}" = "ios" ]; then
            npm run tauri ios init
          fi

      - name: Build mobile app
        run: |
          if [ "${{ matrix.target }}" = "android" ]; then
            npm run tauri android build
          elif [ "${{ matrix.target }}" = "ios" ]; then
            npm run tauri ios build
          fi

      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: mobile-${{ matrix.target }}-artifacts
          path: |
            src-tauri/gen/android/app/build/outputs/apk/universal/release/*.apk
            src-tauri/gen/apple/*.app
```

## Universal Build Workflow (Desktop + Mobile)

Create `.github/workflows/universal-build.yml`:

```yaml
name: 'Universal Build'

on:
  push:
    tags: ['v*']
  workflow_dispatch:

jobs:
  build-desktop:
    permissions:
      contents: write
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: 'macos-latest'
            args: '--target aarch64-apple-darwin'
          - platform: 'macos-latest' 
            args: '--target x86_64-apple-darwin'
          - platform: 'ubuntu-22.04'
            args: ''
          - platform: 'windows-latest'
            args: ''

    runs-on: ${{ matrix.platform }}
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup dependencies
        if: matrix.platform == 'ubuntu-22.04'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.0-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 'lts/*'
          cache: 'npm'

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.platform == 'macos-latest' && 'aarch64-apple-darwin,x86_64-apple-darwin' || '' }}

      - name: Rust cache
        uses: swatinem/rust-cache@v2

      - name: Install dependencies
        run: npm ci

      - name: Build desktop app
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tagName: v__VERSION__
          releaseName: 'Release v__VERSION__'
          releaseBody: 'Multi-platform release'
          releaseDraft: true
          prerelease: false
          args: ${{ matrix.args }}

  build-mobile:
    needs: build-desktop
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: 'ubuntu-latest'
            target: 'android'
          - platform: 'macos-latest'
            target: 'ios'

    runs-on: ${{ matrix.platform }}
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 'lts/*'
          cache: 'npm'

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Setup Android
        if: matrix.target == 'android'
        uses: android-actions/setup-android@v3

      - name: Configure mobile targets
        run: |
          if [ "${{ matrix.target }}" = "android" ]; then
            rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
          else
            rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim
          fi

      - name: Install dependencies
        run: npm ci

      - name: Build mobile
        run: npm run tauri ${{ matrix.target }} build

      - name: Upload mobile artifacts
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.target }}-build
          path: |
            src-tauri/gen/android/app/build/outputs/apk/universal/release/*.apk
            src-tauri/gen/apple/*.app
```

## Code Signing Configuration

### macOS Code Signing

Add these secrets to your GitHub repository:
- `APPLE_CERTIFICATE`: Base64-encoded Developer ID certificate (.p12)
- `APPLE_CERTIFICATE_PASSWORD`: Certificate password
- `KEYCHAIN_PASSWORD`: Temporary keychain password
- `APPLE_ID`: Apple ID for notarization
- `APPLE_ID_PASSWORD`: App-specific password
- `APPLE_TEAM_ID`: Team ID

### Android Code Signing

Add these secrets:
- `ANDROID_KEYSTORE`: Base64-encoded keystore file
- `ANDROID_KEYSTORE_PASSWORD`: Keystore password
- `ANDROID_KEY_ALIAS`: Key alias
- `ANDROID_KEY_PASSWORD`: Key password

## Package.json Scripts

Add these scripts to your `package.json`:

```json
{
  "scripts": {
    "tauri": "tauri",
    "dev": "tauri dev",
    "build": "tauri build",
    "android:init": "tauri android init",
    "android:dev": "tauri android dev", 
    "android:build": "tauri android build",
    "ios:init": "tauri ios init",
    "ios:dev": "tauri ios dev",
    "ios:build": "tauri ios build"
  }
}
```

## Platform-Specific Configuration

### Tauri Configuration for Mobile

Create platform-specific config files:

**tauri.android.conf.json**:
```json
{
  "bundle": {
    "android": {
      "minSdkVersion": 24,
      "permissions": [
        "android.permission.INTERNET",
        "android.permission.CAMERA"
      ]
    }
  }
}
```

**tauri.ios.conf.json**:
```json
{
  "bundle": {
    "iOS": {
      "developmentTeam": "YOUR_TEAM_ID",
      "frameworks": [],
      "minimumSystemVersion": "13.0"
    }
  }
}
```

## Troubleshooting

### Common Issues

1. **Mobile builds not supported in tauri-action**: Manual builds using CLI required until official support
2. **Android SDK path issues**: Set `ANDROID_HOME` and `NDK_HOME` environment variables correctly
3. **iOS signing issues**: Ensure proper certificates and team ID configuration
4. **Permission errors**: Add required write permissions to workflow jobs

### GitHub Secrets Setup

1. Go to repository Settings → Secrets and variables → Actions
2. Add required secrets for your target platforms
3. Ensure secrets are properly formatted (base64 for certificates)

### Mobile Platform Initialization

Before first mobile build, initialize platforms locally:
```bash
npm run tauri android init
npm run tauri ios init
```

This guide covers both current capabilities and future-ready configurations for when official GitHub Actions mobile support is added to tauri-action.