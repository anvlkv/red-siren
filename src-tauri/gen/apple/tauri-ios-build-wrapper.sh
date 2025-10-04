#!/bin/sh
# Wrapper script for Tauri iOS builds to handle Xcode Cloud environment
# This script disables dev server and IPC mechanisms that fail in CI

set -e

# Ensure Cargo binaries are on PATH
if [ -f "$HOME/.cargo/env" ]; then
  . "$HOME/.cargo/env"
else
  export PATH="$HOME/.cargo/bin:$PATH"
fi

# Detect if we're in Xcode Cloud or CI environment
if [ -n "${CI}" ] || [ -n "${XCODE_CLOUD}" ] || [ -d "/Volumes/workspace" ]; then
  echo "Detected CI/Xcode Cloud environment - disabling Tauri dev server"

  # Disable all dev server and file watching features
  export TAURI_CLI_NO_DEV_SERVER=1
  export TAURI_CLI_NO_DEV_SERVER_WAIT=1
  export TAURI_CLI_NO_WATCH=1
  export TAURI_SKIP_DEVSERVER_CHECK=true
  export TAURI_ENV_TARGET_TRIPLE="${ARCHS:-aarch64-apple-ios}"
  export TAURI_MOBILE=true
  export CI=1

  # Create dummy addr file to bypass IPC checks
  # Determine temp directory (Xcode Cloud uses /Volumes/workspace/tmp)
  TMP_BASE="${TMPDIR:-/tmp}"
  if [ -d "/Volumes/workspace/tmp" ]; then
    TMP_BASE="/Volumes/workspace/tmp"
  fi

  # Extract bundle identifier from project path or use default
  BUNDLE_ID="com.anvlkv.red-siren.app"

  # Create required files for Tauri CLI
  ADDR_FILE="$TMP_BASE/com.anvlkv.red-siren.app-server-addr"
  LOCK_FILE="$TMP_BASE/com.anvlkv.red-siren.app-server.lock"

  mkdir -p "$TMP_BASE"

  # Create addr file with localhost address to satisfy Tauri's IPC check
  # The server won't actually be running, but this prevents the file read error
  echo "127.0.0.1:0" > "$ADDR_FILE"
  touch "$LOCK_FILE"

  echo "Created Tauri IPC files:"
  echo "  - Addr file: $ADDR_FILE"
  echo "  - Lock file: $LOCK_FILE"

  # Export temp directory for Tauri to find the files
  export TMPDIR="$TMP_BASE"
fi

# Pass through all arguments to the actual Tauri command
echo "Executing: cargo tauri ios xcode-script $*"

# Use exec to replace this process with cargo tauri
exec cargo tauri ios xcode-script "$@"
