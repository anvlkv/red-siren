<img src="https://r2cdn.perplexity.ai/pplx-full-logo-primary-dark%402x.png" style="height:64px;margin-right:32px"/>

# getting this error in Xcode Cloud

Debug [jsonrpsee-client] Connecting to target: Target { host: "127.0.0.1", host_header: "127.0.0.1:0", _mode: Plain, path_and_query: "/", basic_auth: None }
Debug [jsonrpsee-client] Connecting to target: Target { host: "127.0.0.1", host_header: "127.0.0.1:0", _mode: Plain, path_and_query: "/", basic_auth: None }
Debug [jsonrpsee-client] Connecting to target: Target { host: "127.0.0.1", host_header: "127.0.0.1:0", _mode: Plain, path_and_query: "/", basic_auth: None }
thread 'main' (54210) panicked at /Users/local/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tauri-cli-2.8.4/src/mobile/mod.rs:428:6:
failed to read CLI options: Error when opening the TCP socket: Can't assign requested address (os error 49)
run with `RUST_BACKTRACE=1` environment variable to display a backtrace
Error
Command PhaseScriptExecution failed with a nonzero exit code

Based on my analysis of the error and research, this issue occurs because Tauri CLI is trying to establish communication with a development server that doesn't exist in Xcode Cloud's build environment. The error `failed to read CLI options: Error when opening the TCP socket: Can't assign requested address` indicates that the Tauri CLI is attempting to connect to `127.0.0.1:0` for development server communication, which is not available during a production build in Xcode Cloud.[^1][^2][^3]

## Root Cause

The issue stems from using development mode commands (`tauri ios dev`) or development-oriented build processes in Xcode Cloud, which expects a local development server to be running. This causes the JSON-RPC client to attempt TCP connections that fail in the CI environment.[^4][^3][^5]

## Solution: Configure Production Build for Xcode Cloud

### 1. Use Production Build Commands

Instead of using development commands, modify your Xcode Cloud build scripts to use production build commands:

**Update ci_pre_xcodebuild.sh:**

```bash
#!/bin/sh

# Ensure Rust is in PATH
export PATH="$HOME/.cargo/bin:$PATH"
source ~/.cargo/env

# Navigate to the Tauri directory
cd $CI_PRIMARY_REPOSITORY_PATH/src-tauri

# Set production build environment
export NODE_ENV=production
export TAURI_ENV=production

# Build the frontend first (production build)
if [ -f "../package.json" ]; then
    cd ..
    npm run build  # This should create a production build
    cd src-tauri
fi

# Use production iOS build instead of dev
cargo tauri ios build --release --no-bundle

echo "Production build complete"
```


### 2. Update tauri.conf.json Configuration

Ensure your Tauri configuration is set up correctly for production builds:[^6][^7]

```json
{
  "build": {
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist",
    "devPath": "http://localhost:5173"
  },
  "bundle": {
    "identifier": "com.anvlkv.red-siren",
    "iOS": {
      "developmentTeam": "YOUR_TEAM_ID",
      "minimumSystemVersion": "13.0"
    }
  }
}
```


### 3. Create Environment-Specific Configuration

You can create different configurations for development and production by using environment variables:[^8][^9]

**Add to ci_pre_xcodebuild.sh:**

```bash
# Set build configuration environment variables
export TAURI_PLATFORM=ios
export TAURI_ARCH=aarch64
export TAURI_FAMILY=mobile
export TAURI_PLATFORM_TYPE=mobile
export TAURI_DEBUG=false

# Disable development server features
export TAURI_SKIP_DEVSERVER_CHECK=true
```


### 4. Modify Xcode Build Configuration

Ensure Xcode Cloud is using the correct build configuration:[^10][^11]

1. **Create a Release Scheme** in your Xcode project specifically for Xcode Cloud
2. **Set Build Configuration** to "Release" instead of "Debug"
3. **Configure the scheme** in your Xcode Cloud workflow settings

### 5. Alternative: Use Xcode-Native Build Process

If the above doesn't work, you can bypass Tauri CLI's development server entirely by building the Rust library directly:[^5][^12]

**Replace your build script with:**

```bash
#!/bin/sh

export PATH="$HOME/.cargo/bin:$PATH"
cd $CI_PRIMARY_REPOSITORY_PATH

# Build frontend
npm install
npm run build

# Build Rust library directly without Tauri CLI dev server
cd src-tauri
cargo build --release --target aarch64-apple-ios --lib

echo "Direct build complete"
```


