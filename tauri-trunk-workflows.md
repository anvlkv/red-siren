# Tauri v2 + Trunk GitHub Workflows Guide

This guide provides exact GitHub Actions workflow configurations for building Tauri v2 applications using **cargo Tauri CLI** with **Trunk** frontend builds across desktop and mobile platforms using matrix builds.

## Prerequisites

### Development Environment Setup
- **Rust** - Latest stable version with `cargo` 
- **Tauri CLI** - Installed via cargo: `cargo install tauri-cli`
- **Trunk** - Installed via cargo: `cargo install trunk`
- **WebAssembly Target** - `rustup target add wasm32-unknown-unknown`

### Desktop Platform Requirements
- **Windows**: Microsoft C++ Build Tools, WebView2 Runtime
- **Linux**: WebKit2GTK development libraries 
- **macOS**: Xcode command line tools

### Mobile Platform Requirements
- **Android**: JDK 17+, Android SDK (API 24+), Android NDK, `ANDROID_HOME` and `NDK_HOME` env vars
- **iOS**: Xcode (latest stable), iOS development targets

## Tauri Configuration for Trunk

Your `src-tauri/tauri.conf.json` should include:

```json
{
  "build": {
    "distDir": "../dist",
    "devPath": "http://localhost:8080",
    "beforeDevCommand": "trunk serve",
    "beforeBuildCommand": "trunk build --release",
    "withGlobalTauri": true
  }
}
```

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

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown,${{ matrix.target }}

      - name: Rust cache
        uses: swatinem/rust-cache@v2
        with:
          workspaces: './src-tauri -> target'

      - name: Install Trunk
        uses: jetli/trunk-action@v0.4.0
        with:
          version: 'latest'

      - name: Install Tauri CLI
        uses: baptiste0928/cargo-install@v3
        with:
          crate: tauri-cli
          version: '^2.0'
          cache-key: tauri-cli-v2

      - name: Build Tauri app
        run: cargo tauri build ${{ matrix.args }}

      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: tauri-${{ matrix.platform }}-${{ matrix.target }}
          path: |
            src-tauri/target/*/release/bundle/
            src-tauri/target/*/release/*.exe
            src-tauri/target/*/release/*.app
```

## Mobile Matrix Build Workflow

**Important**: The official `tauri-action` doesn't support mobile builds yet. Use manual CLI builds:

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
            mobile-targets: 'aarch64-linux-android,armv7-linux-androideabi,i686-linux-android,x86_64-linux-android'
          # iOS build  
          - platform: 'macos-latest'
            target: 'ios'
            mobile-targets: 'aarch64-apple-ios,x86_64-apple-ios,aarch64-apple-ios-sim'

    runs-on: ${{ matrix.platform }}
    
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown,${{ matrix.mobile-targets }}

      - name: Rust cache
        uses: swatinem/rust-cache@v2
        with:
          workspaces: './src-tauri -> target'

      - name: Install Trunk
        uses: jetli/trunk-action@v0.4.0
        with:
          version: 'latest'

      - name: Install Tauri CLI
        uses: baptiste0928/cargo-install@v3
        with:
          crate: tauri-cli
          version: '^2.0'
          cache-key: tauri-cli-v2

      - name: Setup Android SDK
        if: matrix.target == 'android'
        uses: android-actions/setup-android@v3
        with:
          api-level: 31
          target: default
          arch: x86_64

      - name: Setup Android environment
        if: matrix.target == 'android'
        run: |
          echo "ANDROID_HOME=$ANDROID_SDK_ROOT" >> $GITHUB_ENV
          echo "NDK_HOME=$ANDROID_SDK_ROOT/ndk/25.1.8937393" >> $GITHUB_ENV

      - name: Initialize mobile platform
        run: |
          if [ "${{ matrix.target }}" = "android" ]; then
            cargo tauri android init
          elif [ "${{ matrix.target }}" = "ios" ]; then
            cargo tauri ios init
          fi

      - name: Build mobile app
        run: |
          if [ "${{ matrix.target }}" = "android" ]; then
            cargo tauri android build --apk
          elif [ "${{ matrix.target }}" = "ios" ]; then
            cargo tauri ios build
          fi

      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: mobile-${{ matrix.target }}-build
          path: |
            src-tauri/gen/android/app/build/outputs/apk/**/*.apk
            src-tauri/gen/apple/**/*.app
```

## Auto-Release Workflow with Matrix

Create `.github/workflows/release.yml`:

```yaml
name: 'Auto Release'

on:
  push:
    tags: ['v*']

