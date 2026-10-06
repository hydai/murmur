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

# Frontend type checking and regression tests
npm --prefix ui run check
npm --prefix ui test

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
- `crates/lt-stt/` - STT providers (ElevenLabs, OpenAI, Groq, Custom, Apple wrapper)
- `crates/lt-stt-apple/` - Swift FFI bridge for Apple SpeechTranscriber (on-device STT)
- `crates/lt-tauri/` - Tauri app, menu bar, IPC commands
- `crates/lt-tauri/src/storage.rs` - Serialized document transactions; file I/O runs on blocking workers
- `crates/lt-tauri/src/events.rs` - One app-lifetime pipeline event forwarder
- `crates/lt-tauri/src/capsule.rs` - Places, shows, and hides the recording capsule window (label `main`)
- `crates/lt-tauri/permissions/default.toml` - ACL command allowlist (update when adding IPC commands)
- `ui/` - Svelte 5 + TypeScript frontend
- `ui/src/components/capsule/` - RecordingCapsule (the recording indicator) and its capsuleState reducer
- `ui/src/components/history/` - HistoryPanel (transcription history with search)
- `ui/src/components/settings/` - SettingsPanel (the single Murmur window: a sidebar plus six panes, History included)
- `ui/src/components/ui/` - Shared components (Pane, Group, Row, Sheet, Select, Switch, Toast, SearchField, ShortcutField)
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
- Settings and History share one Murmur window (760×560, min 640×460)

### LLM Model Configuration
- Each LLM processor has a `DEFAULT_MODEL` constant and `with_model(Option<String>)` constructor
- CLI defaults: Gemini → `gemini-3-flash-preview`, Copilot → `gpt-5-mini`, Apple → system default
- HTTP API defaults: OpenAI → `gpt-4o-mini`, Claude → `claude-sonnet-4-20250514`, Gemini API → `gemini-2.0-flash`
- `AppConfig.llm_model` stores the user override (`None` = use provider default)
- `create_llm_processor()` in `main.rs` is the single factory — accepts `(type, model, config, prompts)` and is used at startup and before each recording
- Config setters persist through `AppStore`; `start_pipeline` applies a fresh LLM, output, dictionary, and prompt snapshot for the next recording
- `set_llm_model` saves the next recording's model; empty string resets to default
- When adding a new LLM provider: add `DEFAULT_MODEL`, `with_model()`, and update the factory + `get_llm_processors()`

### HTTP API LLM Providers
- `HttpLlmProcessor` in `crates/lt-llm/src/http_api.rs` — single struct with `ApiFormat` enum for OpenAI/Claude/Gemini API formats
- API keys stored in `AppConfig.api_keys` HashMap: `"openai"` (shared with STT), `"anthropic"`, `"google_ai"`, `"custom_llm"`
- `HttpLlmConfig` in `AppConfig` stores `custom_base_url` and `custom_display_name` for custom endpoints
- Custom endpoint uses OpenAI-compatible format (works with Ollama, LM Studio, Azure OpenAI)
- `set_custom_llm_endpoint` IPC command saves custom endpoint config

### STT Language Configuration
- Apple STT: `AppConfig.apple_stt_locale` — locale codes like `"en_US"`, `"ja_JP"`; `"auto"` detects system locale
- ElevenLabs: `AppConfig.elevenlabs_language` — ISO 639-3 codes like `"eng"`, `"jpn"`, `"zho"`; `"auto"` omits `language_code` from the WebSocket URL so Scribe v2 auto-detects
- Both use `serde(default)` for backward-compatible deserialization of existing config files
- `get_elevenlabs_languages` IPC returns the full static list of 98 supported languages; `set_elevenlabs_language` persists to config
- No hot-swap needed — STT provider is recreated on every `start_pipeline` call, so language changes take effect on the next recording
- UI: Language select in the provider-in-use group of `ProviderConfig.svelte`

