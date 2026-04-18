# macOS Code Signing & Notarization

A signed + notarized `.dmg` opens on a fresh Mac with zero Gatekeeper prompts and passes the sidecar notarization bug in [tauri-apps/tauri#11992](https://github.com/tauri-apps/tauri/issues/11992).

## One-time setup

1. Join the Apple Developer Program.
2. In Xcode → Settings → Accounts, create a **Developer ID Application** certificate. Install it into the login keychain.
3. Generate an **app-specific password** at [appleid.apple.com](https://appleid.apple.com/account/manage) → Sign-In and Security → App-Specific Passwords. Save it — it is used for notarization.
4. Note your **Team ID** from [developer.apple.com/account](https://developer.apple.com/account) → Membership.

## Environment variables for a release build

```sh
export APPLE_SIGNING_IDENTITY="Developer ID Application: <Your Name> (<TEAMID>)"
export APPLE_ID="you@example.com"
export APPLE_PASSWORD="xxxx-xxxx-xxxx-xxxx"   # app-specific password, NOT your Apple ID password
export APPLE_TEAM_ID="<TEAMID>"
```

Optional — build universal binaries for both arches:

```sh
export TARGETS="aarch64-apple-darwin x86_64-apple-darwin"
```

## Build

```sh
cargo tauri build
```

The flow:

1. `beforeBuildCommand` builds the UI.
2. `beforeBundleCommand` runs `scripts/prepare-sidecar.sh`, which:
   - Builds `contextcli` for each target in `$TARGETS`.
   - Copies the binary to `src-tauri/binaries/contextcli-<triple>` (Tauri's `externalBin` layout).
   - When `APPLE_SIGNING_IDENTITY` is set, signs the sidecar with `--options runtime --timestamp --entitlements Entitlements.plist`. Pre-signing is what avoids the externalBin notarization bug.
3. Tauri bundles the `.app`, signs the outer bundle with the same identity, submits it for notarization, and staples the ticket.
4. The resulting `.dmg` lives in `src-tauri/target/release/bundle/dmg/`.

## Verification checklist

```sh
APP=src-tauri/target/release/bundle/macos/ContextCLI.app

# Outer bundle.
codesign --verify --deep --strict --verbose=2 "$APP"
spctl --assess --type execute --verbose "$APP"

# Sidecar (should also be runtime-hardened).
codesign -dvv "$APP/Contents/MacOS/contextcli-aarch64-apple-darwin"

# DMG staple.
xcrun stapler validate src-tauri/target/release/bundle/dmg/*.dmg
```

Expect `satisfies its Designated Requirement` and `flags=0x10000(runtime)` on the sidecar.

## Dev builds

Unset `APPLE_SIGNING_IDENTITY` (or leave it unset in `.zshenv`) and Tauri builds unsigned. The CLI inside `install_cli` is re-signed ad-hoc at install time, so the downloaded-but-unsigned dev flow still works locally.
