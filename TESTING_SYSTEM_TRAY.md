# System Tray Integration Testing Guide

This document provides comprehensive testing procedures for the system tray integration (SYS-001).

## Prerequisites

1. Build the app: `cargo tauri dev` or `cargo tauri build`
2. Ensure you have API keys configured in `~/.config/localtype/config.toml`
3. macOS 10.15 or later

## Test Cases

### AC1: Launch App → Tray Icon Appears

**Steps:**
1. Launch the app: `cargo tauri dev`
2. Look at the macOS menu bar (top-right corner)

**Expected:**
- ✅ Localtype icon appears in the menu bar
- ✅ App does NOT appear in the Dock (background mode)
- ✅ Icon is visible and properly rendered

**Screenshot location:** `.screenshots/tray-icon-visible.png`

---

### AC2: Click Tray Icon → Dropdown Menu Appears

**Steps:**
1. Click the Localtype icon in the menu bar

**Expected:**
- ✅ Dropdown menu appears with the following items:
  - ⏺ Start Recording
  - ⚙ Open Settings
  - 👁 Show/Hide Overlay
  - ─────────────── (separator)
  - Quit
- ✅ Menu is properly styled
- ✅ All menu items are readable

**Screenshot location:** `.screenshots/tray-menu-idle.png`

---

### AC3: Click "Start Recording" → Recording Begins

**Steps:**
1. Click the tray icon
2. Click "⏺ Start Recording"
3. Speak into the microphone

**Expected:**
- ✅ Recording starts (same as pressing hotkey)
- ✅ Overlay window shows recording state
- ✅ Waveform animation appears
- ✅ Menu item changes to "⏸ Stop Recording"
- ✅ Tray tooltip changes to "Localtype - Recording"
- ✅ Tray icon turns red-tinted

**Screenshot location:** `.screenshots/tray-menu-recording.png`

---

### AC4: Click "Open Settings" → Settings Panel Opens

**Steps:**
1. Click the tray icon
2. Click "⚙ Open Settings"

**Expected:**
- ✅ Overlay window becomes visible (if hidden)
- ✅ Settings panel opens in the overlay
- ✅ All settings options are visible

**Screenshot location:** `.screenshots/tray-open-settings.png`

---

### AC5: Click "Show/Hide Overlay" → Overlay Toggles

**Steps:**
1. Ensure overlay is visible
2. Click tray icon → "👁 Show/Hide Overlay"
3. Verify overlay is hidden
4. Click tray icon → "👁 Show/Hide Overlay" again
5. Verify overlay is visible

**Expected:**
- ✅ First click hides the overlay window
- ✅ Second click shows the overlay window
- ✅ Window focus is set when shown

---

### AC6: Click "Quit" → App Exits Gracefully

**Steps:**
1. Click tray icon
2. Click "Quit"

**Expected:**
- ✅ App exits immediately
- ✅ No error messages
- ✅ Tray icon disappears
- ✅ No background processes remain (check Activity Monitor)

---

### AC7: Tray Icon Changes During Recording

**Steps:**
1. Start recording via hotkey (Cmd+Shift+Space)
2. Observe tray icon
3. Hover over tray icon
4. Click tray icon to see menu

**Expected:**
- ✅ Icon changes to red-tinted version
- ✅ Tooltip shows "Localtype - Recording"
- ✅ Menu shows "⏸ Stop Recording"
- ✅ Icon returns to normal after stopping

**Screenshot locations:**
- `.screenshots/tray-icon-recording.png`
- `.screenshots/tray-icon-idle.png`

---

### AC8: Background Mode - No Dock Icon

**Steps:**
1. Launch the app
2. Check the Dock (bottom of screen)
3. Check Activity Monitor for "Localtype"

**Expected:**
- ✅ No Localtype icon in the Dock
- ✅ App is only accessible via menu bar
- ✅ App shows in Activity Monitor as running
- ✅ macOS activation policy is set to "Accessory"

**Verification:**
Check console logs for: `macOS activation policy set to Accessory (background mode)`

---

## Integration Tests

### Test 1: Full Recording Workflow via Tray

**Steps:**
1. Click tray → "⏺ Start Recording"
2. Speak: "Hello, this is a test"
3. Wait 2 seconds
4. Click tray → "⏸ Stop Recording"
5. Wait for processing

**Expected:**
- ✅ Recording starts and stops
- ✅ Transcription appears in overlay
- ✅ Text is copied to clipboard
- ✅ Tray menu updates correctly throughout

