import type { Action } from 'svelte/action';

const TABBABLE = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

function tabbables(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(TABBABLE)]
    .filter((element) => !element.hidden && element.getAttribute('aria-hidden') !== 'true');
}

/**
 * Focus management for modal dialogs: move focus into the dialog when it
 * opens, keep Tab cycling inside it, and return focus to the opener when it
 * closes. `aria-modal` alone does none of this, so without it Tab keeps
 * walking the page behind.
 *
 * Focus can also leave the dialog without the user choosing to: a press on a
 * part of the page that cannot take focus, or a button that is disabled while
 * it holds the focus, leaves it on the body. A key pressed there never passes
 * through the dialog, so Tab is also heard on the document, and brings the
 * focus back in.
 */
export const trapFocus: Action<HTMLElement> = (node) => {
  const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  if (!node.hasAttribute('tabindex')) node.setAttribute('tabindex', '-1');
  (tabbables(node)[0] ?? node).focus();

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== 'Tab') return;
    const items = tabbables(node);
    if (items.length === 0) {
      event.preventDefault();
      node.focus();
      return;
    }
    const first = items[0];
    const last = items[items.length - 1];
    const active = document.activeElement;
    if (event.shiftKey && (active === first || active === node)) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && (active === last || active === node)) {
      event.preventDefault();
      first.focus();
    }
  }

  function onDocumentKeydown(event: KeyboardEvent) {
    if (event.key !== 'Tab') return;
    // From inside, `onKeydown` has the key: it lets Tab move on, or wraps it at the ends.
    if (node.contains(document.activeElement)) return;
    event.preventDefault();
    const items = tabbables(node);
    const target = event.shiftKey ? items[items.length - 1] : items[0];
    (target ?? node).focus();
  }

  node.addEventListener('keydown', onKeydown);
  document.addEventListener('keydown', onDocumentKeydown);
  return {
    destroy() {
      node.removeEventListener('keydown', onKeydown);
      document.removeEventListener('keydown', onDocumentKeydown);
      if (opener?.isConnected) opener.focus();
    },
  };
};
