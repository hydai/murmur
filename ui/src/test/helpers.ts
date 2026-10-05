import { expect } from 'vitest';
import { flushSync, mount, tick, unmount, type Component } from 'svelte';

let mounted: ReturnType<typeof mount>[] = [];

/** Mount a component into the document and flush its first render. */
export function render<P extends Record<string, unknown>>(component: Component<P>, props: P) {
  const target = document.createElement('div');
  document.body.append(target);
  const instance = mount(component, { target, props });
  mounted.push(instance);
  flushSync();
  return { target, instance };
}

/** Let async effects, such as sequential IPC listener registration, and Svelte DOM flushes finish. */
export async function settle() {
  for (let i = 0; i < 20; i++) await tick();
}

/** The first button inside `root` whose text contains `text`. */
export function button(root: Element, text: string): HTMLButtonElement {
  const result = [...root.querySelectorAll('button')].find(b => b.textContent?.includes(text));
  expect(result, `button ${text}`).toBeDefined();
  return result!;
}

/** Unmount everything `render` mounted and empty the document. */
export async function unmountAll() {
  for (const instance of mounted) await unmount(instance);
  mounted = [];
  document.body.replaceChildren();
}