---

### Test 2: Tray Menu vs Hotkey Consistency

**Steps:**
1. Start recording via tray menu
2. Stop recording via hotkey (Cmd+Shift+Space)
3. Start recording via hotkey
4. Stop recording via tray menu

**Expected:**
- ✅ Both methods work interchangeably
- ✅ State is synchronized
- ✅ No conflicts or errors

---

### Test 3: Settings Changes via Tray

**Steps:**
1. Click tray → "⚙ Open Settings"
2. Change STT provider to OpenAI
3. Click tray → "⏺ Start Recording"
4. Record some audio
5. Verify new provider is used

**Expected:**
- ✅ Settings open correctly
- ✅ Changes are applied
- ✅ New provider is used for recording

---

## Performance Tests

### Test 1: Tray Menu Response Time

**Steps:**
1. Click tray icon
2. Measure time until menu appears

**Expected:**
- ✅ Menu appears in < 100ms
- ✅ No lag or stuttering

---

### Test 2: Icon Update Performance

**Steps:**
1. Start and stop recording 10 times rapidly
2. Observe tray icon updates

**Expected:**
- ✅ Icon updates smoothly
- ✅ No flickering
- ✅ No delays

---

## Edge Cases

### Edge Case 1: Multiple Rapid Clicks

**Steps:**
1. Click tray icon rapidly 5 times

**Expected:**
- ✅ Menu opens/closes correctly
- ✅ No crashes or errors
- ✅ App remains responsive

---

### Edge Case 2: Recording While Overlay Hidden

**Steps:**
1. Hide overlay via tray menu
2. Start recording via tray menu
3. Stop recording via tray menu

**Expected:**
- ✅ Recording works without overlay visible
- ✅ Text still copied to clipboard
- ✅ No errors

---

### Edge Case 3: Quit During Recording

**Steps:**
1. Start recording
2. Immediately click tray → "Quit"

**Expected:**
- ✅ App exits gracefully
- ✅ No crash or error messages
- ✅ Recording stops cleanly

---

## Troubleshooting

### Issue: Tray icon not appearing

**Solutions:**
1. Check console logs for errors
2. Verify icon file exists: `crates/lt-tauri/icons/32x32.png`
3. Restart the app
4. Check macOS permissions: System Settings → Privacy & Security

---

### Issue: Menu items not responding

**Solutions:**
1. Check console logs for event handler errors
2. Restart the app
3. Verify Tauri version: `cargo tree -p tauri | head -1`

---

### Issue: Icon not changing during recording

**Solutions:**
1. Check if `rebuild_tray_menu()` is being called
2. Check console logs for image loading errors
3. Verify recording state is being emitted correctly

---

### Issue: App still appears in Dock

**Solutions:**
1. Check console logs for: `macOS activation policy set to Accessory`
2. Verify build is for macOS target
3. Restart macOS (activation policy may require reboot)

---

## Screenshot Checklist

Create screenshots for:

- [ ] Tray icon visible in menu bar (idle state)
- [ ] Tray icon visible in menu bar (recording state)
- [ ] Tray menu with all items (idle)
- [ ] Tray menu with all items (recording)
- [ ] Settings panel opened via tray
- [ ] Overlay toggled via tray

Save all screenshots to `.screenshots/` directory.

---

## Acceptance Criteria Summary

| AC | Description | Status |
|----|-------------|--------|
| 1  | Tray icon appears in menu bar | ⏳ Pending |
| 2  | Menu appears on click | ⏳ Pending |
| 3  | Start Recording works | ⏳ Pending |
| 4  | Open Settings works | ⏳ Pending |
| 5  | Show/Hide Overlay works | ⏳ Pending |
| 6  | Quit works gracefully | ⏳ Pending |
| 7  | Icon changes during recording | ⏳ Pending |
| 8  | Background mode (no Dock icon) | ⏳ Pending |

---

## Notes

- System tray is a macOS-specific feature in this implementation
- Tray icon uses `icons/32x32.png` from the app bundle
- Recording state icon is dynamically generated (red-tinted)
- Background mode uses `ActivationPolicy::Accessory`
- Menu uses Unicode symbols: ⏺ (record), ⏸ (stop), ⚙ (settings), 👁 (overlay)

---

## Quick Test Command

```bash
# Run in development mode
cargo tauri dev

# Build for production
cargo tauri build
```

After running, verify all 8 acceptance criteria manually using the test cases above.
