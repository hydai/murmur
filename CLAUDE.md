# Localtype - Project Instructions

## Overview

Privacy-first BYOK voice typing app built with Tauri 2 + Svelte 5. Rust backend, TypeScript frontend.

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
- `crates/lt-tauri/` - Tauri app, system tray, IPC commands, pipeline orchestration
- `ui/` - Svelte 5 + TypeScript frontend
- `ui/src/components/overlay/` - FloatingOverlay (main UI), WaveformIndicator, TranscriptionView
- `ui/src/components/settings/` - SettingsPanel and provider configuration tabs
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
- The overlay window dynamically resizes: 600x200 (compact overlay) ↔ 800x600 (settings panel open)

### Tauri Events
- Rust emits events like `audio-level`, `recording-state`, `pipeline-state`, `open-settings`
- The frontend listens for these in `FloatingOverlay.svelte`'s `onMount`
- The `open-settings` event is emitted from the system tray menu in `main.rs`

## Common Pitfalls

- The window starts at 600x200 (set in `tauri.conf.json`). Settings panel needs the window resized to 800x600 first.
- Tauri IPC may not be ready immediately on startup — always use `safeInvoke()`.
- The `macos-private-api` feature is required for transparent windows (not Mac App Store compatible).
