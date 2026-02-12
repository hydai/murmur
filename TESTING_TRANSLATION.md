# Translation Testing Guide (TRANS-001)

This document provides comprehensive testing instructions for the translation feature via voice commands.

## Overview

The translation feature allows users to translate text by saying:
```
"translate to [language]: [text to translate]"
```

The app detects the command, extracts the target language and text, sends it to the LLM processor with a translation prompt, and outputs the translated text to the clipboard.

## Architecture

```
Voice Input: "translate to Chinese: hello world"
    ↓
STT Provider → Transcription: "translate to Chinese: hello world"
    ↓
Command Detection → Extract language: "Chinese", text: "hello world"
    ↓
ProcessingTask::Translate { text: "hello world", target_language: "Chinese" }
    ↓
PromptManager → Load translate.md template
    ↓
Replace {language} → "Chinese", {text} → "hello world"
    ↓
LLM Processor (Gemini/Copilot) → Execute translation
    ↓
Output → Clipboard: "你好世界"
    ↓
UI Status: "Translating to Chinese..."
```

## Prerequisites

### Required Software

1. **Rust** (for building the project)
2. **Node.js** (for UI development)
3. **At least ONE LLM CLI**:
   - **Gemini CLI** (recommended):
     ```bash
     # Install from https://github.com/google/generative-ai-cli
     brew install google/cli/gemini-cli
     # OR build from source
     ```
   - **Copilot CLI** (alternative):
     ```bash
     npm install -g @githubnext/github-copilot-cli
     ```

4. **STT Provider API Key** (at least one):
   - ElevenLabs: https://elevenlabs.io/
   - OpenAI: https://platform.openai.com/
   - Groq: https://console.groq.com/

### Configuration

Create or edit `~/.config/localtype/config.toml`:

```toml
# LLM Processor (choose one)
llm_processor = "gemini"  # or "copilot"

# STT Provider (choose one)
[stt]
provider = "elevenlabs"  # or "openai" or "groq"

[stt.elevenlabs]
api_key = "your-elevenlabs-api-key"

# OR

[stt.openai]
api_key = "your-openai-api-key"

# OR

[stt.groq]
api_key = "your-groq-api-key"
```

### Verify LLM CLI Installation

```bash
# Check Gemini CLI
which gemini
gemini --version

# OR Check Copilot CLI
which copilot
copilot --version
```

## Unit Tests (Automated)

### Command Detection Tests

Run the translation command detection tests:

```bash
# Run all pipeline tests (includes 9 translation tests)
cargo test -p lt-pipeline commands::tests

# Run specific translation tests
cargo test -p lt-pipeline test_translate_command
cargo test -p lt-pipeline test_translate_to_japanese
cargo test -p lt-pipeline test_translate_to_spanish
cargo test -p lt-pipeline test_translate_to_french
cargo test -p lt-pipeline test_translate_to_german
cargo test -p lt-pipeline test_translate_to_korean
cargo test -p lt-pipeline test_translate_case_insensitive
cargo test -p lt-pipeline test_translate_with_extra_whitespace
cargo test -p lt-pipeline test_translate_multiword_language
cargo test -p lt-pipeline test_translate_complex_content
```

### Prompt Template Tests

Run the prompt interpolation tests:

```bash
# Run all LLM tests (includes translate prompt test)
cargo test -p lt-llm prompts::tests

# Run specific translate prompt test
cargo test -p lt-llm test_build_translate_prompt
```

### Expected Results

All tests should pass:
- ✅ 25 tests in lt-pipeline (16 original + 9 translation tests)
- ✅ 12 tests in lt-llm (9 original + 3 new prompt tests)

## Manual Testing (End-to-End)

### Test Plan Overview

| Test ID | Scenario | Expected Result |
|---------|----------|-----------------|
| AC1 | Translate to Chinese | Chinese translation in clipboard |
| AC2 | Command detection | App detects "translate to" command |
| AC3 | Clipboard output | Translated text copied to clipboard |
| AC4 | Translate to Japanese | Japanese translation in clipboard |
| AC5 | Translate to Spanish | Spanish translation in clipboard |
| AC6 | UI status | Shows "Translating to [language]..." |
| AC7 | Unrecognized language | Clear error or best-effort translation |
| AC8 | Multiple LLM processors | Works with both Gemini and Copilot |

### Test Execution

#### Start the Application

```bash
cd /Users/hydai/workspace/vibe/localtype
cargo tauri dev
```

Wait for the app to start and the floating overlay to appear.

#### AC1: Translate to Chinese

1. **Setup**: Clear clipboard
   ```bash
   pbcopy < /dev/null
   ```

2. **Action**: Press `Cmd+Shift+Space` (or click "Start Recording")

3. **Say**: "translate to Chinese: hello world, how are you today?"

4. **Observe**:
   - Status changes: Recording → Transcribing → Processing
   - UI shows: "Translating to Chinese..."
   - After processing: Shows "Copied to clipboard!"

