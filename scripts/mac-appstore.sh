#!/usr/bin/env bash

cargo tauri build --bundles app --target universal-apple-darwin

codesign --force --deep --timestamp \
  --options runtime \
  --entitlements "src-tauri/Entitlements.plist" \
  --sign "Apple Distribution: Aleksandr Novolokov (M59X77GZ2V)" \
  "target/universal-apple-darwin/release/bundle/macos/Red Siren.app"

productbuild \
  --component "target/universal-apple-darwin/release/bundle/macos/Red Siren.app" /Applications \
  --sign "3rd Party Mac Developer Installer: Aleksandr Novolokov (M59X77GZ2V)" \
  --product "target/universal-apple-darwin/release/bundle/macos/Red Siren.app/Contents/Info.plist" \
  "Red_Siren.pkg"
