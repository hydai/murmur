# Changelog
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
