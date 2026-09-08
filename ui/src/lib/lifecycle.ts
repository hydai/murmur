import { onDestroy } from 'svelte';
import { listen, type EventCallback, type EventName } from '@tauri-apps/api/event';

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
    async listen<T>(event: EventName, callback: EventCallback<T>) {
      if (disposed) return;
      const unlisten = await listen<T>(event, (payload) => {
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
