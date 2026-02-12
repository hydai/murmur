# STT Provider Testing Guide (STT-002)

This guide covers manual testing for the multi-provider STT feature with OpenAI Whisper, Groq Whisper Turbo, and ElevenLabs Scribe.

## Prerequisites

1. **API Keys Required**:
   - ElevenLabs API key (for ElevenLabs Scribe)
   - OpenAI API key (for OpenAI Whisper)
   - Groq API key (for Groq Whisper Turbo)

2. **Environment**:
   - macOS with microphone access
   - gemini-cli installed (for LLM post-processing)

## Test Suite

### Test 1: Configure OpenAI Whisper (AC1)

**Steps**:
1. Run `cargo tauri dev`
2. Click the settings button (⚙) in the overlay window
3. Verify the provider list shows:
   - "ElevenLabs Scribe" (Streaming) - may show "Not Configured" if no API key
   - "OpenAI Whisper" (Batch) - should show "Not Configured"
   - "Groq Whisper Turbo" (Batch) - should show "Not Configured"
4. Click on "OpenAI Whisper"
5. Verify a modal appears prompting for API key
6. Enter your OpenAI API key
7. Click "Save & Activate"
8. Verify success message appears
9. Verify "OpenAI Whisper" now shows "Active" badge

**Expected Result**: OpenAI Whisper is configured and active

---

### Test 2: OpenAI Whisper Transcription (AC2, AC3)

**Steps**:
1. Ensure OpenAI Whisper is the active provider
2. Press `Cmd+Shift+Space` to start recording
3. Speak clearly: "This is a test of OpenAI Whisper transcription. I am testing the batch mode processing."
4. Press `Cmd+Shift+Space` to stop recording
5. Observe the status indicator:
   - Should show "Recording" (red pulse)
   - Then "Transcribing" (blue pulse) - may take a few seconds
   - Then "Processing" (purple pulse) - LLM post-processing
   - Finally "Done" (green)
6. Verify transcription appears in the overlay
7. Open a text editor and press `Cmd+V` to paste
8. Verify the transcribed text is accurate and complete

**Expected Result**:
- Text appears after a delay (3-5 seconds for batch processing)
- Final text is copied to clipboard
- Text is accurate and complete

**Note**: OpenAI Whisper uses batch mode, so there will be a slight delay compared to streaming mode. Partial transcriptions may appear as chunks are processed.

---

### Test 3: Configure Groq Whisper Turbo (AC4)

**Steps**:
1. Open settings panel
2. Click on "Groq Whisper Turbo"
3. Verify modal appears
4. Enter your Groq API key
5. Click "Save & Activate"
6. Verify Groq is now active

**Expected Result**: Groq Whisper Turbo is configured and active

---

### Test 4: Groq Whisper Turbo Speed Test (AC5)

**Steps**:
1. Ensure Groq Whisper Turbo is active
2. Press `Cmd+Shift+Space` to start
3. Speak: "Testing Groq Whisper Turbo. This should be very fast."
4. Press `Cmd+Shift+Space` to stop
5. Note the time it takes to see the transcription
6. Compare with OpenAI Whisper timing

**Expected Result**:
- Groq should be noticeably faster than OpenAI
- Groq claims 216x real-time speed
- Transcription should appear within 1-2 seconds

---

### Test 5: Switch Back to ElevenLabs (AC6)

**Steps**:
1. Open settings panel
2. Click on "ElevenLabs Scribe"
3. If not configured, enter your ElevenLabs API key
4. Otherwise, just click to activate
5. Close settings
6. Press `Cmd+Shift+Space` to start
7. Speak: "Testing ElevenLabs streaming mode."
8. Observe real-time partial transcriptions appearing
9. Press `Cmd+Shift+Space` to stop

**Expected Result**:
- ElevenLabs shows streaming behavior (partial text appears immediately)
- No delay in seeing transcription
- Streaming mode still works correctly

---

### Test 6: Provider UI Verification (AC7)

**Steps**:
1. Open settings panel
2. Verify for each provider:
   - Name is clear and descriptive
   - Type label shows "Streaming" or "Batch"
   - Configuration status is accurate (Active / Configured / Not Configured)
   - Badge colors match status (blue for active, green for configured, red for not configured)
3. Hover over each provider card
4. Verify hover effects work (card lifts, border brightens)

**Expected Result**: UI clearly shows all 3 providers with proper labels and status

---

### Test 7: API Key Prompt for Unconfigured Provider (AC8)

**Steps**:
1. If you have a provider that's not configured:
   - Click on it
   - Verify modal appears with:
     - Provider name in title
     - Instructions to enter API key
     - Password input field
     - Cancel and "Save & Activate" buttons
2. Test Cancel button - modal should close
3. Test empty API key - should show error
4. Test valid API key - should save and activate

