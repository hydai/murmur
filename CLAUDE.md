# Murmur - Project Instructions

## Overview

Privacy-first BYOK voice typing app built with Tauri 2 + Svelte 5. Rust backend, TypeScript frontend.

## Setup

```bash
# Enable the pre-commit hook (fmt + clippy checks)
git config core.hooksPath .githooks
```

## Build & Test

```bash
# Build Tauri app (always release mode)
cargo build -p lt-tauri --release

# Run all tests
cargo test --workspace

# Dev mode
cargo tauri dev

# Production bundle (.dmg)
cargo tauri build
```

## Project Structure

- `crates/lt-audio/` - Audio capture, cpal resampling, voice activity detection (VAD)
- `crates/lt-core/` - Domain types and traits (STT, LLM, config, dictionary, history, output)
- `crates/lt-llm/` - LLM processors (Gemini CLI, Copilot CLI, HTTP API for OpenAI/Claude/Gemini/custom)
- `crates/lt-llm-apple/` - Swift FFI bridge for Apple Foundation Models (on-device LLM)
- `crates/lt-output/` - Output routing (clipboard, keyboard simulation, combined mode)
- `crates/lt-pipeline/` - Pipeline orchestration, state machine, voice command detection
- `crates/lt-stt/` - STT providers (ElevenLabs, OpenAI, Groq, Apple wrapper)
- `crates/lt-stt-apple/` - Swift FFI bridge for Apple SpeechTranscriber (on-device STT)
- `crates/lt-tauri/` - Tauri app, system tray, IPC commands
- `crates/lt-tauri/permissions/default.toml` - ACL command allowlist (update when adding IPC commands)
- `ui/` - Svelte 5 + TypeScript frontend
- `ui/src/components/history/` - HistoryPanel (transcription history with search)
- `ui/src/components/overlay/` - FloatingOverlay (main UI), WaveformIndicator, TranscriptionView
- `ui/src/components/settings/` - SettingsPanel (standalone 720x560 window)
- `ui/src/lib/tauri.ts` - `safeInvoke()` wrapper that guards against IPC readiness
- `config/default.toml` - Default configuration template
- `prompts/` - LLM prompt templates for post-processing

## Key Conventions

### Rust
- Always use release builds (`cargo build --release`)
- Tauri IPC commands are defined in `crates/lt-tauri/src/main.rs`
- ACL capabilities are in `crates/lt-tauri/capabilities/`

### Frontend (Svelte 5)
- Use `safeInvoke()` from `ui/src/lib/tauri.ts` instead of raw `invoke()` — it guards against Tauri IPC not being ready
- Event listeners from Tauri use `listen()` from `@tauri-apps/api/event` — always clean up with unlisten in `onDestroy`
- Window operations use `getCurrentWindow()` and `LogicalSize` from `@tauri-apps/api/window`
- Settings is a standalone window (720x560), separate from the overlay

### LLM Model Configuration
- Each LLM processor has a `DEFAULT_MODEL` constant and `with_model(Option<String>)` constructor
- CLI defaults: Gemini → `gemini-3-flash-preview`, Copilot → `gpt-5-mini`, Apple → system default
- HTTP API defaults: OpenAI → `gpt-4o-mini`, Claude → `claude-sonnet-4-20250514`, Gemini API → `gemini-2.0-flash`
- `AppConfig.llm_model` stores the user override (`None` = use provider default)
- `create_llm_processor()` in `main.rs` is the single factory — accepts `(type, model, config)` and is used at both startup and hot-swap
- `set_llm_model` IPC command saves config and hot-swaps the processor; empty string resets to default
- When adding a new LLM provider: add `DEFAULT_MODEL`, `with_model()`, and update the factory + `get_llm_processors()`

### HTTP API LLM Providers
- `HttpLlmProcessor` in `crates/lt-llm/src/http_api.rs` — single struct with `ApiFormat` enum for OpenAI/Claude/Gemini API formats
- API keys stored in `AppConfig.api_keys` HashMap: `"openai"` (shared with STT), `"anthropic"`, `"google_ai"`, `"custom_llm"`
- `HttpLlmConfig` in `AppConfig` stores `custom_base_url` and `custom_display_name` for custom endpoints
- Custom endpoint uses OpenAI-compatible format (works with Ollama, LM Studio, Azure OpenAI)
- `set_custom_llm_endpoint` IPC command saves custom endpoint config

### Tauri Events
- Rust emits events like `audio-level`, `recording-state`, `pipeline-state`, `open-settings`
- Additional events: `apple-stt-model-progress`, `transcription-partial`, `transcription-committed`, `pipeline-result`, `pipeline-error`, `command-detected`
- The frontend listens for these in `FloatingOverlay.svelte`'s `onMount`
- The `open-settings` event is emitted from the system tray menu in `main.rs`

### Pipeline State Machine
- States: Idle → Recording → Transcribing → Processing → Done / Error
- Reference: `crates/lt-pipeline/src/state.rs`

### Voice Commands
- Detection: `crates/lt-pipeline/src/commands.rs` (`detect_command()`)
- Prefixes: `"shorten:"`, `"make it formal:"`, `"make it casual:"`, `"reply to:"`, `"translate to [language]:"`
- Default (no prefix): PostProcess with dictionary terms

### Output Modes
- `OutputMode` in `crates/lt-core/src/output.rs`: Clipboard (default), Keyboard, Both
- Implementations in `crates/lt-output/src/`

### Settings Window
- 6 tabs: STT Providers, LLM Processor, Hotkey, Output, Dictionary, About
- Component files in `ui/src/components/settings/`
- About tab includes auto-updater (`@tauri-apps/plugin-updater`)

## Common Pitfalls

- Tauri IPC may not be ready immediately on startup — always use `safeInvoke()`.
- Use `writeText()` from `@tauri-apps/plugin-clipboard-manager` for clipboard writes — `navigator.clipboard` doesn't work reliably in Tauri webviews.
- The `macos-private-api` feature is required for transparent windows (not Mac App Store compatible).
- Apple STT requires macOS 26+ for speech recognition model downloads.
