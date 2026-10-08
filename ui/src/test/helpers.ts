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

/** Type into a field: set its value, then send the input event Svelte listens for. */
export function fill(input: HTMLInputElement | HTMLTextAreaElement, value: string) {
  input.value = value;
  input.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
}

/** The sheet or confirmation on show, if there is one. */
export function dialog(target: Element) {
  return target.querySelector<HTMLElement>('[role="dialog"]');
}

/** The title of the sheet or confirmation on show. */
export function dialogTitle(target: Element) {
  return dialog(target)?.querySelector('h2')?.textContent;
}

/** What the error toast says, if one is showing. */
export function errorToast(target: Element) {
  return target.querySelector('.toast-error')?.textContent;
}

/** What the success toast says, if one is showing. */
export function successToast(target: Element) {
  return target.querySelector('.toast-success')?.textContent?.trim();
}

/** The rows marked as the one in use. */
export function inUse(target: Element) {
  return [...target.querySelectorAll('[aria-current="true"]')];
}

/** Unmount everything `render` mounted and empty the document. */
export async function unmountAll() {
  for (const instance of mounted) await unmount(instance);
  mounted = [];
  document.body.replaceChildren();
}
