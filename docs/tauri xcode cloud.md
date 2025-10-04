<img src="https://r2cdn.perplexity.ai/pplx-full-logo-primary-dark%402x.png" style="height:64px;margin-right:32px"/>

# build tauri v2 iOS app in Xcode cloud

Run custom shell script 'Build Rust Code'

1s
Debug [tauri_cli::helpers::app_paths] Found Tauri project inside /Volumes/workspace/repository/src-tauri on early lookup

...

thread 'main' (54637) panicked at /Users/local/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tauri-cli-2.8.4/src/mobile/mod.rs:415:59:
failed to read missing addr file /Volumes/workspace/tmp/com.anvlkv.red-siren.app-server-addr: No such file or directory (os error 2)
run with `RUST_BACKTRACE=1` environment variable to display a backtrace
Error
Command PhaseScriptExecution failed with a nonzero exit code

Based on my research, this is a common issue with Tauri v2 iOS builds in Xcode Cloud. The error occurs because the Tauri CLI's development server communication mechanism isn't properly set up in the Xcode Cloud environment. Here's a comprehensive solution for building Tauri v2 iOS apps in Xcode Cloud:

## Understanding the Error

The error `failed to read missing addr file /Volumes/workspace/tmp/com.anvlkv.red-siren.app-server-addr` indicates that Tauri CLI's internal communication system between the build process and development server is failing. This happens because Xcode Cloud's sandboxed environment prevents the normal file-based IPC mechanism that Tauri uses locally.[^1][^2]

## Solution: Configure Xcode Cloud Custom Build Scripts

### 1. Create the ci_scripts Directory

In your Xcode project, create a `ci_scripts` directory at the root level (same level as your `.xcodeproj` file):[^3][^4]

```
YourProject/
├── ci_scripts/
│   ├── ci_post_clone.sh
│   └── ci_pre_xcodebuild.sh
├── YourProject.xcodeproj
└── src-tauri/
```


### 2. Create ci_post_clone.sh

This script runs after Xcode Cloud clones your repository and should install Rust and Tauri CLI:[^4][^5]

```bash
#!/bin/sh

# Navigate to the repository root
cd $CI_PRIMARY_REPOSITORY_PATH

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env

# Add Rust to PATH for subsequent scripts
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bash_profile

# Install Tauri CLI
cargo install tauri-cli --version 2.8.4

# Install frontend dependencies if needed
if [ -f "package.json" ]; then
    npm install
fi

echo "Post-clone setup complete"
```


### 3. Create ci_pre_xcodebuild.sh

This script runs before the Xcode build and should handle the Tauri build process:[^6][^4]

```bash
#!/bin/sh

# Ensure Rust is in PATH
export PATH="$HOME/.cargo/bin:$PATH"

# Navigate to the Tauri directory
cd $CI_PRIMARY_REPOSITORY_PATH/src-tauri

# Set environment variables needed for iOS build
export APPLE_DEVELOPMENT_TEAM="${APPLE_DEVELOPMENT_TEAM}"
export TAURI_SIGNING_PRIVATE_KEY="${TAURI_SIGNING_PRIVATE_KEY}"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD}"

# Build the frontend first
if [ -f "../package.json" ]; then
    cd ..
    npm run build
    cd src-tauri
fi

# Build Rust code for iOS
cargo tauri ios build --release

echo "Pre-xcodebuild script complete"
```


### 4. Make Scripts Executable

Run these commands in your terminal to make the scripts executable:[^7][^6]

```bash
chmod +x ci_scripts/ci_post_clone.sh
chmod +x ci_scripts/ci_pre_xcodebuild.sh
```


## Configuration Updates

### Update tauri.conf.json

Ensure your Tauri configuration includes the necessary iOS settings:[^8][^9]

```json
{
  "build": {
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
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


### Environment Variables in Xcode Cloud

Configure these environment variables in your Xcode Cloud workflow:[^10][^3]

- `APPLE_DEVELOPMENT_TEAM`: Your Apple Developer Team ID
- `TAURI_SIGNING_PRIVATE_KEY`: Your signing certificate (if needed)
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: Certificate password (if needed)


## Alternative Approach: Xcode-Only Build

If the custom scripts approach doesn't work, you can modify your build script to use Xcode's native building process:[^9][^11]

1. **Remove the custom Tauri build script** from your Xcode project's build phases
2. **Add a new "Run Script" build phase** with:
```bash
#!/bin/sh
export PATH="$HOME/.cargo/bin:$PATH"

