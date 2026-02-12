# UI-001: Polished Overlay UI - Testing Guide

## Overview

This document provides comprehensive testing procedures for Task #11 (UI-001): Polished overlay UI with animations and waveform.

## Prerequisites

1. **Start the development server**:
   ```bash
   cd /Users/hydai/workspace/vibe/localtype
   cargo tauri dev
   ```

2. **Wait for app to launch**: The floating overlay window should appear on your screen

3. **Ensure microphone permissions**: macOS will request microphone access on first run

## Test Procedures

### AC1: Glassmorphism Design (Rounded corners + semi-transparent background)

**Objective**: Verify the overlay has polished glassmorphism styling consistent with macOS design

**Steps**:
1. Launch the app with `cargo tauri dev`
2. Observe the floating overlay window

**Expected Results**:
- ✅ Rounded corners (16px border-radius) visible
- ✅ Semi-transparent dark background (rgba(28, 28, 30, 0.88))
- ✅ Backdrop blur effect visible through the overlay
- ✅ Subtle border with slight glow (rgba(255, 255, 255, 0.12))
- ✅ Inset highlight on top edge for depth
- ✅ Multi-layered shadow for elevation
- ✅ Looks native on macOS

**Verification Method**: Visual inspection
**Status**: ⬜ Pass ⬜ Fail

---

### AC2: Drag-to-Reposition (Overlay stays in new position)

**Objective**: Verify the overlay can be dragged to a new screen position and stays there

**Steps**:
1. Click and hold on the overlay window (anywhere on the main content)
2. Drag to a different position on screen
3. Release mouse button
4. Try dragging to multiple positions (top-left, center, bottom-right)

**Expected Results**:
- ✅ Cursor changes to indicate draggable area
- ✅ Overlay follows mouse movement smoothly
- ✅ Overlay stays in new position after release
- ✅ No lag or stuttering during drag
- ✅ Drag works from any non-interactive area

**Verification Method**: Manual interaction
**Status**: ⬜ Pass ⬜ Fail

---

### AC3: Smooth Waveform Animation (Responds to voice)

**Objective**: Verify waveform indicator animates smoothly in response to audio input

**Steps**:
1. Click "Start Recording" button (or press Cmd+Shift+Space)
2. Observe the waveform indicator appears
3. Speak normally into the microphone
4. Vary volume: whisper, normal speech, loud speech
5. Stop speaking and observe the waveform settle

**Expected Results**:
- ✅ Waveform appears with smooth fade-in (200ms)
- ✅ 24 vertical bars displayed
- ✅ Bars animate smoothly (not jumpy or flickering)
- ✅ Bar heights increase with volume
- ✅ Bars show green gradient when voice is active
- ✅ Bars show gray when no voice detected
- ✅ Subtle glow effect on active bars
- ✅ Smooth transitions between states (0.2s cubic-bezier)
- ✅ Wave pattern creates organic movement

**Verification Method**: Record audio and observe visual feedback
**Status**: ⬜ Pass ⬜ Fail

---

### AC4: Smooth State Transitions (Idle → Recording → Transcribing → Processing → Done)

**Objective**: Verify all state transitions have smooth animations

**Steps**:
1. **Idle → Recording**:
   - Start from idle state (overlay shows "Ready")
   - Click "Start Recording" or press hotkey
   - Observe status dot and text transition

2. **Recording → Transcribing**:
   - While recording, start speaking
   - Observe status change when transcription text appears

3. **Transcribing → Processing**:
   - Stop recording (stop speaking or click "Stop Recording")
   - Observe status change to "Processing..."

4. **Processing → Done**:
   - Wait for LLM processing to complete
   - Observe status change to "Done"

**Expected Results**:
- ✅ Status dot fades smoothly between colors (150ms fade)
- ✅ Status text transitions without jarring changes
- ✅ Colors: Green (ready) → Red (recording) → Blue (transcribing) → Purple (processing) → Green (done)
- ✅ No layout jumps or flickering
- ✅ Smooth fade/slide animations for content changes
- ✅ Overlay auto-resizes smoothly (0.3s cubic-bezier)

**Verification Method**: Complete full recording cycle
**Status**: ⬜ Pass ⬜ Fail

---

### AC5: Transcription Text Reveal Animation

**Objective**: Verify transcription text appears with smooth animation

**Steps**:
1. Start recording
2. Speak clearly: "This is a test of the transcription system"
3. Observe how text appears in the transcription box
4. Continue speaking to add more text

**Expected Results**:
- ✅ Committed text appears with fly-in animation (y: 5px, 300ms)
- ✅ Partial text fades in smoothly (200ms fade)
- ✅ Partial text has pulsing animation to indicate it's temporary
- ✅ Text is readable (14px, line-height 1.65)
- ✅ Proper contrast (white text on dark background)
- ✅ No text clipping or overlap

**Verification Method**: Record speech and observe text appearance
**Status**: ⬜ Pass ⬜ Fail

---

### AC6: Auto-sizing Based on Content Length

**Objective**: Verify overlay resizes based on transcription text length

**Steps**:
1. **Short text test**:
   - Record: "Hello"
   - Observe overlay size

