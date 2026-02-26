<p align="center">
  <img src="murmur.png" alt="Murmur" width="128" />
</p>

# Murmur

Privacy-first BYOK (Bring Your Own Key) voice typing application built with Tauri 2 and Svelte 5.

## Features

### Speech-to-Text

- **Cloud providers**: ElevenLabs, OpenAI Whisper, Groq — bring your own API key
- **On-device**: Apple Speech recognition (macOS, no API key needed)
- **Self-hosted**: any OpenAI-compatible Whisper API (whisper.cpp, faster-whisper, LocalAI)

### LLM Post-Processing

- **Cloud APIs**: OpenAI, Claude, Gemini, or custom OpenAI-compatible endpoints
- **On-device**: Apple Foundation Models — no API key needed
- **CLI tools**: gemini-cli, copilot-cli for local processing
- **Voice commands**: shorten, translate, change tone, generate replies
- **Personal dictionary** for custom terms and aliases

### Interface

- Floating glassmorphism overlay with waveform visualization
- System tray with configurable global hotkey (default `Ctrl+``)
- Transcription history with search and persistent storage
- Output to clipboard, keyboard simulation, or both
- Audio cues for recording start/stop and errors

### Privacy

- BYOK — all data goes directly to your chosen providers
- On-device alternatives for both STT and LLM require no cloud at all
- Auto-updater for seamless in-app updates

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
│   ├── lt-stt/                   # STT providers (ElevenLabs, OpenAI, Groq, Custom)
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

## Installation

### Homebrew (recommended)

```bash
brew tap hydai/murmur
brew install --cask murmur
```

After installation, clear the quarantine attribute (unsigned app):
```bash
xattr -cr /Applications/Murmur.app
```

### Manual

1. Download the `.dmg` from [Releases](https://github.com/hydai/murmur/releases)
2. Drag Murmur.app to Applications
3. Clear the quarantine attribute (unsigned app):
   ```bash
   xattr -cr /Applications/Murmur.app
   ```

### First launch

1. Grant microphone and accessibility permissions when prompted
2. Configure your providers in Settings (system tray → Settings)

## Development

### Prerequisites
- Rust 1.92+
- Node.js 22+
- npm 11+

### Build & Run

```bash
cargo tauri dev              # Development mode
cargo test --workspace       # Run all tests (~120)
cargo build -p lt-tauri --release  # Release binary
cargo tauri build            # Production bundle (.dmg)
```

### Configuration

User config: `~/Library/Application Support/com.hydai.Murmur/config.toml`

Default template: `config/default.toml`

## Known Limitations

- **Not on Mac App Store** — uses `macos-private-api` for transparent windows (direct distribution only)
- **BYOK** — cloud providers require your own API keys; on-device and self-hosted options need none
- **Apple STT** — requires macOS 26+ for on-device speech recognition models

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding conventions, and how to submit changes.