# Only build Rust code, let Xcode handle the rest
cd "${SRCROOT}/../src-tauri"
cargo build --release --target aarch64-apple-ios
```


## Development Team Configuration

Make sure your development team is properly configured:[^12][^8]

1. In Xcode, go to your project settings
2. Under "Signing \& Capabilities", ensure "Automatically manage signing" is enabled
3. Select your development team from the dropdown
4. Ensure the bundle identifier matches your Tauri configuration

## Troubleshooting Tips

1. **Check Tauri CLI version**: Ensure you're using a recent version (2.8.4+)[^2]
2. **Verify file paths**: Make sure all paths in your scripts are correct
3. **Add logging**: Include `echo` statements in your scripts for debugging
4. **Test locally**: Run the scripts locally first to ensure they work

This configuration should resolve the missing addr file error by properly setting up the build environment in Xcode Cloud and ensuring all dependencies are correctly installed and configured.
<span style="display:none">[^13][^14][^15][^16][^17][^18][^19][^20][^21][^22][^23][^24][^25][^26][^27][^28][^29][^30][^31][^32][^33][^34][^35][^36][^37][^38][^39][^40][^41][^42][^43][^44]</span>

<div align="center">⁂</div>

[^1]: https://stackoverflow.com/questions/78246822/tauri-with-ng-app-uses-local-ip-instead-localhost-for-accessing-dev-server

[^2]: https://zenn.dev/laiso/scraps/84f7ec78078a0f

[^3]: https://www.createwithswift.com/custom-build-scripts-and-environment-variables-for-xcode-cloud/

[^4]: https://developer.apple.com/videos/play/wwdc2021/10269/

[^5]: https://encoresky.com/blog/xcode-cloud-integration/

[^6]: https://stackoverflow.com/questions/73062195/how-to-use-custom-xcode-cloud-environment-variables

[^7]: https://stackoverflow.com/questions/73062195/how-to-use-custom-xcode-cloud-environment-variables/75225641

[^8]: https://github.com/tauri-apps/tauri/discussions/8325

[^9]: https://v2.tauri.app/distribute/app-store/

[^10]: https://github.com/tauri-apps/tauri/discussions/11810

[^11]: https://github.com/tauri-apps/tauri/discussions/10197

[^12]: https://stackoverflow.com/questions/73468873/change-team-in-xcode-cloud-configure-screen

[^13]: https://github.com/tauri-apps/tauri/issues/13930

[^14]: https://github.com/tauri-apps/tauri/issues/13284

[^15]: https://github.com/tauri-apps/tauri/issues/6077

[^16]: https://v2.tauri.app/reference/config/

[^17]: https://github.com/tauri-apps/tauri/issues/10635

[^18]: https://github.com/tauri-apps/tauri/issues/13547

[^19]: https://v2.tauri.app/plugin/shell/

[^20]: https://v2.tauri.app/develop/

[^21]: https://tauri.app/v1/api/config

[^22]: https://github.com/tauri-apps/tauri/issues/8418

[^23]: https://stackoverflow.com/questions/78976640/unable-to-produce-a-working-build-using-nextjs-tauri-app

[^24]: https://stackoverflow.com/questions/67833794/npm-err-could-not-determine-executable-to-run

[^25]: https://developer.apple.com/documentation/xcode/writing-custom-build-scripts

[^26]: https://tauri.app/llms.txt

[^27]: https://classic.yarnpkg.com/en/package/@tauri-apps/cli

[^28]: https://stackoverflow.com/questions/78651388/how-would-i-build-my-tauri-app-using-tauri-clirun-command

[^29]: https://dev.to/adimac93/tauri-mobile-for-ios-4dp6

[^30]: https://stackoverflow.com/questions/78318927/how-to-customize-my-programs-installer-made-with-tauri

[^31]: https://stackoverflow.com/questions/10167442/whats-the-xcode-no-such-file-or-directory-error

[^32]: https://www.youtube.com/watch?v=eqswOAJdJ9U

[^33]: https://stackoverflow.com/questions/55235825/error-failed-to-build-ios-project-we-ran-xcodebuild-command-but-it-exited-wit

[^34]: https://v2.tauri.app/reference/environment-variables/

[^35]: https://developer.apple.com/documentation/xcode/configuring-xcode-cloud-for-your-team

[^36]: https://developer.apple.com/forums/topics/code-signing-topic?page=2\&sortBy=newest

[^37]: https://gitverse.ru/mibazarow/tauri-mobileapp-template

[^38]: https://developer.apple.com/forums/forums/topics/developer-tools-and-services/developer-tools-and-services-general

[^39]: https://github.com/tauri-apps/tauri/issues/13542

[^40]: https://tauri.app/v1/api/config/

[^41]: https://stackoverflow.com/questions/77141841/xcode-version-15-0-command-phasescriptexecution-failed-with-a-nonzero-exit-cod

[^42]: https://www.reddit.com/r/rust/comments/1fukj52/tauri_20_stable_has_just_been_released/

[^43]: https://github.com/tauri-apps/tauri/discussions/4964

[^44]: https://stackoverflow.com/questions/tagged/tauri

