import { describe, expect, it, vi } from 'vitest';
import { createStatus } from '../lib/status.svelte';

/** A lifecycle stand-in that runs timers on demand. */
function fakeLifecycle() {
  const pending = new Map<string | symbol, () => void>();
  return {
    lifecycle: {
      get disposed() {
        return false;
      },
      listen: vi.fn(),
      onCleanup: vi.fn(),
      cancelTimeout: (key: string | symbol) => pending.delete(key),
      timeout(callback: () => void, _delay: number, key: string | symbol = Symbol()) {
        pending.set(key, callback);
      },
    },
    fire: () => {
      for (const callback of pending.values()) callback();
      pending.clear();
    },
    pendingKeys: () => [...pending.keys()],
  };
}

describe('createStatus', () => {
  it('reports a rejection against the description the caller gave', async () => {
    const { lifecycle } = fakeLifecycle();
    const status = createStatus(lifecycle as never);
    vi.spyOn(console, 'error').mockImplementation(() => {});

    const result = await status.run('Failed to save', async () => {
      throw new Error('boom');
    });

    expect(result).toBeUndefined();
    expect(status.error).toBe('Failed to save: Error: boom');
    expect(status.busy).toBe(false);
  });

  it('clears both banners before running and stays busy only while awaiting', async () => {
    const { lifecycle } = fakeLifecycle();
    const status = createStatus(lifecycle as never);
    status.fail('stale');
    status.confirm('older');

    let busyDuringJob = false;
    const value = await status.run('Failed', async () => {
      busyDuringJob = status.busy;
      expect(status.error).toBe('');
      expect(status.success).toBe('');
      return 42;
    });

    expect(value).toBe(42);
    expect(busyDuringJob).toBe(true);
    expect(status.busy).toBe(false);
  });

  it('schedules confirmations under one key so a stale timer cannot clear a newer banner', () => {
    const { lifecycle, fire, pendingKeys } = fakeLifecycle();
    const status = createStatus(lifecycle as never);

    status.confirm('first');
    status.confirm('second');
    expect(pendingKeys()).toEqual(['success']);
    expect(status.success).toBe('second');

    fire();
    expect(status.success).toBe('');
  });

  it('only auto-clears a failure when asked to', () => {
    const { lifecycle, fire, pendingKeys } = fakeLifecycle();
    const status = createStatus(lifecycle as never);

    status.fail('stays');
    expect(pendingKeys()).toEqual([]);

    status.fail('goes', 1000);
    expect(pendingKeys()).toEqual(['error']);
    fire();
    expect(status.error).toBe('');
  });
});