5. **Verify**: Check clipboard
   ```bash
   pbpaste
   ```

6. **Expected**: Chinese translation (e.g., "你好世界，你今天怎么样？")

7. **Pass Criteria**:
   - ✅ Translation is in Chinese
   - ✅ Translation is accurate and natural
   - ✅ No English text in output
   - ✅ No error messages

#### AC2: Command Detection

1. **Action**: Check browser console (Dev Tools)

2. **Expected**: Log message showing:
   ```
   Command detected: translate to Chinese
   ```

3. **Pass Criteria**:
   - ✅ Command name includes target language
   - ✅ Event emitted with correct command_name

#### AC3: Clipboard Output

1. **Action**: Paste into a text editor (`Cmd+V`)

2. **Expected**: Translated text appears

3. **Pass Criteria**:
   - ✅ Clipboard contains translated text
   - ✅ Text is properly formatted
   - ✅ No extra whitespace or artifacts

#### AC4: Translate to Japanese

1. **Say**: "translate to Japanese: thank you very much"

2. **Expected**: Japanese translation (e.g., "ありがとうございます")

3. **Pass Criteria**:
   - ✅ Translation is in Japanese characters
   - ✅ Translation is polite and appropriate

#### AC5: Translate to Spanish

1. **Say**: "translate to Spanish: the meeting is at 3pm"

2. **Expected**: Spanish translation (e.g., "La reunión es a las 3pm")

3. **Pass Criteria**:
   - ✅ Translation is in Spanish
   - ✅ Grammar is correct

#### AC6: UI Status Display

1. **Test Multiple Languages**:
   - Say: "translate to French: good morning"
   - Observe UI: Should show "Translating to French..."

   - Say: "translate to German: how are you"
   - Observe UI: Should show "Translating to German..."

2. **Pass Criteria**:
   - ✅ UI shows specific language being translated to
   - ✅ Status updates in real-time
   - ✅ Status clears after completion

#### AC7: Unrecognized Language

1. **Test Obscure Language**:
   - Say: "translate to Klingon: hello world"

2. **Expected Behavior** (one of):
   - Best-effort translation (LLM attempts it)
   - Clear error message
   - Fallback to raw text

3. **Pass Criteria**:
   - ✅ No app crash
   - ✅ User receives feedback
   - ✅ Can continue using app

4. **Test Multi-Word Language**:
   - Say: "translate to Traditional Chinese: hello"

5. **Expected**: Correctly extracts "Traditional Chinese" as language

6. **Pass Criteria**:
   - ✅ Multi-word languages work
   - ✅ Translation uses correct variant

#### AC8: Multiple LLM Processors

##### Test with Gemini CLI

1. **Configure**: Edit `~/.config/localtype/config.toml`
   ```toml
   llm_processor = "gemini"
   ```

2. **Restart**: Stop and restart the app

3. **Test**: Translate to Chinese
   - Say: "translate to Chinese: hello world"

4. **Expected**: Translation appears in clipboard

5. **Check Logs**: Look for "Executing gemini CLI" in console

6. **Pass Criteria**:
   - ✅ Gemini CLI used successfully
   - ✅ Translation quality is good

##### Test with Copilot CLI (if installed)

1. **Configure**: Edit `~/.config/localtype/config.toml`
   ```toml
   llm_processor = "copilot"
   ```

2. **Restart**: Stop and restart the app

3. **Test**: Translate to Japanese
   - Say: "translate to Japanese: thank you"

4. **Expected**: Translation appears in clipboard

5. **Check Logs**: Look for "Executing copilot CLI" in console

6. **Pass Criteria**:
   - ✅ Copilot CLI used successfully
   - ✅ Translation quality is good

### Additional Test Cases

#### Test Case: Long Text Translation

1. **Say**: "translate to Spanish: I would like to inform you that our quarterly business review meeting has been scheduled for next Friday at 3pm in the main conference room. Please make sure to bring your reports and be prepared to discuss the results from last quarter."

2. **Expected**: Complete Spanish translation of entire passage

3. **Pass Criteria**:
   - ✅ Full text translated
   - ✅ No truncation
   - ✅ Maintains meaning

#### Test Case: Special Characters

1. **Say**: "translate to French: Hello! How are you? I'm fine, thanks."

2. **Expected**: Proper French with correct punctuation and accents

3. **Pass Criteria**:
   - ✅ Accents preserved (e.g., "Comment allez-vous?")
   - ✅ Punctuation appropriate for target language

#### Test Case: Numbers and Times

1. **Say**: "translate to German: The meeting is at 3:30pm on May 15th"

2. **Expected**: German translation with appropriate number formatting

3. **Pass Criteria**:
   - ✅ Numbers translated correctly
   - ✅ Date/time format appropriate for German

#### Test Case: Case Insensitivity

1. **Say**: "TRANSLATE TO CHINESE: HELLO"

