# Murmur Distribution Guide

This document covers the build, packaging, and distribution process for Murmur on macOS.

## Build Configuration

### Bundle Settings (tauri.conf.json)

```json
{
  "productName": "Murmur",
  "identifier": "com.hydai.murmur",
  "bundle": {
    "active": true,
    "targets": "all",
    "publisher": "Murmur",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "macOS": {
      "minimumSystemVersion": "10.15",
      "infoPlist": "Info.plist"
    }
  }
}
```

### Required Features (Cargo.toml)

```toml
[dependencies]
tauri = {
  version = "2.10.2",
  features = [
    "tray-icon",      # System tray integration
    "image-png",      # PNG icon support
    "macos-private-api" # Transparent windows (not App Store compatible)
  ]
}
```

### Info.plist Privacy Descriptions

The Info.plist includes required privacy descriptions for macOS permissions:

```xml
<key>NSMicrophoneUsageDescription</key>
<string>Murmur needs microphone access for voice-to-text transcription</string>

<key>NSAccessibilityUsageDescription</key>
<string>Murmur needs Accessibility permission to simulate keyboard input for typing transcribed text</string>
```

## Build Process

### Production Build

```bash
# From project root
cargo tauri build
```

**Build Time**: ~5-10 minutes on Apple Silicon

**Build Steps**:
1. Frontend compilation (Svelte → optimized bundle)
2. Rust compilation (release mode with optimizations)
3. Binary stripping and optimization
4. .app bundle creation
5. Info.plist injection
6. Icon embedding
7. .dmg packaging

### Build Output

```
target/release/bundle/
├── macos/
│   └── Murmur.app/           # Standalone .app bundle
│       └── Contents/
│           ├── Info.plist       # Bundle metadata + privacy descriptions
│           ├── MacOS/
│           │   └── lt-tauri     # Main binary (20MB)
│           └── Resources/
│               └── icon.icns    # App icon
└── dmg/
    └── Murmur_0.1.0_aarch64.dmg  # Installer (7.7MB)
```

## DMG Contents

When mounted, the .dmg shows:
- Murmur.app (the application)
- Applications symlink (for drag-and-drop installation)
- Volume icon (.VolumeIcon.icns)

## Installation Flow

### End User Installation

1. **Download**: User downloads `Murmur_0.1.0_aarch64.dmg`
2. **Mount**: Double-click the .dmg to mount the disk image
3. **Install**: Drag Murmur.app to the Applications folder
4. **Launch**: Open from Applications or Spotlight
5. **First Launch**:
   - System prompts for microphone permission
   - System prompts for accessibility permission
   - App opens with overlay window
   - User configures API keys via Settings (system tray → Settings)

### Permission Prompts

On first launch, macOS will show system permission dialogs:

**Microphone**:
> "Murmur" would like to access the microphone.
> Murmur needs microphone access for voice-to-text transcription

**Accessibility**:
> "Murmur" would like to control this computer using accessibility features.
> Murmur needs Accessibility permission to simulate keyboard input for typing transcribed text

If denied, the app provides UI guidance to open System Settings → Privacy & Security.

## Verification Checklist

After building, verify the following:

### AC1: Build Success
- [ ] `cargo tauri build` completes without errors
- [ ] .dmg file exists at `target/release/bundle/dmg/Murmur_0.1.0_aarch64.dmg`
- [ ] .app bundle exists at `target/release/bundle/macos/Murmur.app`

### AC2: DMG Contents
- [ ] Mount the .dmg: `hdiutil attach Murmur_0.1.0_aarch64.dmg -readonly`
- [ ] Verify Murmur.app is present
- [ ] Verify Applications symlink exists
- [ ] Verify volume has icon
- [ ] Unmount: `hdiutil detach /Volumes/Murmur`

### AC3: App Bundle Structure
- [ ] Info.plist exists: `ls target/release/bundle/macos/Murmur.app/Contents/Info.plist`
- [ ] Binary exists: `ls target/release/bundle/macos/Murmur.app/Contents/MacOS/lt-tauri`
- [ ] Icon exists: `ls target/release/bundle/macos/Murmur.app/Contents/Resources/icon.icns`

### AC4: Info.plist Privacy Descriptions
```bash
grep "NSMicrophoneUsageDescription" target/release/bundle/macos/Murmur.app/Contents/Info.plist
grep "NSAccessibilityUsageDescription" target/release/bundle/macos/Murmur.app/Contents/Info.plist
```

### AC5: File Sizes
- [ ] .dmg size < 50MB (actual: ~7.7MB)
- [ ] .app size < 50MB (actual: ~20MB)
- [ ] Binary size reasonable (~20MB for Rust + Tauri + embedded frontend)

## Bundle Verification Commands

```bash
# Check bundle identifier
defaults read target/release/bundle/macos/Murmur.app/Contents/Info.plist CFBundleIdentifier

# Check bundle version
defaults read target/release/bundle/macos/Murmur.app/Contents/Info.plist CFBundleShortVersionString

# Check minimum macOS version
defaults read target/release/bundle/macos/Murmur.app/Contents/Info.plist LSMinimumSystemVersion

# Verify code signature (if signed)
codesign -dv target/release/bundle/macos/Murmur.app

# Check for malformed Info.plist
plutil -lint target/release/bundle/macos/Murmur.app/Contents/Info.plist
```

