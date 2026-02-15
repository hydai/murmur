<p align="center">
  <img src="murmur.png" alt="Murmur" width="128" />
</p>

# Murmur

Privacy-first BYOK (Bring Your Own Key) voice typing application built with Tauri 2 and Svelte 5.

## Features

- Real-time speech-to-text with multiple providers (Apple, ElevenLabs, OpenAI, Groq)
- Apple STT: on-device speech recognition via macOS — no API key needed
- LLM post-processing via HTTP APIs (OpenAI, Claude, Gemini API, custom OpenAI-compatible endpoints), local CLI tools (gemini-cli, copilot-cli), or Apple Foundation Models (on-device)
- Configurable LLM model selection per provider (override defaults from Settings)
- Output modes: clipboard (default), keyboard simulation, or both
- Transcription history with search, copy, and persistent storage
- Voice commands (shorten, translate, change tone, generate reply)
- Personal dictionary for custom terms and aliases
- macOS floating overlay window with glassmorphism UI
- Audio cues for state feedback (recording start/stop, errors)
- System tray integration with dynamic menu
- Global hotkey support (configurable, default Ctrl+`)
- Auto-opens settings on first launch for easy onboarding
- Auto-updater for seamless in-app updates
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
│   │       ├── history.rs         # TranscriptionHistory, HistoryEntry
│   │       ├── output.rs         # OutputSink trait, OutputMode
│   │       └── error.rs          # MurmurError
│   ├── lt-audio/                 # Audio capture (cpal + resampling + VAD)
│   ├── lt-stt/                   # STT providers (ElevenLabs, OpenAI, Groq)
│   ├── lt-stt-apple/             # Swift FFI bridge for Apple SpeechTranscriber
│   ├── lt-llm/                   # LLM post-processing (HTTP APIs + CLI)
│   ├── lt-llm-apple/             # Apple Foundation Models (on-device LLM via Swift FFI)
│   ├── lt-output/                # Output (clipboard + keyboard simulation)
│   ├── lt-pipeline/              # Pipeline orchestration + voice commands
│   └── lt-tauri/                 # Tauri app (IPC, state, events, window)
├── ui/                           # Svelte 5 + TypeScript frontend
│   └── src/
│       ├── components/
│       │   ├── history/          # HistoryPanel (transcription history with search)
│       │   ├── overlay/          # FloatingOverlay, WaveformIndicator, TranscriptionView
│       │   └── settings/         # SettingsPanel (standalone 720x560 window)
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

# Run all tests (93 tests)
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
- Apple STT provider: on-device speech recognition via Swift FFI bridge (lt-stt-apple)
- Audio cues for state feedback (recording start/stop, errors) replace visual overlay indicators

**LLM Processing**
- `LlmProcessor` trait: Text post-processing via HTTP APIs (OpenAI, Claude, Gemini API, custom endpoints), local CLI tools, or Apple Foundation Models
- `ProcessingTask`: PostProcess/Shorten/ChangeTone/GenerateReply/Translate
- `ProcessingOutput`: Processed text with metadata
- Apple Foundation Models provider: on-device LLM via Swift FFI bridge (lt-llm-apple)

**Configuration**
- `AppConfig`: TOML-based configuration (API keys, providers, hotkeys, UI preferences, LLM model override)
- `PersonalDictionary`: Custom terms and aliases

**Output**
- `OutputSink` trait: Abstract output destination
- `OutputMode`: Clipboard/Keyboard/Both

## Known Limitations

1. **Mac App Store**: The app uses the `macos-private-api` feature for transparent windows, which is not allowed in the Mac App Store. This is intentional for direct distribution.

2. **LLM Post-Processing**: Multiple options are available — no single tool is required:
   - **HTTP APIs** (OpenAI, Claude, Gemini API, or custom OpenAI-compatible endpoints) — just provide an API key in Settings
   - **Apple Foundation Models** — on-device processing, no API key or external tools needed
   - **CLI tools** — `gemini-cli` (default: `gemini-3-flash-preview`) or `copilot-cli` (default: `gpt-5-mini`) for local CLI-based processing

   You can override the default model in Settings > LLM Processor > Model Override, or by setting `llm_model` in `config.toml`. If no LLM provider is configured, the app will output raw transcriptions without post-processing.

3. **API Keys Required**: You must provide your own API keys for cloud STT providers (ElevenLabs, OpenAI, or Groq). Apple STT is a free on-device alternative that requires no API key.

4. **Apple STT**: Requires macOS 26+ for downloading speech recognition models on-device.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding conventions, and how to submit changes.