### 6. Disable Development Features

Add these environment variables to your Xcode Cloud configuration:[^9][^13]

- `TAURI_SKIP_DEVSERVER_CHECK=true`
- `TAURI_BUILD_RUNNER=none`
- `NODE_ENV=production`


## Key Points

- **Never use `tauri ios dev`** in Xcode Cloud - this command expects a development server[^12][^5]
- **Always use `tauri ios build --release`** for production builds[^14][^6]
- **Ensure your frontend is built** before running the Tauri build command
- **Set production environment variables** to disable development features[^13]
- **Use the correct Xcode build configuration** (Release, not Debug)[^11][^10]

This approach eliminates the JSON-RPC client connection attempts and builds your app using the production-ready assets instead of trying to connect to a non-existent development server.
<span style="display:none">[^15][^16][^17][^18][^19][^20][^21][^22][^23][^24][^25][^26][^27][^28][^29][^30][^31][^32][^33][^34][^35][^36][^37][^38][^39][^40][^41]</span>

<div align="center">⁂</div>

[^1]: https://stackoverflow.com/questions/64128286/oserror-errno-49-cant-assign-requested-address-os-x-python-socket

[^2]: https://github.com/tauri-apps/tauri/issues/10925

[^3]: https://github.com/tauri-apps/tauri/issues/10742

[^4]: https://stackoverflow.com/questions/75148428/tauri-frontend-server-not-starting

[^5]: https://stackoverflow.com/questions/79040774/how-to-run-tauri-without-dev-server

[^6]: https://v2.tauri.app/reference/cli/

[^7]: https://v2.tauri.app/reference/config/

[^8]: https://github.com/tauri-apps/tauri/issues/8418

[^9]: https://github.com/tauri-apps/tauri/discussions/11810

[^10]: https://stackoverflow.com/questions/74901175/how-to-change-build-configuration-in-xcode-cloud-pipeline

[^11]: https://stackoverflow.com/questions/5706548/how-do-i-create-a-release-build-in-xcode

[^12]: https://v2.tauri.app/develop/

[^13]: https://v2.tauri.app/reference/environment-variables/

[^14]: https://v2.tauri.app/develop/debug/

[^15]: https://www.gurux.fi/node/24803

[^16]: https://github.com/tauri-apps/tauri/issues/6659

[^17]: https://stackoverflow.com/questions/19246103/socket-errorerrno-99-cannot-assign-requested-address-and-namespace-in-python

[^18]: https://stackoverflow.com/questions/tagged/tauri

[^19]: https://metin2.dev/topic/23299-cant-assign-requested-address/

[^20]: https://tauri.app/tauri-community-survey-2022-data.csv

[^21]: https://tauri.app/llms.txt

[^22]: https://github.com/tauri-apps/tauri/issues/9509

[^23]: https://www.reddit.com/r/rust/comments/1kdp6yk/announcing_nyquest_a_truly_native_http_client/

[^24]: https://users.rust-lang.org/t/solved-testing-a-tcp-server-gives-cannot-assign-address-error/6788

[^25]: https://foojay.io/sitemap/

[^26]: https://changelog.com/sitemap.xml

[^27]: https://groups.google.com/g/golang-nuts/c/sP3DBLZMyFg

[^28]: https://packages.msys2.org/package/

[^29]: https://metin2.dev/topic/4650-socket_tcp_bind-tcp_socket-cant-assign-requested-address/

[^30]: https://github.com/tauri-apps/tauri/issues/8195

[^31]: https://github.com/tauri-apps/tauri/discussions/2197

[^32]: https://github.com/tauri-apps/tauri/issues/6077

[^33]: https://github.com/tauri-apps/tauri/issues/14233

[^34]: https://stackoverflow.com/questions/76010463/tauri-production-build

[^35]: https://dev.to/adimac93/tauri-mobile-for-ios-4dp6

[^36]: https://tauri.app/v1/api/config/

[^37]: https://www.reddit.com/r/rust/comments/uty69p/has_anyone_used_tauri_for_crossplatform_desktop/

[^38]: https://stackoverflow.com/questions/78679317/tauri-angular-app-for-macos-after-adding-signing-running-bash-script-shuts-do

[^39]: https://github.com/tauri-apps/tauri/discussions/3059

[^40]: https://wiki.nikiv.dev/programming-languages/rust/rust-libraries/

[^41]: https://v2.tauri.app/develop/configuration-files/