## Full Workflow Test (Manual)

This requires a clean macOS environment or permission reset:

```bash
# Reset permissions (requires root)
tccutil reset Microphone com.hydai.murmur
tccutil reset Accessibility com.hydai.murmur

# Install from .dmg
hdiutil attach target/release/bundle/dmg/Murmur_0.1.0_aarch64.dmg
cp -R /Volumes/Murmur/Murmur.app /Applications/
hdiutil detach /Volumes/Murmur

# Launch
open /Applications/Murmur.app

# Test workflow:
# 1. Grant microphone permission when prompted
# 2. Grant accessibility permission when prompted
# 3. Click system tray icon → Settings
# 4. Configure API key (e.g., ElevenLabs)
# 5. Press Cmd+Shift+Space
# 6. Speak into microphone
# 7. Verify transcription appears in overlay
# 8. Verify text appears in clipboard (Cmd+V)
# 9. Test voice command: "translate to Chinese: hello world"
# 10. Verify translation in clipboard

# Quit cleanly
# Click system tray icon → Quit
```

## Troubleshooting

### Build Fails with "Bundle identifier ends with .app"

This is a warning, not an error. The identifier `com.hydai.murmur` is functional but not recommended. To fix:

```json
// In tauri.conf.json
"identifier": "com.hydai.murmur"  // or any other valid identifier
```

### Icons Not Showing

Verify icon files exist:
```bash
ls -l crates/lt-tauri/icons/
# Should show: 32x32.png, 128x128.png, 128x128@2x.png, icon.icns, icon.ico
```

### Info.plist Not Included

Check tauri.conf.json has:
```json
"macOS": {
  "infoPlist": "Info.plist"
}
```

And the file exists:
```bash
ls crates/lt-tauri/Info.plist
```

### DMG Not Created

Check build output for errors. Common issues:
- hdiutil not found (macOS only)
- Insufficient disk space
- Permissions issues in target/ directory

### App Won't Launch

Check for:
- Missing permissions (microphone, accessibility)
- Corrupted bundle (rebuild)
- macOS security blocking unsigned apps (Right-click → Open)

## Size Optimization

Current sizes are well within limits:
- DMG: 7.7MB (< 50MB target)
- App: 20MB (< 50MB target)

Further optimization possible:
- Strip debug symbols (already done in release mode)
- Compress frontend assets (already done by Vite)
- Use UPX for binary compression (not recommended for macOS)

## Code Signing & Notarization

For distribution outside the App Store, you should:

1. **Get Apple Developer ID**: Enroll in Apple Developer Program ($99/year)

2. **Create signing certificate**: In Xcode or via Developer portal

3. **Sign the app**:
```bash
codesign --force --options runtime --sign "Developer ID Application: Your Name" \
  target/release/bundle/macos/Murmur.app
```

4. **Notarize**:
```bash
# Create .zip for notarization
ditto -c -k --keepParent target/release/bundle/macos/Murmur.app Murmur.zip

# Submit to Apple
xcrun notarytool submit Murmur.zip --apple-id your@email.com --team-id TEAMID --password APP_SPECIFIC_PASSWORD --wait

# Staple the notarization ticket
xcrun stapler staple target/release/bundle/macos/Murmur.app
```

5. **Re-package DMG** with signed app

**Note**: Code signing is optional for personal use but required for wide distribution to avoid macOS Gatekeeper warnings.

## Distribution Channels

### Direct Download
- Host the .dmg on your website or GitHub Releases
- Provide SHA256 checksum for verification

### GitHub Releases
```bash
# Create release with gh cli
gh release create v0.1.0 \
  target/release/bundle/dmg/Murmur_0.1.0_aarch64.dmg \
  --title "Murmur v0.1.0" \
  --notes "Initial release"
```

### Homebrew Cask
Create a Homebrew cask for easier installation:

```ruby
cask "murmur" do
  version "0.1.0"
  sha256 "..."

  url "https://github.com/hydai/murmur/releases/download/v#{version}/Murmur_#{version}_aarch64.dmg"
  name "Murmur"
  desc "Privacy-first BYOK voice typing app"
  homepage "https://github.com/hydai/murmur"

  app "Murmur.app"
end
```

## Release Checklist

Before distributing a new version:

- [ ] All tests pass (`cargo test --workspace`)
- [ ] Version updated in Cargo.toml and tauri.conf.json
- [ ] CHANGELOG.md updated
- [ ] README.md updated
- [ ] Build completes successfully
- [ ] Manual smoke test on clean system
- [ ] DMG mounted and verified
- [ ] Permissions prompts appear correctly
- [ ] Full workflow tested (record → transcribe → post-process → output)
- [ ] Code signed (if distributing widely)
- [ ] Notarized (if distributing widely)
- [ ] Release notes prepared
- [ ] GitHub release created with .dmg attachment
- [ ] Documentation updated

## Support

For build issues, check:
1. Rust version: `rustc --version` (requires 1.92+)
2. Node version: `node --version` (requires 22+)
3. Tauri CLI: `cargo tauri --version` (should be 2.x)
4. macOS version: `sw_vers` (requires 10.15+)
5. Xcode Command Line Tools: `xcode-select --install`

Build artifacts are in `target/release/bundle/` directory. Clean builds with `cargo clean` if you encounter issues.
