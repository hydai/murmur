# Murmur: Privacy-First BYOK Voice Typing App

## Context

Typeless 存在嚴重隱私問題：語音資料上傳到他們的雲端處理、轉交未公開的第三方 LLM providers、收集裝置識別碼、與廣告合作夥伴（LinkedIn 等）分享資料。Murmur 透過 BYOK（Bring Your Own Key）架構解決這個問題 — 語音資料直接送到使用者自選的 STT provider，AI 後處理透過本地 CLI 工具完成，不經任何第三方中轉。

## Architecture Overview

```
麥克風 → cpal 音訊擷取 → 重新取樣 16kHz mono
    → STT Provider (BYOK: ElevenLabs/OpenAI/Groq)
    → LLM 後處理 (Local CLI: gemini-cli/copilot-cli)
    → 輸出 (剪貼簿 / 模擬鍵盤輸入)
```

**Tech Stack**: Tauri 2 (Rust backend + Svelte 5 frontend)
**UI**: macOS 浮動 overlay 視窗 (always-on-top, transparent, decorationless)

## Rust Workspace 結構

```
murmur/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── lt-core/                  # Domain types + traits (zero external deps beyond serde/thiserror)
│   │   └── src/
│   │       ├── stt.rs            # SttProvider trait, TranscriptionEvent, AudioChunk
│   │       ├── llm.rs            # LlmProcessor trait, ProcessingTask, ProcessingOutput
│   │       ├── config.rs         # AppConfig (TOML-based)
│   │       ├── dictionary.rs     # PersonalDictionary, DictionaryEntry
│   │       ├── output.rs         # OutputSink trait
│   │       └── error.rs          # MurmurError
│   │
│   ├── lt-audio/                 # 音訊擷取 (cpal + rubato resampling + VAD)
│   ├── lt-stt/                   # STT providers
│   │   ├── elevenlabs.rs         # WebSocket streaming (Scribe v2, 150ms latency)
│   │   ├── openai.rs             # REST batch (Whisper API, $0.006/min)
│   │   ├── groq.rs               # REST batch (Whisper Turbo, 216x real-time)
│   │   └── chunker.rs            # 音訊分塊邏輯 (for REST APIs)
│   │
│   ├── lt-llm/                   # LLM 後處理 via CLI
│   │   ├── gemini.rs             # gemini -p "prompt" --output-format json
│   │   ├── copilot.rs            # copilot --prompt "prompt"
│   │   ├── executor.rs           # CLI process spawning + output parsing
│   │   └── prompts.rs            # Prompt templates (去填充詞/格式化/語氣/翻譯)
│   │
│   ├── lt-output/                # 輸出 (arboard clipboard + enigo keyboard sim)
│   ├── lt-pipeline/              # Pipeline orchestration + voice command detection
│   └── lt-tauri/                 # Tauri app (IPC commands, state, events, window config)
│
├── ui/                           # Svelte 5 + TypeScript frontend
│   └── src/
│       ├── components/
│       │   ├── overlay/          # FloatingOverlay, TranscriptionView, WaveformIndicator, ControlBar
│       │   └── settings/         # SettingsPanel, ProviderConfig, LlmConfig, DictionaryEditor
│       └── lib/stores/           # Svelte stores (recording, transcription, settings, dictionary)
│
├── config/default.toml           # 預設設定模板
└── prompts/                      # LLM prompt templates
    ├── post_process.md           # 去填充詞 + 修正文法 + 自動格式化
    ├── shorten.md                # 語音指令：縮短
    ├── change_tone.md            # 語音指令：改語氣
    ├── generate_reply.md         # 語音指令：產生回覆
    └── translate.md              # 翻譯
```

## 核心設計決策

### 1. STT Provider 統一介面

`SttProvider` trait 統一 WebSocket streaming 和 REST batch 兩種模式：

- **ElevenLabs**: `send_audio()` 即時透過 WebSocket 送出 base64 編碼音訊，背景 task 接收 `partial_transcript` / `committed_transcript` 事件
- **OpenAI / Groq**: `send_audio()` 累積到內部 buffer，每 3-5 秒定時 flush — 編碼成 WAV (hound) → REST POST multipart → 解析 JSON 回傳

### 2. LLM 後處理透過 CLI subprocess

不直接呼叫 API，而是 spawn local CLI process：
- `gemini -p "prompt" --output-format json -m gemini-2.5-flash`
- `copilot --prompt "prompt"`

Prompt 包含：task instruction + raw transcription + personal dictionary terms。啟動時 `health_check()` 驗證 CLI 是否安裝。

### 3. 音訊管線

```
cpal callback (OS audio thread, non-blocking try_send)
  → bounded channel (64)
  → Processing task: resample to 16kHz mono (rubato), RMS-based VAD
  → bounded channel (32)
  → Pipeline Orchestrator → SttProvider
```

### 4. Tauri Window 配置

Overlay window: `decorations: false`, `transparent: true`, `alwaysOnTop: true`, `skipTaskbar: true`。需啟用 `macos-private-api` feature flag（不影響直接分發，但無法上架 Mac App Store）。

### 5. 設定與儲存

