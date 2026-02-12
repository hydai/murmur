# Testing LLM Post-Processing (LLM-001)

This document describes how to test the LLM post-processing functionality.

## Prerequisites

1. **Install gemini-cli**:
   ```bash
   # Follow instructions at: https://github.com/google/generative-ai-cli
   # Or install via npm (example):
   npm install -g @google/generative-ai-cli

   # Verify installation:
   gemini --version
   ```

2. **Configure API Key** (if required by gemini-cli):
   - Set up your Google AI API key according to gemini-cli documentation

3. **ElevenLabs API Key**:
   - Ensure your `~/.config/localtype/config.toml` has a valid ElevenLabs API key

## Architecture Overview

```
Recording → STT (ElevenLabs) → Transcription Events
                                      ↓
                              Accumulate full text
                                      ↓
                              LLM Post-Processing
                                      ↓
                          Cleaned Text → Frontend
```

### Components

1. **lt-llm Crate**:
   - `executor.rs`: Generic CLI process spawner with timeout
   - `prompts.rs`: Template manager for loading and interpolating prompts
   - `gemini.rs`: Gemini CLI adapter implementing LlmProcessor trait
   - `copilot.rs`: Copilot CLI adapter (stub implementation)

2. **Tauri Backend**:
   - Health check on startup: verify gemini CLI availability
   - Transcription accumulation: collect all committed transcripts
   - Automatic processing: when transcription completes, trigger LLM
   - Events: `processing-status`, `transcription-processed`

3. **Frontend**:
   - Status indicator: Ready → Recording → Transcribing → Processing → Done
   - Processing state with purple pulsing indicator
   - Display processed text when LLM completes

## Acceptance Criteria Testing

### Step 1: Verify gemini CLI Health Check

**Expected Behavior**: On app startup, the health check should verify if gemini CLI is installed.

**Test Procedure**:
1. Build and run the app:
   ```bash
   cargo tauri dev
   ```

2. Check the console logs for one of these messages:
   - `✓ Gemini CLI is available and ready` (if gemini is installed)
   - `⚠ Gemini CLI is not installed. LLM post-processing will not be available.` (if not installed)

**Pass Criteria**: Appropriate message is logged based on gemini CLI availability.

---

### Step 2: Record Sentence with Filler Words

**Test Input**: "um, so like, I was thinking, uh, we should, you know, start the meeting"

**Test Procedure**:
1. Run `cargo tauri dev`
2. Press `Cmd+Shift+Space` to start recording
3. Speak the test sentence clearly
4. Press `Cmd+Shift+Space` to stop recording

**Pass Criteria**: Recording captures the audio and sends it to ElevenLabs for transcription.

---

### Step 3: Automatic LLM Post-Processing

**Expected Behavior**: After transcription completes, the text is automatically sent to gemini-cli.

**Test Procedure**:
1. Complete Step 2 above
2. Observe the console logs

**Expected Logs**:
```
Committed transcript: um, so like, I was thinking, uh, we should, you know, start the meeting
Transcription complete, starting LLM post-processing
Executing gemini CLI with prompt (length: XXX chars)
LLM processing successful (took XXXms)
```

**Pass Criteria**: Logs show the transcription was sent to LLM for processing.

---

### Step 4: Status Transitions

**Expected Flow**: transcribing → processing → done

**Test Procedure**:
1. Complete Step 2 above
2. Watch the overlay status indicator and text

**Expected Transitions**:
1. **Recording** (red pulsing dot): While speaking
2. **Transcribing** (blue pulsing dot): While ElevenLabs processes audio
3. **Processing** (purple pulsing dot): While gemini-cli post-processes text
4. **Done** (green dot): After LLM completes

**Pass Criteria**: All status transitions occur smoothly with appropriate indicators.

---

### Step 5: Cleaned Text Output

**Expected Output**: "I was thinking we should start the meeting"

**Test Procedure**:
1. Complete Step 2 above
2. Wait for processing to complete
3. Observe the final text in the overlay

**Pass Criteria**:
- Filler words removed: "um", "uh", "like", "you know", "so"
- Grammar corrected
- Proper capitalization and punctuation

---

### Step 6: Verify Prompt Template

**Expected Content**: Task instruction + raw transcription + personal dictionary terms

**Test Procedure**:
1. Enable debug logging to see the full prompt:
   ```bash
   RUST_LOG=lt_llm=debug cargo tauri dev
   ```

2. Complete Step 2 above

3. Check the logs for prompt content

**Expected Prompt Structure**:
```markdown
# Post-Process Transcription

You are a transcription post-processor...

## Personal Dictionary Terms

[List of terms or "No custom terms defined."]

## Raw Transcription

um, so like, I was thinking, uh, we should, you know, start the meeting

## Output

Return only the cleaned text...
```

