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

### Phase 1 (Complete - Foundation)
- ✅ Tauri 2 + Svelte 5 workspace scaffolding
- ✅ lt-core domain types and traits
- ✅ macOS floating overlay window (always-on-top, transparent, decorationless)
- ✅ Basic UI with status indicator
- ✅ Global shortcut support (Cmd+Shift+Space)
- ✅ Clipboard manager plugin integration

## Development

### Prerequisites
- Rust 1.92+
- Node.js 22+
- npm 11+

### Building

```bash
# Build lt-core independently
cargo build -p lt-core

# Run the full app in development mode
cd crates/lt-tauri
cargo tauri dev
```

### Configuration

Default configuration is stored in `config/default.toml`. User configuration will be stored at:
- macOS: `~/.config/localtype/config.toml`

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

1. **Global Shortcut**: Currently, the global shortcut (Cmd+Shift+Space) may fail to register on macOS due to system conflicts or permission requirements. The app will still launch and function, but you'll need to use window controls instead.

2. **Transparent Window**: The window transparency requires the `macos-private-api` feature, which is currently disabled to allow development. Full transparency will be enabled in production builds.

## Next Steps

- Phase 2: Audio capture (cpal + VAD) and ElevenLabs STT integration
- Phase 3: LLM post-processing via gemini-cli
- Phase 4: Complete pipeline with clipboard output
- Phase 5: Multi-provider support and voice commands
