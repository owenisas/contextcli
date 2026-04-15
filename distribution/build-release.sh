#!/usr/bin/env bash
set -euo pipefail

# ContextCLI Release Build Script
# Builds CLI + GUI for macOS (arm64)

VERSION="${1:-0.1.0}"
DIST_DIR="dist/v${VERSION}"
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "=== ContextCLI v${VERSION} Release Build ==="
cd "$PROJECT_ROOT"

mkdir -p "$DIST_DIR"

# ── CLI ──────────────────────────────────────────────────
echo ""
echo "→ Building CLI (release)..."
cargo build --release -p contextcli

cp target/release/contextcli "$DIST_DIR/contextcli"
codesign --force --sign - --identifier "com.contextcli.cli" "$DIST_DIR/contextcli"

echo "  ✓ CLI: $DIST_DIR/contextcli ($(du -h "$DIST_DIR/contextcli" | cut -f1))"

# ── GUI ──────────────────────────────────────────────────
echo ""
echo "→ Building frontend..."
cd ui
pnpm install --frozen-lockfile 2>/dev/null || pnpm install
pnpm build
cd "$PROJECT_ROOT"

echo "→ Building GUI (release)..."
cargo build --release -p contextcli-gui

# Create .app bundle
APP_DIR="$DIST_DIR/ContextCLI.app/Contents"
mkdir -p "$APP_DIR/MacOS" "$APP_DIR/Resources"

cp target/release/contextcli-gui "$APP_DIR/MacOS/ContextCLI"

# Info.plist
cat > "$APP_DIR/Info.plist" << PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>ContextCLI</string>
    <key>CFBundleDisplayName</key>
    <string>ContextCLI</string>
    <key>CFBundleIdentifier</key>
    <string>com.contextcli.app</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleExecutable</key>
    <string>ContextCLI</string>
    <key>CFBundleIconFile</key>
    <string>icon</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
PLIST

# Copy icon
cp src-tauri/icons/icon.png "$APP_DIR/Resources/icon.png"

# Embed CLI binary inside the .app bundle
cp target/release/contextcli "$APP_DIR/Resources/contextcli"
echo "  ✓ CLI embedded in .app at Contents/Resources/contextcli"

# Codesign the .app bundle
codesign --force --deep --sign - "$DIST_DIR/ContextCLI.app"

echo "  ✓ GUI: $DIST_DIR/ContextCLI.app"

# ── DMG (optional, requires create-dmg) ──────────────────
if command -v create-dmg &>/dev/null; then
    echo ""
    echo "→ Creating DMG..."
    create-dmg \
        --volname "ContextCLI" \
        --window-size 500 300 \
        --icon "ContextCLI.app" 150 150 \
        --app-drop-link 350 150 \
        "$DIST_DIR/ContextCLI-v${VERSION}.dmg" \
        "$DIST_DIR/ContextCLI.app" 2>/dev/null || echo "  ⚠ DMG creation failed (install create-dmg)"
fi

# ── Summary ──────────────────────────────────────────────
echo ""
echo "=== Build Complete ==="
echo ""
ls -lh "$DIST_DIR/"
echo ""
echo "CLI:  $DIST_DIR/contextcli"
echo "GUI:  $DIST_DIR/ContextCLI.app"
echo ""
echo "Install CLI:  cp $DIST_DIR/contextcli /usr/local/bin/"
echo "Install GUI:  cp -r $DIST_DIR/ContextCLI.app /Applications/"
