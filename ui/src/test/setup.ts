import { vi } from 'vitest';

// Animation timing is unrelated to these state and event regression tests.
vi.mock('svelte/transition', () => ({
  fade: () => ({ duration: 0 }),
  fly: () => ({ duration: 0 }),
  slide: () => ({ duration: 0 }),
}));
