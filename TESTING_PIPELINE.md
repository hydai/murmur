# Testing Guide: Pipeline Orchestration (OUT-001)

This document provides comprehensive testing procedures for the pipeline orchestration feature.

## Overview

OUT-001 implements:
- `lt-output` crate: Clipboard + keyboard simulation output
- `lt-pipeline` crate: Full pipeline orchestration (audio → STT → LLM → output)
- Unified pipeline integration in Tauri app
- Frontend pipeline status display

## Prerequisites

1. **gemini-cli installed** and in PATH:
   ```bash
   gemini --version
   ```
   Install from: https://github.com/google/generative-ai-cli

2. **ElevenLabs API key** configured:
   ```bash
   ~/.config/localtype/config.toml should contain:
   [api_keys]
   elevenlabs = "your-key-here"
   ```

3. **Microphone permission** granted to the app

4. **Test environment**:
   - macOS (for keyboard simulation)
   - Active internet connection
   - Text editor for paste testing

## Acceptance Criteria Testing

### AC1: Start recording with global hotkey
**Steps**:
1. Run `cargo tauri dev`
2. Wait for app window to appear
3. Press `Cmd+Shift+Space`

**Expected**:
- Overlay shows "Recording" status with red pulsing indicator
- Waveform visualizes audio input
- No errors in console

### AC2: Speak a sentence with filler words
**Steps**:
1. After starting recording, speak clearly into microphone:
   "um, I think we should, like, schedule the meeting for tomorrow"
2. Observe the overlay

**Expected**:
- Partial transcription appears in real-time
- Status transitions from "Recording" to "Transcribing" (blue pulsing)
- Committed transcription accumulates correctly

### AC3: Stop recording
**Steps**:
1. Press `Cmd+Shift+Space` again to stop

**Expected**:
- Status transitions: Transcribing → Processing (purple pulsing)
- Backend logs show "Transcription complete, starting LLM post-processing"
- No crashes or hangs

### AC4: Pipeline status progression
**Expected flow**:
1. Ready (green) → idle state
2. Recording (red pulsing) → capturing audio
3. Transcribing (blue pulsing) → STT in progress
4. Processing (purple pulsing) → LLM cleaning text
5. Done (green solid) → completed successfully

**Verify in UI**:
- Each status shows correct color and animation
- Status text matches the state
- Smooth transitions without flicker

### AC5: Cleaned text automatically copied to clipboard
**Steps**:
1. After "Done" status appears, open any text editor
2. Press `Cmd+V` to paste

**Expected**:
- Text is pasted from clipboard
- Filler words removed: "I think we should schedule the meeting for tomorrow"
- Grammar corrected if needed
- No "um", "like", "uh" present

### AC6: Final text displayed before fade
**Expected**:
- Overlay shows the final cleaned text
- Text is readable and correctly formatted
- User can review before it fades or is dismissed

### AC7: Multiple recordings in sequence
**Steps**:
1. Press `Cmd+Shift+Space`, speak: "This is the first test"
2. Stop recording, wait for clipboard copy
3. Verify clipboard: paste in editor
4. Press `Cmd+Shift+Space`, speak: "This is the second test"
5. Stop recording, wait for clipboard copy
6. Verify clipboard: paste should show second text

**Expected**:
- Each recording is independently processed
- Clipboard is updated with each result
- No interference between sessions
- Pipeline resets to idle after each completion

### AC8: Full flow without manual intervention
**Expected**:
- After pressing stop, no manual steps needed
- Pipeline automatically:
  - Completes transcription
  - Processes with LLM
  - Copies to clipboard
  - Shows final result
- User only needs to speak and paste

### AC9: Output sink verification
**Test clipboard output**:
1. Complete a recording session
2. Check clipboard contains processed text
3. Paste in multiple applications (TextEdit, Notes, Terminal)

**Expected**:
- Text pastes consistently across apps
- No clipboard corruption
- Text encoding is correct (UTF-8)

**Note**: Keyboard simulation testing requires:
- macOS Accessibility permissions
- Manual verification (automated testing not feasible)

## Error Scenarios