2. **Medium text test**:
   - Record: "This is a medium length sentence to test the overlay sizing"
   - Observe overlay expands

3. **Long text test**:
   - Record multiple sentences (150+ characters)
   - Observe overlay expands further

4. **Empty state test**:
   - Return to idle (no text)
   - Observe overlay returns to compact size

**Expected Results**:
- ✅ Compact mode: 380px min-width, 18px padding (idle/no text)
- ✅ Expanded mode: 500px min-width, 24px padding (recording/has text)
- ✅ Transcription container height adjusts: 60px → 70px → 100px → 150px → 200px max
- ✅ Smooth resize animation (0.3s cubic-bezier)
- ✅ Max width: 600px (prevents excessive width)
- ✅ Content never overflows or gets cut off

**Verification Method**: Test with varying text lengths
**Status**: ⬜ Pass ⬜ Fail

---

### AC7: Done State + Auto-fade/Dismiss

**Objective**: Verify "done" state shows result clearly and can be dismissed

**Steps**:
1. Complete a full recording cycle (record → transcribe → process)
2. Observe the "Done" state
3. Look for "Copied to clipboard!" indicator
4. Wait to see if it auto-fades
5. Click the close button (✕) if visible

**Expected Results**:
- ✅ "Copied to clipboard!" message appears with fly-in animation (y: -10px, 300ms)
- ✅ Message has green background and glow effect
- ✅ Message auto-hides after 2 seconds
- ✅ Close button (✕) appears in header with fly-in animation (x: 10px, 200ms)
- ✅ Close button is red-tinted and clear
- ✅ Clicking close button dismisses overlay content smoothly
- ✅ Overlay fades out gracefully (300ms)
- ✅ Overlay resets and fades back in (ready for next recording)

**Verification Method**: Complete recording and observe done state
**Status**: ⬜ Pass ⬜ Fail

---

### AC8: macOS-native Polished Look

**Objective**: Verify overlay looks polished and native on macOS

**Checklist**:
- ✅ Uses SF Pro Text font (macOS system font)
- ✅ Follows macOS blur/vibrancy patterns
- ✅ Color palette matches macOS dark mode
- ✅ Border radius consistent with macOS (16px for windows)
- ✅ Shadows have multiple layers for depth
- ✅ Animations use native-feeling easing (cubic-bezier)
- ✅ Buttons have macOS-style hover states
- ✅ No Windows/Linux UI elements visible
- ✅ Looks like it could be a built-in macOS app

**Verification Method**: Visual comparison with native macOS apps
**Status**: ⬜ Pass ⬜ Fail

---

### AC9: Clear Control Bar

**Objective**: Verify control bar shows clear controls with proper states

**Steps**:
1. **Idle state controls**:
   - Observe buttons in idle state
   - Hover over settings button
   - Hover over record button

2. **Recording state controls**:
   - Start recording
   - Observe record button changes to "Stop Recording"
   - Button changes to red color scheme

3. **Done state controls**:
   - Complete recording
   - Observe close button appears
   - Hover over close button

**Expected Results**:
- ✅ Settings button (⚙) visible in top-right
- ✅ Settings button has subtle background/border
- ✅ Settings button hover effect: brightens, scales 1.05x
- ✅ Record button has clear text: "⏺ Start Recording" / "⏸ Stop Recording"
- ✅ Record button color: green (start) / red (stop)
- ✅ Record button hover effect: brightens, lifts slightly
- ✅ Close button (✕) appears only when done
- ✅ Close button hover effect: red glow
- ✅ Status text is clear and readable
- ✅ All buttons have smooth transitions (0.15s-0.2s)

**Verification Method**: Test all button interactions
**Status**: ⬜ Pass ⬜ Fail

---

### AC10: Responsive Interactions (< 100ms)

**Objective**: Verify all UI interactions feel responsive with immediate feedback

**Test Matrix**:

| Interaction | Expected Response Time | Visual Feedback |
|-------------|------------------------|-----------------|
| Button hover | < 50ms | Background brightens, scale change |
| Button click | < 50ms | Scale down, immediate action |
| Start recording | < 100ms | Status dot changes, waveform appears |
| Stop recording | < 100ms | Status updates, waveform fades |
| Drag start | < 50ms | Cursor changes, window follows |
| Text appear | < 100ms | Fade/fly animation begins |
| State change | < 100ms | Status dot/text transition |

**Steps**:
1. Perform each interaction in the table above
2. Observe if visual feedback is immediate
3. Check if any interactions feel sluggish

**Expected Results**:
- ✅ All hover states respond instantly (< 50ms)
- ✅ Button clicks provide immediate visual feedback
- ✅ Recording state changes feel instant
- ✅ No perceived lag in animations
- ✅ Drag movement is smooth and immediate
- ✅ No frame drops during transitions

**Verification Method**: Rapid interaction testing
**Status**: ⬜ Pass ⬜ Fail

---

## Integration Tests

### Full Workflow Test

**Objective**: Verify complete user journey with all features

**Scenario**: User wants to transcribe a voice note

