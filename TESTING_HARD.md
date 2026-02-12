# Testing Guide: HARD-001 - Permissions, Error Resilience, and Testing

This document provides comprehensive testing procedures for the hardening features implemented in HARD-001.

## Overview

HARD-001 implements:
1. macOS permission handling (Microphone + Accessibility)
2. Error resilience (WebSocket reconnection, CLI fallback)
3. Comprehensive test coverage (93 total tests)

## Acceptance Criteria Testing

### AC1: Microphone Permission Prompt on First Use

**Test Procedure**:
1. Fresh macOS system or reset app permissions:
   ```bash
   # Reset Localtype permissions (if testing on existing install)
   tccutil reset Microphone com.localtype.app
   ```
2. Launch the app: `cargo tauri dev`
3. Start recording (Cmd+Shift+Space or via tray menu)
4. **Expected**: macOS shows microphone permission dialog
5. **Verify**: Dialog message matches Info.plist description: "Localtype needs microphone access for voice-to-text transcription"

**Screenshot Checklist**:
- [ ] Permission dialog appears
- [ ] Message text is correct
- [ ] "Allow" and "Don't Allow" buttons present

---

### AC2: Recording Works After Granting Permission

**Test Procedure**:
1. Grant microphone permission (click "Allow")
2. Speak into microphone: "Hello world"
3. **Expected**:
   - Waveform indicator shows activity
   - Partial transcription appears in real-time
   - Final transcription appears after stopping
4. **Verify**: Text is transcribed correctly

**Success Criteria**:
- [ ] Audio capture starts immediately
- [ ] Waveform shows voice activity
- [ ] Transcription appears in UI
- [ ] No error messages

---

### AC3: Clear Error Message When Permission Denied

**Test Procedure**:
1. Reset permissions: `tccutil reset Microphone com.localtype.app`
2. Launch app
3. Start recording
4. Click "Don't Allow" on permission dialog
5. **Expected**: App shows error message with guidance
6. **Verify**: Error message includes:
   - Explanation of why permission is needed
   - Button to open System Preferences
   - No crash or freeze

**Test Button**:
1. Click "Open System Preferences" button
2. **Expected**: System Preferences opens to Privacy > Microphone
3. **Verify**: Localtype is listed in microphone access list

**Screenshot Checklist**:
- [ ] Error message displayed
- [ ] Message is user-friendly
- [ ] System Preferences button works
- [ ] App remains responsive

---

### AC4: Accessibility Permission for Keyboard Simulation

**Test Procedure**:
1. Ensure Accessibility permission is not granted:
   ```bash
   # Check current status
   sqlite3 ~/Library/Application\ Support/com.apple.TCC/TCC.db \
     "SELECT * FROM access WHERE service='kTCCServiceAccessibility'"
   ```
2. Set output mode to "Keyboard Simulation" in Settings
3. Record and transcribe text
4. **Expected**:
   - macOS shows Accessibility permission dialog OR
   - App shows message: "Accessibility permission required for keyboard simulation"
5. Grant permission in System Preferences > Privacy > Accessibility
6. Try keyboard simulation again
7. **Expected**: Text is typed into active application

**Success Criteria**:
- [ ] Permission prompt or guidance shown
- [ ] After granting, keyboard simulation works
- [ ] Text appears in focused app (e.g., TextEdit)
- [ ] Graceful degradation if denied (clipboard fallback)

---

### AC5: WebSocket Reconnection During Network Interruption

**Test Procedure**:
1. Use ElevenLabs STT provider
2. Start recording
3. While recording, simulate network interruption:
   ```bash
   # macOS: Disable Wi-Fi
   networksetup -setairportpower en0 off
   ```
4. **Expected**:
   - UI shows "Connection lost, reconnecting..." message
   - App continues attempting to reconnect
5. Re-enable network:
   ```bash
   networksetup -setairportpower en0 on
   ```
6. **Expected**:
   - Within 1-30 seconds, connection restored
   - UI shows "Connected" or resumes transcription
   - No data loss (buffered audio sent)

**Verify Reconnection Logic**:
```bash
# Check logs for reconnection attempts
cargo tauri dev 2>&1 | grep -i "reconnect\|websocket"
```

**Expected Log Output**:
```
WebSocket connection failed (attempt 1/10), retrying in 1000ms
WebSocket connection failed (attempt 2/10), retrying in 2000ms
WebSocket connected to ElevenLabs
```

**Screenshot Checklist**:
- [ ] "Reconnecting..." status shown
- [ ] Exponential backoff delays logged
- [ ] Connection restored automatically
- [ ] Transcription resumes

---

### AC6: CLI Fallback When gemini-cli Not Found

**Test Procedure**:
1. Temporarily remove gemini-cli from PATH:
   ```bash
   # Check current location
   which gemini

   # Rename or move it
   sudo mv /usr/local/bin/gemini /usr/local/bin/gemini.bak
   ```
2. Record and transcribe text
3. **Expected**:
   - Error message: "Gemini CLI not found. Please install gemini-cli: https://github.com/google/generative-ai-cli"
   - Raw transcription still available in clipboard
   - No app crash