2. **Expected**: Works the same as lowercase

3. **Pass Criteria**:
   - ✅ Command detected despite uppercase
   - ✅ Translation successful

## Error Scenarios

### Scenario 1: LLM CLI Not Installed

**Setup**: Temporarily rename the LLM CLI binary
```bash
sudo mv /usr/local/bin/gemini /usr/local/bin/gemini.bak
```

**Test**: Try to translate

**Expected**:
- Error message about missing CLI
- Fallback to raw transcription
- Clear user feedback

**Cleanup**:
```bash
sudo mv /usr/local/bin/gemini.bak /usr/local/bin/gemini
```

### Scenario 2: Missing Colon in Command

**Say**: "translate to Chinese hello world" (no colon)

**Expected**:
- Falls back to default post-processing
- No crash

### Scenario 3: Empty Text After Colon

**Say**: "translate to Chinese:"

**Expected**:
- Either error or empty output
- No crash

## Performance Testing

### Translation Latency

Measure end-to-end time from saying the command to clipboard update:

1. **Record timestamp** when you start speaking
2. **Record timestamp** when "Copied!" indicator appears
3. **Calculate latency**

**Expected Latency**:
- ElevenLabs STT: ~500ms
- LLM Processing (Gemini): 1-3 seconds
- Total: 1.5-3.5 seconds

**Pass Criteria**:
- ✅ Total latency < 5 seconds
- ✅ No UI freezing
- ✅ Responsive during processing

## Troubleshooting

### Translation Not Working

1. **Check LLM CLI**:
   ```bash
   which gemini
   gemini -p "Translate to Chinese: hello" -m gemini-2.5-flash
   ```

2. **Check Logs**:
   - Look for "Command detected: translate to [language]"
   - Look for "LLM processing failed"
   - Check for prompt template errors

3. **Verify Prompt Template**:
   ```bash
   cat prompts/translate.md
   ```
   Should contain `{language}` and `{text}` placeholders

### UI Not Showing "Translating to [Language]"

1. **Check Event Emission**:
   - Open browser Dev Tools console
   - Look for "Command detected" log
   - Verify `command_name` field

2. **Check Svelte Component**:
   - Verify `detectedCommand` state is updated
   - Check conditional rendering logic

### Wrong Language Translation

1. **Verify Command Detection**:
   - Check logs for extracted language name
   - Ensure colon placement is correct

2. **Test Directly**:
   ```bash
   # Test the LLM CLI directly
   gemini -p "Translate the following text to Chinese: hello world" -m gemini-2.5-flash
   ```

## Success Criteria

All acceptance criteria must pass:

- ✅ AC1: "translate to Chinese: hello world, how are you today?" → Chinese text in clipboard
- ✅ AC2: App detects "translate to" command with target language
- ✅ AC3: Clipboard contains translated text
- ✅ AC4: "translate to Japanese: thank you very much" → Japanese translation
- ✅ AC5: "translate to Spanish: the meeting is at 3pm" → Spanish translation
- ✅ AC6: Overlay shows "Translating to [language]..." status
- ✅ AC7: Unrecognized target language shows clear error or best-effort
- ✅ AC8: Translation works with both gemini-cli and copilot-cli

## Code Coverage

### Files Implementing Translation

1. **Command Detection**: `crates/lt-pipeline/src/commands.rs`
   - Lines 95-114: Translate command parsing
   - Tests: Lines 194-304 (9 translation tests)

2. **Processing Task**: `crates/lt-core/src/llm.rs`
   - Lines 28-32: ProcessingTask::Translate variant

3. **Prompt Management**: `crates/lt-llm/src/prompts.rs`
   - Lines 58-66: Translate prompt building
   - Test: Lines 122-132: Translate prompt test

4. **Prompt Template**: `prompts/translate.md`
   - Template with {language} and {text} placeholders

5. **LLM Processors**:
   - `crates/lt-llm/src/gemini.rs`: Lines 66-102 (handles all tasks)
   - `crates/lt-llm/src/copilot.rs`: Lines 41-92 (handles all tasks)

6. **Pipeline Orchestration**: `crates/lt-pipeline/src/orchestrator.rs`
   - Lines 152-175: Command detection integration
   - Lines 187-222: Task processing and output

7. **UI Status**: `ui/src/components/overlay/FloatingOverlay.svelte`
   - Lines 192-196: Command detection event listener
   - Lines 256-257: Translation status display

### Test Coverage Summary

- **Unit Tests**: 25 tests in lt-pipeline, 12 tests in lt-llm
- **Integration Tests**: Manual end-to-end testing required
- **Edge Cases**: Case insensitivity, whitespace, multi-word languages, empty text

## Conclusion

The translation feature is fully implemented with comprehensive test coverage. All unit tests pass automatically. Manual testing is required to verify end-to-end functionality with real STT providers and LLM CLIs.

For questions or issues, check the logs and refer to the troubleshooting section above.
