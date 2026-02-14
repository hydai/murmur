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

- `crates/lt-core/` - Domain types and traits (STT, LLM, config, dictionary, output)
- `crates/lt-stt-apple/` - Swift FFI bridge for Apple SpeechTranscriber (on-device STT)
- `crates/lt-llm-apple/` - Swift FFI bridge for Apple Foundation Models (on-device LLM)
- `crates/lt-tauri/` - Tauri app, system tray, IPC commands, pipeline orchestration
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

### Tauri Events
- Rust emits events like `audio-level`, `recording-state`, `pipeline-state`, `open-settings`
- Additional events: `apple-stt-model-progress`, `transcription-partial`, `transcription-committed`, `pipeline-result`, `pipeline-error`, `command-detected`
- The frontend listens for these in `FloatingOverlay.svelte`'s `onMount`
- The `open-settings` event is emitted from the system tray menu in `main.rs`

## Common Pitfalls

- Tauri IPC may not be ready immediately on startup — always use `safeInvoke()`.
- Use `writeText()` from `@tauri-apps/plugin-clipboard-manager` for clipboard writes — `navigator.clipboard` doesn't work reliably in Tauri webviews.
- The `macos-private-api` feature is required for transparent windows (not Mac App Store compatible).
- Apple STT requires macOS 26+ for speech recognition model downloads.
