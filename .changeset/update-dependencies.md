---
default: patch
---

Update all Rust and UI dependencies. The bundled TLS stack now uses aws-lc-sys 0.45.0, which fixes RUSTSEC-2026-0044 through RUSTSEC-2026-0048; audio capture moves to cpal 0.18.2 and Tauri to 2.11.5.