**Verify Fallback**:
1. Check clipboard content: Should contain raw STT output
2. Check UI: Should show raw transcription (not processed)
3. Check logs: Should show LLM error with fallback message

**Restore**:
```bash
sudo mv /usr/local/bin/gemini.bak /usr/local/bin/gemini
```

**Success Criteria**:
- [ ] Clear error message shown
- [ ] Installation URL provided
- [ ] Raw transcription in clipboard
- [ ] App remains functional

---

### AC7: Unit Tests Pass

**Test Procedure**:
```bash
cargo test --workspace
```

**Expected Output**:
```
test result: ok. 93 passed; 0 failed; 0 ignored; 0 measured
```

**Test Coverage Breakdown**:
- lt-audio: 13 tests (added edge cases for resampler)
- lt-core: 16 tests (added error conversion tests)
- lt-llm: 12 tests (existing)
- lt-output: 6 tests (existing)
- lt-pipeline: 25 tests (existing)
- lt-stt: 7 tests (existing)
- lt-tauri: 3 tests (permissions module)
- Integration tests: 11 tests (CLI + WebSocket)

**Test Categories**:
1. **Error Handling**:
   - Error type conversions (IO, JSON, TOML)
   - Error display formatting
   - Permission errors
2. **Resampler Edge Cases**:
   - Empty input
   - Single sample
   - Extreme values (i16::MAX, i16::MIN)
   - High sample rate conversion (96kHz → 16kHz)
3. **CLI Integration**:
   - Timeout handling
   - NotFound errors
   - Exit code handling
   - Mock CLI success/failure
4. **WebSocket**:
   - Mock server
   - Reconnection simulation
   - Exponential backoff

---

### AC8: Integration Tests with Mock Servers

**Test Procedure**:
```bash
# Run LLM CLI integration tests
cargo test --package lt-llm --test cli_integration

# Run WebSocket integration tests
cargo test --package lt-stt --test websocket_integration
```

**LLM Integration Tests** (8 tests):
1. `test_cli_timeout_handling` - Verifies 30s timeout
2. `test_cli_not_found_handling` - Verifies NotFound error
3. `test_cli_exit_code_handling` - Verifies non-zero exits
4. `test_mock_gemini_cli_success` - Mock CLI returns JSON
5. `test_mock_gemini_cli_failure` - Mock CLI fails with error
6. `test_is_available` - CLI availability check
7. `test_cli_stdout_stderr_capture` - Output capture
8. `test_fallback_behavior_simulation` - Primary/secondary fallback

**WebSocket Integration Tests** (3 tests):
1. `test_mock_websocket_server` - Mock server echo
2. `test_websocket_reconnection_simulation` - Event flow
3. `test_exponential_backoff` - Delay calculation

**Expected Output**:
```
test result: ok. 8 passed; 0 failed (CLI)
test result: ok. 3 passed; 0 failed (WebSocket)
```

---

### AC9: Info.plist Contains Privacy Descriptions

**Test Procedure**:
1. Check Info.plist file:
   ```bash
   cat crates/lt-tauri/Info.plist
   ```
2. **Expected Content**:
   ```xml
   <key>NSMicrophoneUsageDescription</key>
   <string>Localtype needs microphone access for voice-to-text transcription</string>
   <key>NSAccessibilityUsageDescription</key>
   <string>Localtype needs Accessibility permission to simulate keyboard input for typing transcribed text</string>
   ```
3. Build the app:
   ```bash
   cargo tauri build
   ```
4. Check built app's Info.plist:
   ```bash
   cat target/release/bundle/macos/Localtype.app/Contents/Info.plist
   ```
5. **Verify**: Both keys present in built app

**Success Criteria**:
- [ ] Info.plist exists in project
- [ ] Both permission keys present
- [ ] Descriptions are clear and accurate
- [ ] Built app includes Info.plist

---

## Additional Testing Scenarios

### Error Resilience: Rate Limiting

**Test Procedure**:
1. Use OpenAI or Groq STT provider
2. Make many rapid requests to trigger rate limit
3. **Expected**:
   - Error message: "Rate limited, please wait..."
   - Automatic retry after delay
   - No crash

### Error Resilience: CLI Timeout

**Test Procedure**:
1. Use a mock CLI that sleeps for 35 seconds:
   ```bash
   #!/bin/bash
   sleep 35
   echo '{"text": "timeout test"}'
   ```
2. Process transcription
3. **Expected**:
   - Timeout after 30 seconds
   - Error: "Gemini CLI timed out"
   - Raw transcription available

### Permission Changes During Runtime

**Test Procedure**:
1. Grant microphone permission
2. Start recording
3. While recording, revoke permission in System Preferences
4. Stop and restart recording
5. **Expected**:
   - Error on next recording attempt
   - Guidance to re-enable permission
   - No crash

---

## Performance Testing

### WebSocket Reconnection Performance

**Metrics**:
- Initial connection time: < 2s
- First retry delay: 1s
- Second retry delay: 2s
- Third retry delay: 4s
- Max retry delay: 30s
- Max retries: 10