### E1: LLM processing failure
**Steps**:
1. Stop gemini-cli: `killall gemini` (if running as service)
2. Or temporarily rename gemini binary
3. Complete a recording

**Expected**:
- Error message displayed
- Raw transcription falls back to clipboard
- Status shows "Error" state
- No crash

### E2: Network interruption during STT
**Steps**:
1. Disable network mid-recording
2. Complete recording

**Expected**:
- STT error displayed
- Pipeline gracefully handles failure
- Can retry new recording after re-enabling network

### E3: Missing API key
**Steps**:
1. Remove ElevenLabs API key from config
2. Try to start recording

**Expected**:
- Clear error message: "API key not configured"
- Guidance on where to add key
- App doesn't crash

## Performance Expectations

| Metric | Target | Notes |
|--------|--------|-------|
| Transcription latency | < 500ms | Partial results should appear quickly |
| LLM processing time | 1-5s | Depends on text length and network |
| Clipboard copy time | < 100ms | Should be instant |
| Status update latency | < 200ms | UI should feel responsive |
| Pipeline reset time | < 500ms | Ready for next recording |

## Backend Log Verification

Enable debug logs:
```bash
RUST_LOG=lt_pipeline=debug,lt_output=debug,lt_tauri=debug cargo tauri dev
```

**Expected log sequence**:
1. "Starting pipeline"
2. "STT session started"
3. "Partial transcript: ..."
4. "Committed transcript: ..."
5. "Transcription complete, starting LLM post-processing"
6. "LLM processing successful (took Xms)"
7. "Text copied to clipboard (X chars)"
8. "Pipeline state: Done"

## Frontend Event Verification

Open browser DevTools console, check for events:
```javascript
// Listen to pipeline events
window.__TAURI__.event.listen('pipeline-state', (event) => {
  console.log('Pipeline state:', event.payload);
});
```

**Expected events**:
- `pipeline-state`: state transitions
- `audio-level`: waveform data
- `pipeline-result`: final processed text

## Integration Testing

### Integration Test 1: Audio → Clipboard
**Full flow**:
1. Start recording
2. Speak: "testing one two three"
3. Stop recording
4. Verify clipboard: "testing one two three"

### Integration Test 2: Long transcription
**Full flow**:
1. Start recording
2. Speak for 30+ seconds continuously
3. Stop recording
4. Verify all text is captured and processed

### Integration Test 3: Multiple sessions
**Full flow**:
1. Complete 5 recording sessions back-to-back
2. Verify each result is independent
3. Check for memory leaks (Activity Monitor)

## Troubleshooting

### Issue: "Pipeline is already running"
**Solution**: Reset state by restarting app

### Issue: Clipboard doesn't update
**Checks**:
1. Check backend logs for "Text copied to clipboard"
2. Try pasting in different apps
3. Check clipboard permissions

### Issue: LLM processing hangs
**Checks**:
1. Test gemini-cli manually: `echo "test" | gemini -p "fix this"`
2. Check network connectivity
3. Check timeout configuration (default 30s)

### Issue: Audio not capturing
**Checks**:
1. Grant microphone permission
2. Check input device in System Preferences
3. Check logs for "No input device" error

## Manual Verification Checklist

- [ ] Pipeline starts successfully
- [ ] Waveform shows audio activity
- [ ] Partial transcription appears in real-time
- [ ] Status transitions are smooth and accurate
- [ ] LLM processing completes without errors
- [ ] Text is copied to clipboard automatically
- [ ] Final result is displayed before fading
- [ ] Multiple sessions work independently
- [ ] Error scenarios are handled gracefully
- [ ] Performance meets expectations

## Next Steps

After successful verification of OUT-001:
- Task #7 (CMD-001): Voice command detection can proceed
- Task #9 (DICT-001): Dictionary integration ready
- Task #11 (UI-001): UI polish can enhance pipeline display

## Notes

- Keyboard simulation (`OutputMode::Keyboard`) requires additional macOS Accessibility permissions
- Testing keyboard output requires manual verification in a text field
- Pipeline is designed to be extensible for future STT providers (OpenAI, Groq)
