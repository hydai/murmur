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

  // Build a diagnostic error message
  const tauriObj = (window as any).__TAURI_INTERNALS__;
  let detail: string;
  if (tauriObj === undefined) {
    detail = '__TAURI_INTERNALS__ is not defined. '
      + 'This usually means the app is running in a regular browser instead of the Tauri webview. '
      + 'Start the app with `cargo tauri dev` so the IPC bridge is injected.';
  } else if (typeof tauriObj.invoke !== 'function') {
    detail = `__TAURI_INTERNALS__ exists but .invoke is ${typeof tauriObj.invoke}. `
      + 'The IPC bridge may not have finished initializing.';
  } else {
    detail = 'Unknown IPC failure.';
  }

  throw new Error(`Tauri IPC not available after ${MAX_ATTEMPTS * INTERVAL_MS}ms (cmd: ${cmd}). ${detail}`);
}