- Config: `~/.config/murmur/config.toml` (API keys, provider 選擇, hotkey, UI 偏好)
- Dictionary: `~/.config/murmur/dictionary.json` (自定義術語 + aliases)
- 完全本地，無 telemetry，無 cloud sync

## Key Dependencies (verified 2026-02-12)

### Rust Crates

| Crate | Version | 用途 |
|-------|---------|------|
| `tauri` | 2.10.2 | App framework |
| `tauri-build` | 2.5.5 | Build integration |
| `tauri-plugin-global-shortcut` | 2.3.1 | Global hotkeys |
| `tauri-plugin-clipboard-manager` | 2.3.2 | Clipboard access |
| `cpal` | 0.17.1 | 跨平台音訊 I/O |
| `rubato` | 1.0.1 | Sample rate conversion |
| `hound` | 3.5.1 | WAV encoding (REST STT APIs) |
| `tokio-tungstenite` | 0.28.0 | WebSocket (ElevenLabs) |
| `reqwest` | 0.13.2 | HTTP client (OpenAI/Groq) |
| `arboard` | 3.6.1 | Clipboard |
| `enigo` | 0.6.1 | Keyboard simulation |
| `tokio` | 1.49.0 | Async runtime |
| `serde` | 1.0.228 | Serialization |
| `serde_json` | 1.0.149 | JSON |
| `toml` | 1.0.0 | TOML config |
| `thiserror` | 2.0.18 | Error types |
| `anyhow` | 1.0.101 | Error context |
| `async-trait` | 0.1.89 | Async trait support |
| `futures-util` | 0.3.31 | Stream utilities |
| `base64` | 0.22.1 | Audio encoding for WebSocket |
| `tracing` | 0.1.44 | Structured logging |
| `tracing-subscriber` | 0.3.22 | Log output |
| `directories` | 6.0.0 | Platform config paths |

### npm Packages

| Package | Version | 用途 |
|---------|---------|------|
| `svelte` | ^5.50.2 | UI framework |
| `@tauri-apps/api` | ^2.10.1 | Tauri JS API |
| `@tauri-apps/plugin-global-shortcut` | ^2 | Shortcut JS bindings |
| `@tauri-apps/plugin-clipboard-manager` | ^2 | Clipboard JS bindings |
| `vite` | ^6 | Build tool |
| `typescript` | ^5.7 | Type checking |

## Phased Implementation

### Phase 1: Foundation — 骨架 + 音訊 + 單一 STT
1. `cargo create-tauri-app` (Svelte template) → 重構成 workspace layout
2. 建立 `lt-core` (所有 domain types + traits)
3. 建立 `lt-audio` (cpal capture → resample → VAD)
4. 建立 `lt-stt` (ElevenLabs WebSocket client only)
5. Tauri 接線：global hotkey → start/stop → 前端顯示 partial transcription

**交付物**: 按 hotkey，講話，overlay 顯示即時轉寫文字

### Phase 2: LLM 後處理 + 輸出
1. 建立 `lt-llm` (gemini-cli adapter + prompt templates)
2. 建立 `lt-output` (clipboard + keyboard simulation)
3. 建立 `lt-pipeline` (orchestrator: audio → STT → LLM → output)
4. 前端顯示處理狀態 (recording → transcribing → processing → done)

**交付物**: 完整管線 — 講話 → 清理後文字進剪貼簿

### Phase 3: 多 Provider + 語音指令
1. 加入 OpenAI Whisper REST client + Groq Whisper REST client
2. Provider 選擇 UI
3. Voice command detection ("shorten this", "make it formal", "reply to", "translate to")
4. 加入 copilot-cli adapter

**交付物**: 可切換 3 種 STT provider，支援語音指令

### Phase 4: 翻譯 + 字典 + UI 打磨
1. Translation pipeline (偵測 "translate to [lang]" 指令 → LLM 翻譯)
2. Personal dictionary CRUD UI
3. 完整 Settings panel (API key 管理, hotkey 自訂, provider 設定)
4. Overlay UI polish (圓角半透明 + 拖曳 + waveform indicator + 動畫)
5. System tray integration

**交付物**: Feature-complete MVP

### Phase 5: Hardening + Distribution
1. macOS 權限處理 (Microphone + Accessibility permissions, Info.plist)
2. Error handling (WebSocket 斷線重連, CLI 不存在 graceful fallback)
3. 測試 (unit tests for traits, integration tests with mock servers via wiremock)
4. macOS `.dmg` bundle via Tauri

## Verification

每個 Phase 的驗證方式：

1. **Phase 1**: 執行 `cargo tauri dev` → 按 hotkey → 對麥克風說話 → overlay 即時顯示 ElevenLabs 轉寫文字
2. **Phase 2**: 說話結束後 → 文字經 gemini-cli 清理 → 結果出現在剪貼簿 (Cmd+V 驗證)
3. **Phase 3**: Settings 切換到 OpenAI/Groq → 同樣流程能運作；說 "shorten this: [長文]" → 得到縮短版
4. **Phase 4**: 說 "translate to Chinese: hello world" → 得到 "你好世界"；新增字典條目 → STT 輸出正確使用自定義術語
5. **Phase 5**: `cargo tauri build` → 產生 `.dmg` → 全新 macOS 安裝測試 → 權限 prompt 正常 → 全功能運作