jobs:
  create-release:
    permissions:
      contents: write
    runs-on: ubuntu-latest
    outputs:
      release_id: ${{ steps.create-release.outputs.result }}

    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Node.js for version extraction
        uses: actions/setup-node@v4
        with:
          node-version: 'lts/*'

      - name: Get version from Cargo.toml
        id: version
        run: |
          VERSION=$(grep '^version = ' src-tauri/Cargo.toml | sed 's/version = "\(.*\)"/\1/')
          echo "VERSION=$VERSION" >> $GITHUB_ENV

      - name: Create release
        id: create-release
        uses: actions/github-script@v6
        with:
          script: |
            const { data } = await github.rest.repos.createRelease({
              owner: context.repo.owner,
              repo: context.repo.repo,
              tag_name: `app-v${process.env.VERSION}`,
              name: `Desktop App v${process.env.VERSION}`,
              body: 'Desktop and Mobile applications built with Tauri v2 and Trunk',
              draft: true,
              prerelease: false
            })
            return data.id

  build-desktop:
    needs: create-release
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

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown,${{ matrix.platform == 'macos-latest' && 'aarch64-apple-darwin,x86_64-apple-darwin' || '' }}

      - name: Rust cache
        uses: swatinem/rust-cache@v2

      - name: Install Trunk
        uses: jetli/trunk-action@v0.4.0
        with:
          version: 'latest'

      - name: Install Tauri CLI
        uses: baptiste0928/cargo-install@v3
        with:
          crate: tauri-cli
          version: '^2.0'

      - name: Build and upload to release
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          releaseId: ${{ needs.create-release.outputs.release_id }}
          args: ${{ matrix.args }}

  publish-release:
    permissions:
      contents: write
    runs-on: ubuntu-latest
    needs: [create-release, build-desktop]
    
    steps:
      - name: Publish release
        id: publish-release
        uses: actions/github-script@v6
        env:
          release_id: ${{ needs.create-release.outputs.release_id }}
        with:
          script: |
            github.rest.repos.updateRelease({
              owner: context.repo.owner,
              repo: context.repo.repo,
              release_id: process.env.release_id,
              draft: false,
              prerelease: false
            })
```

## Test-Only Workflow (No Release)

Create `.github/workflows/test-build.yml`:

```yaml
name: 'Test Build'

on: 
  pull_request:
    branches: [main]

jobs:
  test-build:
    strategy:
      fail-fast: false
      matrix:
        platform: [macos-latest, ubuntu-22.04, windows-latest]

    runs-on: ${{ matrix.platform }}
    
    steps:
      - uses: actions/checkout@v4

      - name: Install Linux dependencies
        if: matrix.platform == 'ubuntu-22.04'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.0-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown

      - name: Rust cache
        uses: swatinem/rust-cache@v2

      - name: Install Trunk
        uses: jetli/trunk-action@v0.4.0
        with:
          version: 'latest'

      - name: Install Tauri CLI
        uses: baptiste0928/cargo-install@v3
        with:
          crate: tauri-cli
          version: '^2.0'

      - name: Test build (no bundle)
        run: |
          trunk build --release
          cargo tauri build --bundles none
```

## Local Development Scripts

Add these to your project root or document them for developers:

**build.sh** (Unix/macOS):
```bash
#!/bin/bash
# Build for development
trunk build
cargo tauri dev
```

**build-release.sh** (Unix/macOS):
```bash
#!/bin/bash
# Build for production
trunk build --release
cargo tauri build
```

**mobile-dev.sh** (Unix/macOS):
```bash
#!/bin/bash
# Mobile development
if [ "$1" = "android" ]; then
    cargo tauri android dev
elif [ "$1" = "ios" ]; then
    cargo tauri ios dev  
else
    echo "Usage: ./mobile-dev.sh [android|ios]"
fi
```

## Platform-Specific Configuration

### Android Configuration

**src-tauri/gen/android/app/build.gradle** (auto-generated, but customizable):
```gradle
android {
    compileSdkVersion 33
    defaultConfig {
        minSdkVersion 24
        targetSdkVersion 33
    }
}
```

### iOS Configuration

**src-tauri/gen/apple/project.yml** (auto-generated):
```yaml
targets:
  YourApp_iOS:
    type: application
    platform: iOS
    deploymentTarget: "13.0"
```

## Performance Optimizations

### Caching Strategy

1. **Rust Cache**: Use `swatinem/rust-cache@v2` for Rust compilation artifacts
2. **Trunk Binary Cache**: Use `jetli/trunk-action` for fast Trunk installation
3. **Tauri CLI Cache**: Use `baptiste0928/cargo-install` for cached CLI installation

### Build Optimizations

1. **Trunk Release Builds**: Always use `--release` flag for production
2. **Target Specification**: Specify exact targets to avoid unnecessary compilation
3. **Parallel Builds**: Matrix strategy runs builds in parallel across platforms

## Troubleshooting

### Common Issues

1. **Trunk not found**: Ensure Trunk is installed before Tauri CLI usage
2. **WASM target missing**: Add `wasm32-unknown-unknown` target to Rust toolchain
3. **Mobile init failures**: Run mobile init commands locally first to generate required files
4. **Trunk serve port conflicts**: Default port 8080, ensure it matches `tauri.conf.json`

### Build Failures

1. **WebKit dependencies**: Install correct WebKit2GTK versions for Linux
2. **Android SDK paths**: Verify `ANDROID_HOME` and `NDK_HOME` are set correctly
3. **iOS code signing**: Ensure proper certificates for iOS builds
4. **Rust cache corruption**: Clear cache with `cargo clean` if builds fail unexpectedly

### Environment Variables for Mobile

```bash
# Android
export ANDROID_HOME=$HOME/Library/Android/sdk
export NDK_HOME=$ANDROID_HOME/ndk/25.1.8937393

# iOS (automatically configured by Xcode)
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
```

## Key Differences from Node.js-based Workflows

1. **No Node.js Setup**: Pure Rust toolchain, no `actions/setup-node` needed
2. **No Package Manager**: No npm/yarn install steps required
3. **Trunk Instead of Bundlers**: Replaces Vite, Webpack, or other JS bundlers
4. **Cargo-based Tools**: All tools installed via `cargo install` or specialized actions
5. **WASM Compilation**: Frontend compiles to WebAssembly instead of JavaScript

This guide provides comprehensive GitHub Actions workflows specifically designed for Tauri v2 applications using the cargo CLI with Trunk frontend builds, ensuring efficient CI/CD for both desktop and mobile platforms.