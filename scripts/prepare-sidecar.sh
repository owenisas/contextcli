#!/usr/bin/env bash
#
# Build the contextcli sidecar binary, place it at
# src-tauri/binaries/contextcli-<target-triple> as Tauri's externalBin expects,
# and (when APPLE_SIGNING_IDENTITY is set) pre-sign it with a hardened runtime
# and the app's entitlements. Pre-signing sidestepps the known notarization
# bug in tauri-apps/tauri#11992.
#
# Runs as tauri.conf.json -> build.beforeBundleCommand. Invoked from src-tauri/.

set -euo pipefail

# Resolve repo root regardless of CWD.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TAURI_DIR="$REPO_ROOT/src-tauri"
BIN_DIR="$TAURI_DIR/binaries"
ENTITLEMENTS="$TAURI_DIR/Entitlements.plist"

mkdir -p "$BIN_DIR"

# Determine host target triple (what Tauri will look for by default).
HOST_TRIPLE="$(rustc -vV | awk '/^host:/ {print $2}')"
if [[ -z "$HOST_TRIPLE" ]]; then
  echo "error: could not determine host target triple from rustc" >&2
  exit 1
fi

# Allow cross-target builds by exporting TARGETS="aarch64-apple-darwin x86_64-apple-darwin".
TARGETS="${TARGETS:-$HOST_TRIPLE}"

for TRIPLE in $TARGETS; do
  echo ">> building contextcli for $TRIPLE"
  (
    cd "$REPO_ROOT"
    cargo build --release -p contextcli --target "$TRIPLE"
  )

  SRC="$REPO_ROOT/target/$TRIPLE/release/contextcli"
  DST="$BIN_DIR/contextcli-$TRIPLE"
  cp "$SRC" "$DST"
  chmod 0755 "$DST"

  if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
    echo ">> codesigning sidecar with Developer ID ($TRIPLE)"
    codesign --force \
      --options runtime \
      --timestamp \
      --entitlements "$ENTITLEMENTS" \
      --sign "$APPLE_SIGNING_IDENTITY" \
      "$DST"
  else
    echo ">> APPLE_SIGNING_IDENTITY not set; skipping sidecar codesign (dev build)"
  fi
done

echo ">> sidecar(s) ready in $BIN_DIR"
