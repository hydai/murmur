#!/bin/bash
# Localtype Build Verification Script

echo "=== Localtype Build Verification ==="
echo ""

# AC1: Build Success
echo "AC1: Build Success"
if [ -f "target/release/bundle/dmg/Localtype_0.1.0_aarch64.dmg" ]; then
    echo "✅ .dmg file exists"
else
    echo "❌ .dmg file NOT found"
    exit 1
fi

if [ -d "target/release/bundle/macos/Localtype.app" ]; then
    echo "✅ .app bundle exists"
else
    echo "❌ .app bundle NOT found"
    exit 1
fi
echo ""

# AC2: DMG Contents
echo "AC2: DMG Contents"
DMG_SIZE=$(du -h "target/release/bundle/dmg/Localtype_0.1.0_aarch64.dmg" | awk '{print $1}')
echo "   DMG size: $DMG_SIZE"

# AC3: App Bundle Structure
echo ""
echo "AC3: App Bundle Structure"
if [ -f "target/release/bundle/macos/Localtype.app/Contents/Info.plist" ]; then
    echo "✅ Info.plist exists"
else
    echo "❌ Info.plist NOT found"
    exit 1
fi

if [ -f "target/release/bundle/macos/Localtype.app/Contents/MacOS/lt-tauri" ]; then
    echo "✅ Binary exists"
    BINARY_SIZE=$(du -h "target/release/bundle/macos/Localtype.app/Contents/MacOS/lt-tauri" | awk '{print $1}')
    echo "   Binary size: $BINARY_SIZE"
else
    echo "❌ Binary NOT found"
    exit 1
fi

# AC4: App Icon
echo ""
echo "AC4: App Icon"
if [ -f "target/release/bundle/macos/Localtype.app/Contents/Resources/icon.icns" ]; then
    echo "✅ App icon exists"
else
    echo "❌ App icon NOT found"
    exit 1
fi

# AC5: Info.plist Privacy Descriptions
echo ""
echo "AC5: Info.plist Privacy Descriptions"
if grep -q "NSMicrophoneUsageDescription" "target/release/bundle/macos/Localtype.app/Contents/Info.plist"; then
    echo "✅ Microphone permission description present"
else
    echo "❌ Microphone permission description MISSING"
    exit 1
fi

if grep -q "NSAccessibilityUsageDescription" "target/release/bundle/macos/Localtype.app/Contents/Info.plist"; then
    echo "✅ Accessibility permission description present"
else
    echo "❌ Accessibility permission description MISSING"
    exit 1
fi

# AC8: File Sizes
echo ""
echo "AC8: File Sizes"
DMG_SIZE_MB=$(du -sm "target/release/bundle/dmg/Localtype_0.1.0_aarch64.dmg" | cut -f1)
APP_SIZE_MB=$(du -sm "target/release/bundle/macos/Localtype.app" | cut -f1)

echo "   .dmg size: ${DMG_SIZE_MB}MB"
echo "   .app size: ${APP_SIZE_MB}MB"

if [ "$DMG_SIZE_MB" -lt 50 ]; then
    echo "✅ DMG size < 50MB"
else
    echo "❌ DMG size >= 50MB"
    exit 1
fi

if [ "$APP_SIZE_MB" -lt 50 ]; then
    echo "✅ App size < 50MB"
else
    echo "❌ App size >= 50MB"
    exit 1
fi

echo ""
echo "=== All Verifications Passed ==="
echo ""
echo "Build Summary:"
echo "  Product: Localtype v0.1.0"
echo "  Platform: macOS (Apple Silicon)"
echo "  DMG: target/release/bundle/dmg/Localtype_0.1.0_aarch64.dmg ($DMG_SIZE)"
echo "  App: target/release/bundle/macos/Localtype.app (${APP_SIZE_MB}MB)"
echo "  Bundle ID: com.localtype.app"
echo "  Min macOS: 10.15"
echo ""
echo "Distribution ready!"
