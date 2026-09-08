import { onDestroy } from 'svelte';
import { listen } from '@tauri-apps/api/event';
import type { AppEventName, AppEvents } from './events';

/** Own subscriptions and delayed work for the lifetime of a component. */
export function useLifecycle() {
  let disposed = false;
  const cleanups = new Set<() => void>();
  const timers = new Map<string | symbol, ReturnType<typeof setTimeout>>();

  function cancelTimeout(key: string | symbol) {
    const timer = timers.get(key);
    if (timer !== undefined) clearTimeout(timer);
    timers.delete(key);
  }

  onDestroy(() => {
    disposed = true;
    for (const cleanup of cleanups) cleanup();
    cleanups.clear();
    for (const key of timers.keys()) cancelTimeout(key);
  });

  return {
    get disposed() { return disposed; },
    /**
     * Subscribe for the component's lifetime. Constrained to the event
     * contract, so a name the backend never emits, or a payload field that was
     * renamed, is a compile error rather than a silent no-op.
     */
    async listen<K extends AppEventName>(
      event: K,
      callback: (event: { payload: AppEvents[K] }) => void,
    ) {
      if (disposed) return;
      const unlisten = await listen<AppEvents[K]>(event, (payload) => {
        if (!disposed) callback(payload);
      });
      // IPC can finish after the component has already been destroyed.
      if (disposed) unlisten();
      else cleanups.add(unlisten);
    },
    onCleanup(cleanup: () => void) {
      if (disposed) cleanup();
      else cleanups.add(cleanup);
    },
    timeout(callback: () => void, delay: number, key: string | symbol = Symbol()) {
      cancelTimeout(key);
      if (disposed) return;
      timers.set(key, setTimeout(() => {
        timers.delete(key);
        if (!disposed) callback();
      }, delay));
    },
    cancelTimeout,
  };
}