**Pass Criteria**: Prompt includes all three components: instructions, dictionary terms, and raw text.

---

### Step 7: Gemini CLI Command Format

**Expected Command**:
```bash
gemini -p "prompt" --output-format json -m gemini-2.5-flash
```

**Test Procedure**:
1. Enable trace logging:
   ```bash
   RUST_LOG=trace cargo tauri dev
   ```

2. Complete Step 2 above

3. Check logs or use process monitoring to verify the command

**Pass Criteria**: The exact command format is used with all specified flags.

---

### Step 8: Fallback on Failure

**Expected Behavior**: If gemini-cli fails or times out, show raw transcription with warning.

**Test Scenarios**:

#### 8a: CLI Not Installed
1. Uninstall or rename gemini CLI temporarily
2. Run the app and complete Step 2
3. Observe: Raw transcription is shown with a warning message

#### 8b: CLI Timeout
1. Set a very short timeout (modify executor timeout for testing)
2. Run the app and complete Step 2
3. Observe: Raw transcription is shown after timeout

#### 8c: CLI Process Error
1. Simulate an error (e.g., invalid prompt format)
2. Run the app and complete Step 2
3. Observe: Raw transcription is shown with error message

**Pass Criteria**: In all failure cases, the app:
- Does not crash
- Shows the raw transcription
- Displays a clear error/warning message
- Emits `processing-status: error` event

---

## Additional Tests

### Test with Dictionary Terms

**Setup**:
1. Create `~/.config/localtype/dictionary.json`:
   ```json
   {
     "entries": [
       {
         "term": "API",
         "aliases": ["A P I", "api"],
         "description": "Application Programming Interface"
       },
       {
         "term": "STT",
         "aliases": ["S T T", "speech to text"],
         "description": "Speech-to-Text"
       }
     ]
   }
   ```

2. Say: "um, the A P I for speech to text is working"

**Expected Output**: "The API for STT is working"

**Pass Criteria**: Dictionary terms are correctly included in the prompt and used by the LLM.

---

### Test Long Transcription

**Input**: Speak continuously for 30+ seconds with multiple filler words

**Expected Behavior**:
- All transcripts are accumulated
- Full text is sent to LLM
- Processing completes within reasonable time
- No timeout errors

**Pass Criteria**: Long transcriptions are handled correctly without truncation or timeout.

---

### Test Multiple Sessions

**Procedure**:
1. Start recording, speak, stop → LLM processes
2. Start recording again, speak, stop → LLM processes again
3. Repeat 5 times

**Expected Behavior**:
- Each session starts with empty transcription
- Each session triggers independent LLM processing
- No state leakage between sessions

**Pass Criteria**: All sessions work independently without interference.

---

## Manual Verification Checklist

- [ ] Gemini CLI health check logs correct status
- [ ] Recording with filler words captures audio
- [ ] Transcription is automatically sent to LLM
- [ ] Status transitions: recording → transcribing → processing → done
- [ ] Cleaned text removes filler words and fixes grammar
- [ ] Prompt includes instructions + raw text + dictionary terms
- [ ] Gemini CLI uses correct command format with flags
- [ ] Fallback shows raw transcription on CLI failure/timeout
- [ ] Dictionary terms are included in prompts
- [ ] Long transcriptions process without timeout
- [ ] Multiple sessions work independently

---

## Troubleshooting

### Issue: "Gemini CLI not found"

**Solution**:
1. Verify installation: `which gemini`
2. Ensure gemini is in PATH
3. Restart terminal/IDE after installation

### Issue: "LLM processing failed"

**Check**:
1. Gemini CLI logs: `gemini --version`
2. API key configuration
3. Network connectivity
4. Prompt format (check for special characters)

### Issue: "Processing takes too long"

**Solutions**:
1. Check network speed
2. Verify gemini model availability
3. Consider increasing timeout in `GeminiProcessor::with_timeout()`

### Issue: "Cleaned text is empty"

**Check**:
1. Gemini CLI output format
2. JSON parsing logic in `parse_json_output()`
3. Prompt template format

---

## Performance Expectations

- **Health Check**: < 1 second
- **LLM Processing**: 1-5 seconds (depends on text length and network)
- **Status Transitions**: Immediate (< 100ms)
- **Total End-to-End**: Recording → Transcription (150ms per chunk) → LLM (1-5s) → Display

---

## Next Steps

After successful LLM-001 verification:
1. Task OUT-001: Pipeline orchestration with clipboard output
2. Task CMD-001: Voice command detection ("shorten this", "translate to", etc.)
3. Task DICT-001: Personal dictionary CRUD UI
