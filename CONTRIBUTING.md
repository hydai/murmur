# Contributing to Murmur

Thank you for your interest in contributing to Murmur! This document covers the development setup, coding conventions, and how to submit changes.

## Development Setup

### Prerequisites

- **Rust** 1.92+ (`rustup update stable`)
- **Node.js** 22+ and **npm** 11+
- **Tauri CLI**: `cargo install tauri-cli`
- **macOS** (required for CoreAudio, system tray, and accessibility APIs)
- **Xcode Command Line Tools**: `xcode-select --install`

### Getting Started

```bash
# Clone the repo
git clone https://github.com/hydai/murmur.git
cd murmur

# Install frontend dependencies
cd ui && npm ci && cd ..

# Run all tests
cargo test --workspace

# Run the app in development mode
cargo tauri dev
```

### Project Structure

- `crates/lt-core/` — Domain types and traits
- `crates/lt-audio/` — Audio capture (cpal + resampling + VAD)
- `crates/lt-stt/` — STT providers (ElevenLabs, OpenAI, Groq)
- `crates/lt-llm/` — LLM post-processing via CLI
- `crates/lt-output/` — Output (clipboard + keyboard simulation)
- `crates/lt-pipeline/` — Pipeline orchestration + voice commands
- `crates/lt-tauri/` — Tauri app (IPC, state, events, window)
- `ui/` — Svelte 5 + TypeScript frontend

## Coding Conventions

### Rust

- Always use release builds for testing performance: `cargo build -p lt-tauri --release`
- Use `thiserror` for error types (see `MurmurError` in `lt-core`)
- Use `tracing` for logging, not `println!`
- Run `cargo clippy` and `cargo fmt` before submitting

### Frontend (Svelte 5 + TypeScript)

- Use `safeInvoke()` from `ui/src/lib/tauri.ts` instead of raw Tauri `invoke()`
- Clean up event listeners in `onDestroy`

### Commits

Follow [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` — new feature
- `fix:` — bug fix
- `docs:` — documentation only
- `refactor:` — code change that neither fixes a bug nor adds a feature
- `chore:` — build, CI, or tooling changes
- `ci:` — CI configuration

Keep commit messages concise and focused on one concern per commit.

## Submitting Changes

### Pull Request Workflow

1. **Fork** the repository and create a feature branch from `main`
2. **Write tests** for any new functionality
3. **Run the full test suite**: `cargo test --workspace`
4. **Run lints**: `cargo clippy` and `cargo fmt --check`
5. **Open a PR** against `main` with a clear description of changes

### PR Guidelines

- Keep PRs focused — one feature or fix per PR
- Include a description of what changed and why
- Reference any related issues
- Ensure CI passes before requesting review

## Reporting Issues

- Use the [bug report template](https://github.com/hydai/murmur/issues/new?template=bug_report.md) for bugs
- Use the [feature request template](https://github.com/hydai/murmur/issues/new?template=feature_request.md) for ideas

## License

By contributing to Murmur, you agree that your contributions will be licensed under the [Apache License 2.0](LICENSE).