### Custom STT Endpoint
- `CustomSttProvider` in `crates/lt-stt/src/custom.rs` — OpenAI-compatible Whisper API client with configurable base URL
- API format: `POST {base_url}/audio/transcriptions` with multipart form (`file`, `model`, `response_format`, optional `language`)
- `HttpSttConfig` in `AppConfig` stores `custom_base_url`, `custom_display_name`, `custom_model`, `language`
- API key stored in `AppConfig.api_keys` HashMap with key `"custom_stt"` (optional — local servers don't need auth)
- `set_custom_stt_endpoint` IPC saves config; `configured` = has `custom_base_url` (not API key)
- Default model: `"whisper-1"`; works with whisper.cpp, faster-whisper, LocalAI, etc.
- No hot-swap needed — STT provider is recreated on every `start_pipeline` call

### Tauri Events
- Rust emits events like `audio-level`, `recording-state`, `pipeline-state`
- Additional events: `apple-stt-model-progress`, `transcription-partial`, `transcription-committed`, `pipeline-result`, `pipeline-error`, `command-detected`, `capsule-context`
- `RecordingCapsule.svelte` listens in its `onMount` for `pipeline-state`, `recording-state`, `command-detected`, `pipeline-result`, `pipeline-error`, `capsule-context`, and `audio-level`
- `recording-state`'s `is_recording` means the microphone is open: `true` when Recording begins, `false` once Stop closes it and on Processing, Done, Error, and Idle; Transcribing leaves the last value standing (`capture_signal` in `events.rs`, `stop_pipeline` in `main.rs`)
- `capsule-context` (payload `{ shortcut, output_mode, save_history }`) goes to the `main` window only, just before the capsule is shown
- `navigate` (payload `{ pane, action? }`) goes to the settings window only: `show_settings` in `main.rs` emits it to a window that is already open (a new window reads the same route from its URL), and `SettingsPanel.svelte` listens for it

### Pipeline State Machine
- States: Idle → Recording → Transcribing → Processing → Done / Error
- Reference: `crates/lt-pipeline/src/state.rs`
- Startup failure rolls back to Error. Terminal STT events stop capture before final processing; `reset()` cancels and joins session tasks before returning to Idle.
- The hotkey and the menu bar call `toggle_recording`, which picks Start / Stop / Cancel from `recording::toggle_action(state, is_capturing)`; Cancel runs `reset()`, so a session that is finishing or processing can always be abandoned.
- A toggle that fails (no API key, say) goes through `report_toggle_failure` in `main.rs`, which always emits `pipeline-error` and, unless the indicator is off or a recording is already under way, brings the capsule up to show it for 4 s.
- Create the event forwarder once in app setup, never once per recording. Reset accumulated event data when Recording begins.
- OpenAI, Groq, and Custom STT share the bounded HTTP worker in `crates/lt-stt/src/http.rs`.

### Logging
- Never log transcript, prompt, or provider payload content at any level; log sizes (`chars = ...`, `bytes = ...`) instead. Diagnostics keep WARN/ERROR and stdout keeps everything, so content in a log line is user data on disk or in a terminal.
- `main.rs` installs a default `lt_*=debug` filter; `RUST_LOG` overrides it.

### Persistence and Tests
- Read and mutate config, history, and dictionary through the shared `AppStore`; do not add independent read-modify-write sequences in IPC commands.
- `get_config` returns `AppConfig::redacted()` (no `api_keys`) and `save_config` applies the copy with `apply_redacted`, so keys only change through `save_api_key`. Data files are owner-only (0600); `AppStore::new` tightens files written by earlier releases.
- `AppConfig.save_history` (default true) gates history writes: `set_save_history` persists it and flips `HistoryStore::set_enabled`, which drops appends while disabled; main applies the stored value at startup.
- `AppConfig.chinese_conversion` (`traditional` default, or `none`) is snapshotted per recording via `set_chinese_conversion`; `text_normalization::finalize_output` applies it and never converts a translation whose target is Simplified Chinese.
- File replacements use `lt_core::persistence::atomic_write`. Corrupt or unreadable files must not silently become empty documents.
- Prompt disk and memory updates share an owned write guard, including when the command caller is cancelled.
- Desktop-mutating tests and tests requiring installed CLI tools are opt-in (`#[ignore]`); normal tests use fake providers/processes and local HTTP servers.
- Run `npm run check`, `npm test`, and `npm run build` in `ui` for frontend changes.

### Voice Commands
- Detection: `crates/lt-pipeline/src/commands.rs` (`detect_command()`)
- Prefixes: `"shorten:"`, `"make it formal:"`, `"make it casual:"`, `"reply to:"`, `"translate to [language]:"`
- Default (no prefix): PostProcess with dictionary terms
- Empty content after a prefix, or a translate language longer than three words, falls back to PostProcess; prefixes match ASCII case-insensitively on char boundaries.

### Output Modes
- `OutputMode` in `crates/lt-core/src/output.rs`: Clipboard (default), Keyboard, Both
- Implementations in `crates/lt-output/src/`

### Settings Window
- One Murmur window (label `settings`) with a sidebar and six panes: General, Transcription, AI Processing, Dictionary, History, About
- Component files are in `ui/src/components/settings/`, except History's `HistoryPanel.svelte` in `ui/src/components/history/`; shared components are in `ui/src/components/ui/`
- General's Recording group has the "Show recording indicator" switch (`set_show_recording_indicator`), which turns the recording capsule on or off
- Voice Commands rows in AI Processing open the prompt editor (`PromptsEditor.svelte`)
- About includes the auto-updater (`@tauri-apps/plugin-updater`); the Diagnostics Log is a subpage of About
- Colors and fonts come from the tokens in `ui/src/lib/design-tokens.css`, which follow the system light/dark appearance; brand blue marks selection and primary actions: light mode uses `#1C74B8` for both, dark mode uses `#4BA8E8` for selection text and icons and keeps `#1C74B8` for the fills of primary buttons and switches

### Recording Capsule
- `RecordingCapsule.svelte` is what the `main` window shows (a different window from the Murmur window, label `settings`): a dark capsule with a timer and level bars while recording, then the pipeline's progress and how it ended. `tauri.conf.json` declares the window as 440×72, transparent, always on top, and never focusable, and `main.rs` makes it click-through
- Rust alone shows and hides the window (`crates/lt-tauri/src/capsule.rs`), so a capsule that misbehaves in the webview cannot stay on screen. When a recording starts it places the window at the bottom centre of the work area of the display under the pointer (computed in points), sends `capsule-context`, and shows it, all before `pipeline-state: recording` is emitted, because the capsule fades in on that event
- The window hides after the session ends: Done 1.5 s and Idle 1 s (4 s for either if the session reported an error), Error 4 s. A new recording cancels a pending hide. `HIDE_AFTER_MS` in `capsuleState.ts` is the same schedule (the capsule's 200 ms fade-out ends as the window hides), so change both together
- The frontend only draws content and animation and calls no IPC; the `main` window's capability grants only `core:default`
- Never call `set_focus()` on `main`: the window never takes focus, so the text the pipeline types goes to the app the user is working in
- `AppConfig.show_recording_indicator` (default true) is read once per recording, from the config snapshot `start_pipeline` configures the recording with (it hands the capsule its context and this switch through `Capsule::prepare`), so a change applies from the next recording; off keeps the capsule from showing, a failed toggle's error included (a config.toml that cannot be read counts as on, so that failure still shows)

## Common Pitfalls

- Tauri IPC may not be ready immediately on startup — always use `safeInvoke()`.
- Use `writeText()` from `@tauri-apps/plugin-clipboard-manager` for clipboard writes — `navigator.clipboard` doesn't work reliably in Tauri webviews.
- The `macos-private-api` feature is required for transparent windows (not Mac App Store compatible).
- Apple STT requires macOS 26+ for speech recognition model downloads.
