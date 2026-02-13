# Murmur

Privacy-first BYOK (Bring Your Own Key) voice typing application built with Tauri 2 and Svelte 5.

## Features

- Real-time speech-to-text with multiple providers (ElevenLabs, OpenAI, Groq)
- LLM post-processing via local CLI tools (gemini-cli, copilot-cli)
- Voice commands (shorten, translate, change tone, generate reply)
- Personal dictionary for custom terms and aliases
- macOS floating overlay window with glassmorphism UI
- System tray integration with dynamic menu
- Global hotkey support (configurable, default Ctrl+`)
- Complete privacy — all data goes directly to your chosen providers
- Comprehensive permissions handling for microphone and accessibility

## Project Structure

```
murmur/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── lt-core/                  # Domain types + traits
│   │   └── src/
│   │       ├── stt.rs            # SttProvider trait, TranscriptionEvent, AudioChunk
│   │       ├── llm.rs            # LlmProcessor trait, ProcessingTask, ProcessingOutput
│   │       ├── config.rs         # AppConfig (TOML-based)
│   │       ├── dictionary.rs     # PersonalDictionary, DictionaryEntry
│   │       ├── output.rs         # OutputSink trait
│   │       └── error.rs          # MurmurError
│   ├── lt-audio/                 # Audio capture (cpal + resampling + VAD)
│   ├── lt-stt/                   # STT providers (ElevenLabs, OpenAI, Groq)
│   ├── lt-llm/                   # LLM post-processing via CLI
│   ├── lt-output/                # Output (clipboard + keyboard simulation)
│   ├── lt-pipeline/              # Pipeline orchestration + voice commands
│   └── lt-tauri/                 # Tauri app (IPC, state, events, window)
├── ui/                           # Svelte 5 + TypeScript frontend
│   └── src/
│       ├── components/
│       │   ├── overlay/          # FloatingOverlay, WaveformIndicator, TranscriptionView
│       │   └── settings/         # SettingsPanel, provider config, DictionaryEditor
│       └── lib/                  # Tauri IPC wrapper
├── config/default.toml           # Default settings template
└── prompts/                      # LLM prompt templates
```

## Development

### Prerequisites
- Rust 1.92+
- Node.js 22+
- npm 11+

### Build & Run

```bash
# Run the full app in development mode
cargo tauri dev

# Run all tests (111 tests)
cargo test --workspace

# Build release binary
cargo build -p lt-tauri --release

# Production bundle (.dmg)
cargo tauri build
```

### Configuration

User configuration is stored at:
- macOS: `~/Library/Application Support/com.hydai.Murmur/config.toml`

Default configuration template: `config/default.toml`

### Installation from .dmg

1. Download the .dmg file from [Releases](https://github.com/hydai/murmur/releases)
2. Double-click to mount the disk image
3. Drag Murmur.app to the Applications folder
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

## Known Limitations

1. **Mac App Store**: The app uses the `macos-private-api` feature for transparent windows, which is not allowed in the Mac App Store. This is intentional for direct distribution.

2. **CLI Tools Required**: To use LLM post-processing features, you need to install local CLI tools:
   - `gemini-cli` for Gemini models
   - `copilot-cli` for GitHub Copilot

   If these are not installed, the app will output raw transcriptions without post-processing.

3. **API Keys Required**: You must provide your own API keys for STT providers (ElevenLabs, OpenAI, or Groq). This is by design for privacy — no third-party servers.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding conventions, and how to submit changes.
