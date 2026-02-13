import { invoke } from '@tauri-apps/api/core';

/**
 * Waits for Tauri's IPC bridge to be available, then calls invoke.
 * Prevents "Cannot read properties of undefined (reading 'invoke')" errors
 * when components mount before __TAURI_INTERNALS__ is injected.
 */
export async function safeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const MAX_ATTEMPTS = 50;
  const INTERVAL_MS = 100;

  for (let i = 0; i < MAX_ATTEMPTS; i++) {
    if ((window as any).__TAURI_INTERNALS__?.invoke) {
      return invoke<T>(cmd, args);
    }
    await new Promise((resolve) => setTimeout(resolve, INTERVAL_MS));
  }

  throw new Error(`Tauri IPC bridge not available after ${MAX_ATTEMPTS * INTERVAL_MS}ms (cmd: ${cmd})`);
}
