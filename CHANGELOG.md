# Changelog
## 0.2.18 (2026-09-08)

### Fixes

- survive a poisoned lock in the Apple STT bridge
- scope the ACL to the window that needs each command

## 0.2.17 (2026-09-08)

### Fixes

- reject unconfigured LLM providers instead of substituting defaults
- default every AppConfig field so a partial config.toml still loads
- compute each terminal state once so the store and the event agree
- keep the prompt editor's save confirmation visible
- stop the prompt selector from discarding unsaved edits
- load ElevenLabs languages when the key is entered during onboarding
- copy diagnostics in the order the panel displays them
- reap the sound playback children
- create config.toml on first launch so settings open only once
- drop the overlay listeners and the duplicate event with no counterpart
- remove the dead status plumbing
- drop write-only state, an unused binding and stale suppressions

## 0.2.16 (2026-09-08)

### Fixes

- replace disabled depends_on macos symbol in cask template (#111)
- harden recording lifecycle and application state (#113)
- address audit follow-ups for secrets, history, cancellation, and parsing (#114)
- keep transcripts out of logs, fail fast on rejected handshakes, and bound keyboard output (#115)

## 0.2.15 (2026-09-07)

### Fixes

- Update all Rust and UI dependencies. The bundled TLS stack now uses aws-lc-sys 0.45.0, which fixes RUSTSEC-2026-0044 through RUSTSEC-2026-0048; audio capture moves to cpal 0.18.2 and Tauri to 2.11.5.

## 0.2.14 (2026-06-24)

### Fixes

- prevent custom STT backpressure
- bound custom STT transcription backlog
- time out custom transcription requests

## 0.2.13 (2026-06-24)

### Features

- add diagnostics log tab
- normalize final output with OpenCC

### Fixes

- allow selecting custom STT endpoint
- harden diagnostics log capture
- use settings border token in diagnostics
- avoid repeated OpenCC init errors
- log character counts correctly

## 0.2.12 (2026-04-17)

### Features

- add IPC commands to edit prompt overrides
- add Prompts tab for editing LLM templates at runtime

### Fixes

- wire tray "Check for Updates" to open settings and run check

## 0.2.11 (2026-04-09)

### Features

- redesign settings UI with sidebar navigation and design tokens

## 0.2.10 (2026-03-21)

### Fixes

- migrate Vite 7→8 minifier from esbuild to oxc (#30)

## 0.2.9 (2026-02-26)

### Features

- add custom OpenAI-compatible STT endpoint support

### Fixes

- HistoryPanel always shows "Loading..." due to missing $state() runes

## 0.2.8 (2026-02-25)

### Features

- enforce Traditional Chinese (Taiwan) in all LLM prompts

### Fixes

- use ClientRequestBuilder for ElevenLabs WebSocket handshake
- use IntoClientRequest for ElevenLabs WebSocket handshake
- correct ElevenLabs WebSocket endpoint URL and message protocol
- use scribe_v2_realtime model and handle invalid_request response
- use scribe_v2_realtime model in Tauri STT factory
- send explicit commit signal before closing ElevenLabs WebSocket

## 0.2.7 (2026-02-25)

### Features

- add ElevenLabs multilingual STT support with language selector

## 0.2.6 (2026-02-25)

### Fixes

- preserve multilingual text in LLM post-processing prompt

## 0.2.5 (2026-02-15)

### Fixes

- grant core:window:allow-close permission for settings X button

## 0.2.4 (2026-02-15)

### Features

- add HTTP API LLM providers (OpenAI, Claude, Gemini API, Custom)
- redesign LLM settings UI with API provider sections
- update app logo and add microphone tray icon
- add auto-update support with tauri-plugin-updater

### Fixes

- add signing secrets to CI build step

## 0.2.3 (2026-02-15)

### Features

- add llm_model config field and processor model support
- add set_llm_model IPC command and wire factory
- add model selection input to LLM settings UI

## 0.2.2 (2026-02-14)

### Features

- improve LLM post-processing prompt for better transcription cleanup

## 0.2.1 (2026-02-14)

### Features

- add Apple Foundation Models as on-device LLM provider
- add transcription history with persistent storage and UI

### Fixes

- LLM hot-swap on settings change and trailing partial transcription loss
- resolve cargo fmt and clippy issues
- add history commands to Tauri ACL permissions allowlist
- use Tauri clipboard plugin for history copy button
- remove emoji prefixes from tray menu for consistent macOS style

## 0.1.4 (2026-02-14)

### Features

- replace visual overlay with audio cues for state feedback
- auto-open settings window on first launch

### Fixes

- embed prompt templates at compile time with include_str!()
- allow pipeline restart from Error/Done states
- restore clipboard content after test
- use partial transcription fallback and allow post-processing after stop

## 0.1.3 (2026-02-13)

### Features

- add Swift bridge for Apple SpeechTranscriber (Phase 1)
- add build.rs to link Swift bridge into lt-stt (Phase 2)
- add AppleSttProvider with FFI bindings and SttProvider impl (Phase 3)
- wire Apple STT into config, IPC, and pipeline (Phase 4)
- update settings UI for local Apple STT provider (Phase 5)

### Fixes

- move Swift runtime rpath from lt-stt to lt-tauri build.rs
- add missing ACL permissions for settings and Apple STT commands
- resolve "auto" locale before model download and improve download UX
- use cumulative audio timeline to prevent SpeechAnalyzer timestamp overlap
- break on STT error and force-stop pipeline to prevent stuck recording
- prevent use-after-free, hotkey double-trigger, and unclean STT shutdown

## 0.1.2 (2026-02-13)

### Features

- add Swift bridge for Apple SpeechTranscriber (Phase 1)
- add build.rs to link Swift bridge into lt-stt (Phase 2)
- add AppleSttProvider with FFI bindings and SttProvider impl (Phase 3)
- wire Apple STT into config, IPC, and pipeline (Phase 4)
- update settings UI for local Apple STT provider (Phase 5)

### Fixes

- move Swift runtime rpath from lt-stt to lt-tauri build.rs
- add missing ACL permissions for settings and Apple STT commands
- resolve "auto" locale before model download and improve download UX
- use cumulative audio timeline to prevent SpeechAnalyzer timestamp overlap
- break on STT error and force-stop pipeline to prevent stuck recording
- prevent use-after-free, hotkey double-trigger, and unclean STT shutdown

## 0.1.1 (2026-02-13)

### Fixes

- handle knope "no release" exit gracefully in prepare-release workflow
- add missing branch and push steps to knope prepare-release workflow
- update branch references from main to master
- fix knope prepare-release workflow