**Steps**:
1. Launch app → Overlay appears in ready state
2. Drag overlay to preferred position
3. Press Cmd+Shift+Space (or click Start Recording)
4. Speak for 5-10 seconds
5. Press Cmd+Shift+Space again (or click Stop Recording)
6. Wait for processing
7. Observe result and "Copied!" message
8. Click close button to dismiss
9. Repeat with different text lengths

**Expected Results**:
- ✅ Entire flow feels smooth and polished
- ✅ No jarring transitions or layout jumps
- ✅ All animations complete properly
- ✅ Text is readable throughout
- ✅ Result is copied to clipboard
- ✅ Can immediately start new recording

**Status**: ⬜ Pass ⬜ Fail

---

## Visual Regression Checklist

Compare current implementation with previous version:

- ✅ Improved glassmorphism (more blur, better opacity)
- ✅ Smoother waveform (24 bars vs 20, better transitions)
- ✅ Auto-sizing (compact ↔ expanded)
- ✅ Better typography (gradient title, improved contrast)
- ✅ Enhanced animations (fade, fly, slide transitions)
- ✅ Close button in done state
- ✅ Improved button styling (better hover states)
- ✅ Refined color palette (greens, reds, purples)

---

## Performance Checks

### Animation Performance

**Test**: Monitor performance during heavy animation

**Steps**:
1. Open Chrome DevTools (if using Tauri's webview inspector)
2. Go to Performance tab
3. Start recording performance
4. Perform full recording cycle with long text
5. Stop performance recording
6. Analyze frame rate

**Expected Results**:
- ✅ Maintains 60fps during animations
- ✅ No dropped frames during transitions
- ✅ Smooth waveform animation (no stuttering)
- ✅ CPU usage reasonable (< 20% during idle)

**Status**: ⬜ Pass ⬜ Fail

---

## Browser Compatibility

**Target**: Tauri uses system WebView (Safari WebKit on macOS)

**Checklist**:
- ✅ backdrop-filter works correctly
- ✅ CSS transitions smooth
- ✅ Svelte transitions render properly
- ✅ Flexbox layout correct
- ✅ Gradient text works (-webkit-background-clip)
- ✅ No console errors

---

## Accessibility Notes

While full accessibility testing is beyond this task's scope, note:
- Keyboard navigation works (tab, enter, escape)
- Screen reader support may need future enhancement
- Color contrast meets minimum standards (AA)

---

## Known Limitations

1. **No persistence of overlay position**: Position resets on app restart
2. **Waveform randomness**: Uses Math.random() for organic feel, not deterministic
3. **Auto-fade timing**: Fixed 2-second delay for "Copied!" message
4. **Max width**: Overlay won't expand beyond 600px

---

## Troubleshooting

### Issue: Overlay doesn't appear
- **Solution**: Check if window is minimized or off-screen. Restart app.

### Issue: Waveform not animating
- **Solution**: Check microphone permissions. Verify audio input working.

### Issue: Animations stuttering
- **Solution**: Check CPU usage. Close other apps. Reduce backdrop blur if needed.

### Issue: Blur effect not visible
- **Solution**: Ensure running on macOS. Backdrop-filter requires WebKit support.

---

## Screenshot Locations

Take screenshots during testing and save to:
```
/Users/hydai/workspace/vibe/localtype/.screenshots/
```

Recommended screenshots:
1. `ui-001-idle-state.png` - Overlay in ready state
2. `ui-001-recording-waveform.png` - Waveform during recording
3. `ui-001-transcription-active.png` - Text appearing
4. `ui-001-processing.png` - Processing state
5. `ui-001-done-copied.png` - Done state with "Copied!" message
6. `ui-001-auto-sizing-comparison.png` - Side-by-side short vs long text

---

## Summary Checklist

Before marking task complete, verify:

- [ ] AC1: Glassmorphism design ✓
- [ ] AC2: Drag-to-reposition ✓
- [ ] AC3: Smooth waveform ✓
- [ ] AC4: State transitions ✓
- [ ] AC5: Text reveal animation ✓
- [ ] AC6: Auto-sizing ✓
- [ ] AC7: Done state + dismiss ✓
- [ ] AC8: macOS-native look ✓
- [ ] AC9: Clear control bar ✓
- [ ] AC10: Responsive interactions ✓
- [ ] Full workflow test ✓
- [ ] Performance acceptable ✓
- [ ] No console errors ✓
- [ ] Screenshots captured ✓

---

## Manual Testing Results

**Date**: _______________
**Tester**: _______________
**Environment**: macOS _______

### Test Summary

- Total Acceptance Criteria: 10
- Passed: ____
- Failed: ____
- Notes: _______________________________________________

### Issues Found

1. _____________________________________________________
2. _____________________________________________________
3. _____________________________________________________

### Overall Assessment

⬜ Ready for production
⬜ Minor issues (acceptable)
⬜ Major issues (needs fixes)

---

## Next Steps

After successful manual testing:
1. Capture required screenshots
2. Run `lineguard` on modified files
3. Commit changes with conventional commit message
4. Update `.autonoe-note.md` with results
5. Mark task complete
