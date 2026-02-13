# Localtype

Privacy-first BYOK (Bring Your Own Key) voice typing application built with Tauri 2 and Svelte 5.

## Project Structure

```
localtype/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── lt-core/                  # Domain types + traits
│   │   └── src/
│   │       ├── stt.rs            # SttProvider trait, TranscriptionEvent, AudioChunk
│   │       ├── llm.rs            # LlmProcessor trait, ProcessingTask, ProcessingOutput
│   │       ├── config.rs         # AppConfig (TOML-based)
│   │       ├── dictionary.rs     # PersonalDictionary, DictionaryEntry
│   │       ├── output.rs         # OutputSink trait
│   │       └── error.rs          # LocaltypeError
│   └── lt-tauri/                 # Tauri app
├── ui/                           # Svelte 5 + TypeScript frontend
│   └── src/
│       ├── components/
│       │   └── overlay/          # FloatingOverlay component
│       └── lib/stores/           # Svelte stores (reserved)
├── config/default.toml           # Default settings template
└── prompts/                      # LLM prompt templates
    ├── post_process.md
    ├── shorten.md
    ├── change_tone.md
    ├── generate_reply.md
    └── translate.md
```

## Features

All phases complete! Localtype is a fully functional privacy-first voice typing application.

### Core Features
- ✅ Real-time speech-to-text with multiple providers (ElevenLabs, OpenAI, Groq)
- ✅ LLM post-processing via local CLI tools (gemini-cli, copilot-cli)
- ✅ Voice commands (shorten, translate, change tone, generate reply)
- ✅ Personal dictionary for custom terms and aliases
- ✅ macOS floating overlay window with glassmorphism UI
- ✅ System tray integration with dynamic menu (including "Open Settings" support)
- ✅ Dynamic window resizing (600x200 overlay ↔ 800x600 settings panel)
- ✅ Global hotkey support (Cmd+Shift+Space)
- ✅ Complete privacy - all data goes directly to your chosen providers
- ✅ Comprehensive permissions handling for microphone and accessibility
- ✅ Error resilience with automatic reconnection and graceful fallbacks

## Development

### Prerequisites
- Rust 1.92+
- Node.js 22+
- npm 11+

### Development Build

```bash
# Run the full app in development mode
cargo tauri dev
```

### Production Build

Build and package the app as a macOS .dmg:

```bash
# Build the production bundle (includes .app and .dmg)
cargo tauri build

# Output will be in:
# - .app: target/release/bundle/macos/Localtype.app
# - .dmg: target/release/bundle/dmg/Localtype_0.1.0_aarch64.dmg
```

The build process:
1. Compiles the frontend (Svelte) into optimized static assets
2. Compiles the Rust backend in release mode
3. Creates the .app bundle with Info.plist and icons
4. Packages the .app into a .dmg with drag-to-Applications visual

Build time: ~5-10 minutes on Apple Silicon
Final .dmg size: ~7.7MB

### Configuration

Default configuration is stored in `config/default.toml`. User configuration will be stored at:
- macOS: `~/.config/localtype/config.toml`

### Installation from .dmg

1. Download the .dmg file
2. Double-click to mount the disk image
3. Drag Localtype.app to the Applications folder
4. Launch from Applications or Spotlight
5. On first launch:
   - Grant microphone permission (required for voice input)
   - Grant accessibility permission (required for keyboard simulation)
   - Configure your API keys in Settings (accessible from system tray)

## Architecture

### Domain Types (lt-core)

**STT (Speech-to-Text)**
- `SttProvider` trait: Unified interface for streaming and batch STT
- `TranscriptionEvent`: Partial/Committed/Error events
- `AudioChunk`: PCM audio data wrapper

**LLM Processing**
- `LlmProcessor` trait: Text post-processing via local CLI tools
- `ProcessingTask`: PostProcess/Shorten/ChangeTone/GenerateReply/Translate
- `ProcessingOutput`: Processed text with metadata

**Configuration**
- `AppConfig`: TOML-based configuration (API keys, providers, hotkeys, UI preferences)
- `PersonalDictionary`: Custom terms and aliases

**Output**
- `OutputSink` trait: Abstract output destination
- `OutputMode`: Clipboard/Keyboard/Both

## Testing

Run the full test suite:

```bash
# Run all unit tests and integration tests (104 tests)
cargo test --workspace
```

Test coverage includes:
- Audio capture and resampling (13 tests)
- STT providers and WebSocket reconnection (7 unit + 3 integration tests)
- LLM processing and CLI execution (12 unit + 8 integration tests)
- Pipeline orchestration and voice commands (25 tests)
- Dictionary management (10 tests)
- Output handling (6 tests)
- Permission checking (3 tests)
- Error handling (5 tests)

## Distribution

The macOS .dmg includes:
- Localtype.app with proper code signing metadata
- Info.plist with privacy descriptions for microphone and accessibility
- App icon (icon.icns)
- Drag-to-Applications installation UI
- All frontend assets embedded in the binary

Bundle details:
- Identifier: com.localtype.app
- Minimum macOS version: 10.15 (Catalina)
- Architecture: Apple Silicon (aarch64)
- Size: ~7.7MB (DMG), ~20MB (installed)

## Known Limitations

1. **Mac App Store**: The app uses the `macos-private-api` feature for transparent windows, which is not allowed in the Mac App Store. This is intentional for direct distribution.

2. **CLI Tools Required**: To use LLM post-processing features, you need to install local CLI tools:
   - `gemini-cli` for Gemini models
   - `copilot-cli` for GitHub Copilot

   If these are not installed, the app will output raw transcriptions without post-processing.

3. **API Keys Required**: You must provide your own API keys for STT providers (ElevenLabs, OpenAI, or Groq). This is by design for privacy - no third-party servers.