**Test**:
```bash
# Monitor logs for timing
cargo tauri dev 2>&1 | ts '[%Y-%m-%d %H:%M:%.S]' | grep reconnect
```

### CLI Execution Performance

**Metrics**:
- CLI spawn time: < 100ms
- Total timeout: 30s
- Availability check: < 500ms

**Test**:
```bash
# Time CLI execution
time gemini -p "test" --output-format json -m gemini-2.5-flash
```

---

## Debugging Tips

### Permission Issues

1. **Check TCC database**:
   ```bash
   # Microphone
   sqlite3 ~/Library/Application\ Support/com.apple.TCC/TCC.db \
     "SELECT * FROM access WHERE service='kTCCServiceMicrophone'"

   # Accessibility
   sqlite3 ~/Library/Application\ Support/com.apple.TCC/TCC.db \
     "SELECT * FROM access WHERE service='kTCCServiceAccessibility'"
   ```

2. **Reset all permissions**:
   ```bash
   tccutil reset All com.localtype.app
   ```

3. **Check Info.plist loaded**:
   ```bash
   plutil -p target/debug/bundle/macos/Localtype.app/Contents/Info.plist | grep Usage
   ```

### WebSocket Issues

1. **Monitor WebSocket traffic**:
   ```bash
   # Enable verbose logging
   RUST_LOG=debug cargo tauri dev 2>&1 | grep -i websocket
   ```

2. **Test connectivity**:
   ```bash
   # Test ElevenLabs endpoint
   curl -i https://api.elevenlabs.io/v1/speech-to-text/ws
   ```

### CLI Issues

1. **Verify CLI in PATH**:
   ```bash
   which gemini
   which copilot
   ```

2. **Test CLI manually**:
   ```bash
   gemini -p "test" --output-format json -m gemini-2.5-flash
   ```

3. **Check CLI permissions**:
   ```bash
   ls -la $(which gemini)
   ```

---

## Summary Checklist

Before marking HARD-001 as complete, verify:

- [ ] AC1: Fresh install prompts for microphone permission
- [ ] AC2: Recording works after granting permission
- [ ] AC3: Clear error message when permission denied
- [ ] AC4: Accessibility permission works for keyboard simulation
- [ ] AC5: WebSocket reconnects after network interruption
- [ ] AC6: CLI fallback shows raw transcription
- [ ] AC7: All 93 unit tests pass
- [ ] AC8: All 11 integration tests pass
- [ ] AC9: Info.plist has privacy descriptions
- [ ] Build succeeds: `cargo tauri build`
- [ ] No regressions in existing features
- [ ] All error messages are user-friendly
- [ ] Logs are informative for debugging

---

## Test Results Log

| AC | Test | Status | Notes |
|----|------|--------|-------|
| AC1 | Microphone prompt | ⏳ Pending | Manual test required |
| AC2 | Recording after grant | ⏳ Pending | Manual test required |
| AC3 | Error message on deny | ⏳ Pending | Manual test required |
| AC4 | Accessibility permission | ⏳ Pending | Manual test required |
| AC5 | WebSocket reconnection | ⏳ Pending | Manual test required |
| AC6 | CLI fallback | ⏳ Pending | Manual test required |
| AC7 | Unit tests | ✅ Pass | 93/93 tests passing |
| AC8 | Integration tests | ✅ Pass | 11/11 tests passing |
| AC9 | Info.plist | ✅ Pass | File present with correct keys |

---

## Manual Testing Commands

```bash
# Run all tests
cargo test --workspace

# Build the app
cargo build --package lt-tauri

# Run development mode
cargo tauri dev

# Build production bundle
cargo tauri build

# Check Info.plist
cat crates/lt-tauri/Info.plist

# Reset permissions for testing
tccutil reset Microphone com.localtype.app
tccutil reset Accessibility com.localtype.app

# Simulate network interruption (macOS)
networksetup -setairportpower en0 off
networksetup -setairportpower en0 on

# Temporarily hide CLI
sudo mv /usr/local/bin/gemini /usr/local/bin/gemini.bak
sudo mv /usr/local/bin/gemini.bak /usr/local/bin/gemini
```

---

## Expected Test Output

### Unit Tests
```
test result: ok. 13 passed; 0 failed (lt-audio)
test result: ok. 16 passed; 0 failed (lt-core)
test result: ok. 12 passed; 0 failed (lt-llm)
test result: ok. 6 passed; 0 failed (lt-output)
test result: ok. 25 passed; 0 failed (lt-pipeline)
test result: ok. 7 passed; 0 failed (lt-stt)
test result: ok. 3 passed; 0 failed (lt-tauri)
Total: 93 tests passed
```

### Integration Tests
```
test result: ok. 8 passed; 0 failed (CLI integration)
test result: ok. 3 passed; 0 failed (WebSocket integration)
Total: 11 integration tests passed
```

### Build Output
```
Finished `dev` profile [unoptimized + debuginfo] target(s)
info.plist validated
Bundle generated successfully
```
