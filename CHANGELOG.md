# Changelog
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