**Expected Result**: Unconfigured providers show API key input modal

---

### Test 8: REST Provider UI Feedback (AC9)

**Steps**:
1. Select OpenAI or Groq as active provider
2. Start recording
3. Speak for 10+ seconds
4. While speaking, observe the status indicator
5. After stopping, observe status transitions:
   - Recording → Transcribing → Processing → Done
6. For REST providers, during "Transcribing":
   - Verify blue pulsing animation
   - Verify partial text may appear as chunks are processed
   - Verify UI is not frozen/silent

**Expected Result**:
- Status indicator provides clear feedback
- User knows transcription is in progress (not hanging)
- Appropriate animations for each state

---

### Test 9: Long Recording Chunking (AC10)

**Steps**:
1. Select OpenAI or Groq as active provider
2. Start recording
3. Speak continuously for 30-40 seconds:
   - "This is a long test recording to verify that audio chunking works correctly. I will speak for about thirty seconds to ensure multiple chunks are created. OpenAI processes audio in chunks every four seconds, while Groq processes every three seconds. The chunks should be reassembled into a complete transcription at the end."
4. Stop recording
5. Verify the final transcription contains all the spoken text
6. Check for:
   - No missing words
   - No repeated segments
   - Proper sentence flow

**Expected Result**:
- Multiple chunks are processed (you may see partial updates)
- Final transcription is complete and accurate
- No text lost between chunks

---

### Test 10: Provider Switching Stability

**Steps**:
1. Switch between all 3 providers multiple times
2. After each switch, do a quick recording test
3. Verify each provider works correctly after switching

**Expected Result**:
- Switching is seamless
- Each provider works correctly after activation
- No errors or crashes

---

## Error Cases to Test

### Missing API Key
1. Try to start recording with unconfigured provider
2. Should show error message about missing API key

### Network Offline
1. Disconnect from internet
2. Try to record with REST provider (OpenAI/Groq)
3. Should show error message about network failure

### Invalid API Key
1. Enter invalid API key
2. Try to record
3. Should show error message about authentication failure

---

## Configuration File

After testing, you can manually inspect the config file:

```bash
cat ~/.config/localtype/config.toml
```

Should look like:

```toml
stt_provider = "openai"  # or "elevenlabs" or "groq"
hotkey = "Cmd+Shift+Space"

[api_keys]
elevenlabs = "your-elevenlabs-key"
openai = "sk-..."
groq = "gsk_..."

# ... other settings
```

---

## Performance Benchmarks

Expected latencies:

| Provider | Type | Latency | Notes |
|----------|------|---------|-------|
| ElevenLabs | Streaming | ~150ms | Real-time partial results |
| OpenAI Whisper | Batch | 3-5s | Processes every 4 seconds |
| Groq Whisper Turbo | Batch | 1-2s | 216x real-time (very fast) |

---

## Troubleshooting

### No transcription appears
- Check API key is valid
- Check network connection
- Check console logs: `RUST_LOG=lt_stt=debug cargo tauri dev`

### Partial text never becomes final
- Check LLM post-processing is working (gemini-cli installed)
- Check console for LLM errors

### UI not updating
- Check browser console for JavaScript errors
- Check Tauri IPC commands are working

### Settings button not visible
- Make sure FloatingOverlay.svelte changes were applied
- Rebuild frontend: `cd ui && npm run build`

---

## Success Criteria

All 10 acceptance criteria must pass:

- [x] AC1: Configure OpenAI API key and select provider
- [x] AC2-3: OpenAI transcription works correctly
- [x] AC4: Configure Groq API key
- [x] AC5: Groq transcription is fast
- [x] AC6: ElevenLabs streaming still works
- [x] AC7: Provider UI shows all 3 with clear labels
- [x] AC8: Unconfigured providers prompt for API key
- [x] AC9: REST providers show UI feedback during processing
- [x] AC10: Long recordings (30+s) chunk and reassemble correctly

---

## Implementation Summary

**Backend**:
- `crates/lt-stt/src/chunker.rs` - Audio chunking with WAV encoding
- `crates/lt-stt/src/openai.rs` - OpenAI Whisper REST client
- `crates/lt-stt/src/groq.rs` - Groq Whisper Turbo REST client
- `crates/lt-tauri/src/main.rs` - Provider selection logic and IPC commands

**Frontend**:
- `ui/src/components/settings/ProviderConfig.svelte` - Provider selection UI
- `ui/src/components/settings/SettingsPanel.svelte` - Settings modal
- `ui/src/components/overlay/FloatingOverlay.svelte` - Settings button integration

**Tests**:
- 7 new tests in `chunker.rs`, `openai.rs`, `groq.rs`
- All 35 workspace tests passing

**Dependencies**:
- `hound 3.5.1` - WAV encoding for REST APIs
- `reqwest 0.13.2` - HTTP client (already present)
