import { invoke } from '@tauri-apps/api/core';

/**
 * Waits for Tauri's IPC bridge to be available, then calls invoke.
 * Prevents "Cannot read properties of undefined (reading 'invoke')" errors
 * when settings components mount before __TAURI_INTERNALS__ is injected.
 */
export async function safeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const MAX_ATTEMPTS = 10;
  const INTERVAL_MS = 100;

  for (let i = 0; i < MAX_ATTEMPTS; i++) {
    if ((window as any).__TAURI_INTERNALS__) {
      break;
    }
    await new Promise((resolve) => setTimeout(resolve, INTERVAL_MS));
  }

  return invoke<T>(cmd, args);
}
