import { afterEach, describe, expect, it, vi } from 'vitest';
import { createRawSnippet, flushSync } from 'svelte';
import userEvent from '@testing-library/user-event';
import Pane from '../components/ui/Pane.svelte';
import SearchField from '../components/ui/SearchField.svelte';
import Sheet from '../components/ui/Sheet.svelte';
import ShortcutField from '../components/ui/ShortcutField.svelte';
import Toast from '../components/ui/Toast.svelte';
import { createStatus } from '../lib/status.svelte';
import { MISSING_MODIFIER_MESSAGE } from '../lib/shortcut';
import PaneHarness from './PaneHarness.svelte';
import SheetHarness from './SheetHarness.svelte';
import { button, render, unmountAll } from './helpers';

afterEach(async () => {
  await unmountAll();
  vi.useRealTimers();
});

/** True when `a` comes before `b` in document order. */
const precedes = (a: Node, b: Node) => Boolean(a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING);

const snippet = (html: string) => createRawSnippet(() => ({ render: () => html }));

describe('Sheet', () => {
  it('focuses the first field, closes on Escape, and ignores backdrop clicks', async () => {
    const onclose = vi.fn(); const onsubmit = vi.fn();
    const { target } = render(SheetHarness, { onclose, onsubmit });
    const dialog = target.querySelector<HTMLElement>('[role="dialog"]')!;
    expect(document.activeElement).toBe(dialog.querySelector('#term'));
    expect(target.querySelector(`#${dialog.getAttribute('aria-labelledby')}`)?.textContent).toBe('Add Word');
    target.querySelector<HTMLElement>('.sheet-backdrop')!.click();
    expect(onclose).not.toHaveBeenCalled();
    await userEvent.setup().keyboard('{Escape}');
    expect(onclose).toHaveBeenCalledTimes(1);
  });

  it('keeps focus in the sheet when the backdrop is pressed', async () => {
    const onclose = vi.fn(); const onsubmit = vi.fn();
    const { target } = render(SheetHarness, { onclose, onsubmit });
    // Tab reaches this only if the sheet's focus trap has let go of the focus.
    const outside = document.createElement('button');
    outside.textContent = 'Outside the sheet';
    document.body.append(outside);
    const dialog = target.querySelector<HTMLElement>('[role="dialog"]')!;
    const user = userEvent.setup();

    // A real press, mousedown and focus change included. The programmatic click
    // in the first test fires none of that, which is how this went unnoticed.
    await user.click(target.querySelector<HTMLElement>('.sheet-backdrop')!);
    expect(document.activeElement).toBe(dialog.querySelector('#term'));
    expect(onclose).not.toHaveBeenCalled();

    // The Tab trap still wraps both ways instead of letting focus out to the button.
    await user.tab({ shift: true });
    expect(document.activeElement).toBe(button(dialog, 'Add Word'));
    await user.tab();
    expect(document.activeElement).toBe(dialog.querySelector('#term'));

    await user.keyboard('{Enter}');
    expect(onsubmit).toHaveBeenCalledTimes(1);
    await user.keyboard('{Escape}');
    expect(onclose).toHaveBeenCalledTimes(1);
  });

  /** An Escape key press in the sheet's first field, carrying the properties an input method sets. */
  const escapeInField = (target: Element, { isComposing = false, keyCode = 27 } = {}) => {
    const event = new KeyboardEvent('keydown', { key: 'Escape', isComposing, bubbles: true });
    // jsdom leaves keyCode out of the init dictionary, so it has to be defined on the event itself.
    Object.defineProperty(event, 'keyCode', { get: () => keyCode });
    target.querySelector('#term')!.dispatchEvent(event);
  };

  it('ignores Escape while an input method is composing', () => {
    const onclose = vi.fn();
    const { target } = render(SheetHarness, { onclose, onsubmit: vi.fn() });
    // An input method cancels its candidate list with Escape. Closing the sheet
    // as well would throw away what the user was typing.
    escapeInField(target, { isComposing: true });
    expect(onclose).not.toHaveBeenCalled();
    // The same key without a composition still closes it, so the guard is the only difference.
    escapeInField(target);
    expect(onclose).toHaveBeenCalledTimes(1);
  });

  it('ignores an Escape the input method has already handled, marked by keyCode 229', () => {
    const onclose = vi.fn();
    const { target } = render(SheetHarness, { onclose, onsubmit: vi.fn() });
    // WebKit can end the composition before the key's keydown arrives, so the
    // event says isComposing: false and only keyCode 229 shows the key was the input method's.
    escapeInField(target, { keyCode: 229 });
    expect(onclose).not.toHaveBeenCalled();
    escapeInField(target);
    expect(onclose).toHaveBeenCalledTimes(1);
  });

  it('submits the sheet with Enter in a field', async () => {
    // Has to be user-event: it simulates a form's implicit submission, which a
    // KeyboardEvent dispatched by hand does not trigger in jsdom.
    const onsubmit = vi.fn();
    render(SheetHarness, { onclose: vi.fn(), onsubmit });
    await userEvent.setup().keyboard('{Enter}'); // focus is already in #term
    expect(onsubmit).toHaveBeenCalledTimes(1);
  });

  it('keeps a submitted form from navigating', () => {
    const onsubmit = vi.fn();
    const { target } = render(SheetHarness, { onclose: vi.fn(), onsubmit });
    const submit = new Event('submit', { bubbles: true, cancelable: true });
    target.querySelector('form')!.dispatchEvent(submit);
    expect(submit.defaultPrevented).toBe(true);
    expect(onsubmit).toHaveBeenCalledTimes(1);
  });

  it('keeps Escape from reaching anything behind the sheet', async () => {
    const behind = vi.fn();
    document.body.addEventListener('keydown', behind);
    try {
      render(SheetHarness, { onclose: vi.fn(), onsubmit: vi.fn() });
      await userEvent.setup().keyboard('{Escape}');
      expect(behind).not.toHaveBeenCalled();
    } finally {
      document.body.removeEventListener('keydown', behind);
    }
  });

  it('puts the error under the fields and a leading action before the buttons', () => {
    const props = {
      title: 'Edit Word',
      onclose: vi.fn(),
      children: snippet('<input id="term" />'),
      actions: snippet('<button type="submit">Save</button>'),
    };
    const { target } = render(Sheet, {
      ...props,
      error: 'Enter a word.',
      leading: snippet('<button type="button">Delete…</button>'),
    });
    const form = target.querySelector('form')!;
    const error = form.querySelector('.sheet-error')!;
    expect(error.getAttribute('role')).toBe('alert');
    expect(error.textContent).toBe('Enter a word.');
    expect(precedes(form.querySelector('#term')!, error)).toBe(true);
    expect(precedes(error, button(form, 'Delete…'))).toBe(true);
    expect(precedes(button(form, 'Delete…'), button(form, 'Save'))).toBe(true);

    expect(render(Sheet, props).target.querySelector('.sheet-error')).toBeNull();
  });
});

describe('Toast', () => {
  it('shows a success toast as status and an error toast as a dismissible alert', () => {
    const ondismiss = vi.fn();
    const success = render(Toast, { success: 'Saved', ondismiss }).target;
    expect(success.querySelector('.toast-success[role="status"]')?.textContent).toContain('Saved');
    const both = render(Toast, { success: 'Saved', error: 'Failed to save: boom', ondismiss }).target;
    expect(both.querySelector('.toast-success')).toBeNull();
    expect(both.querySelector('.toast-error[role="alert"]')?.textContent).toContain('Failed to save: boom');
    both.querySelector<HTMLButtonElement>('[aria-label="Dismiss"]')!.click();
    expect(ondismiss).toHaveBeenCalledTimes(1);
  });

  it('shows nothing without a message, and no dismiss button on a confirmation', () => {
    expect(render(Toast, { ondismiss: vi.fn() }).target.querySelector('.toast')).toBeNull();
    // A confirmation clears itself, so there is nothing to dismiss.
    expect(render(Toast, { success: 'Saved', ondismiss: vi.fn() }).target.querySelector('button')).toBeNull();
  });
});

describe('ShortcutField', () => {
  const setup = (props: { value?: string; disabled?: boolean } = {}) => {
    const onchange = vi.fn(); const onerror = vi.fn();
    const { target } = render(ShortcutField, { value: 'Ctrl+`', ...props, onchange, onerror });
    return { target, onchange, onerror, field: target.querySelector<HTMLButtonElement>('.shortcut')! };
  };
  const press = (init: KeyboardEventInit) => window.dispatchEvent(new KeyboardEvent('keydown', init));

  it('records a new shortcut and rejects one without a modifier', async () => {
    const onchange = vi.fn(); const onerror = vi.fn();
    const { target } = render(ShortcutField, { value: 'Ctrl+`', onchange, onerror });
    expect([...target.querySelectorAll('kbd')].map(k => k.textContent)).toEqual(['⌃', '`']);
    const field = target.querySelector<HTMLButtonElement>('.shortcut')!;
    field.click(); flushSync();
    expect(field.textContent).toContain('Type shortcut…');
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', metaKey: true, shiftKey: true }));
    expect(onchange).toHaveBeenCalledWith('Cmd+Shift+K');
    field.click(); flushSync();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k' }));
    expect(onerror).toHaveBeenCalledWith(MISSING_MODIFIER_MESSAGE);
  });

  it('cancels recording with Escape without saving', () => {
    const onchange = vi.fn(); const onerror = vi.fn();
    const { target } = render(ShortcutField, { value: 'Ctrl+`', onchange, onerror });
    target.querySelector<HTMLButtonElement>('.shortcut')!.click(); flushSync();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' })); flushSync();
    expect(onchange).not.toHaveBeenCalled(); expect(onerror).not.toHaveBeenCalled();
    expect(target.querySelectorAll('kbd')).toHaveLength(2);
  });

  it('keeps waiting while only a modifier is held', () => {
    const { field, onchange, onerror } = setup();
    field.click(); flushSync();
    press({ key: 'Shift', shiftKey: true }); flushSync();
    expect(onchange).not.toHaveBeenCalled(); expect(onerror).not.toHaveBeenCalled();
    expect(field.textContent).toContain('Type shortcut…');
    press({ key: 'k', shiftKey: true, metaKey: true });
    expect(onchange).toHaveBeenCalledWith('Cmd+Shift+K');
  });

  it('records Escape when a modifier is held with it', () => {
    const { field, onchange } = setup();
    field.click(); flushSync();
    press({ key: 'Escape', ctrlKey: true });
    expect(onchange).toHaveBeenCalledWith('Ctrl+Escape');
  });

  it('records the physical key when Shift types a symbol', () => {
    const { field, onchange } = setup();
    field.click(); flushSync();
    // Shift+= types `+`, which the backend would split on, so the event's code names the key.
    press({ key: '+', code: 'Equal', ctrlKey: true, shiftKey: true });
    expect(onchange).toHaveBeenCalledWith('Ctrl+Shift+=');
  });

  it('stops recording when the field loses focus', () => {
    const { target, field, onchange, onerror } = setup();
    field.click(); flushSync();
    // Safari does not focus a button on click, so the field has to take focus itself.
    expect(document.activeElement).toBe(field);
    field.blur(); flushSync();
    expect(target.querySelectorAll('kbd')).toHaveLength(2);
    press({ key: 'k', metaKey: true });
    expect(onchange).not.toHaveBeenCalled(); expect(onerror).not.toHaveBeenCalled();
  });

  it('only swallows keys while recording', () => {
    const { field } = setup();
    const idle = new KeyboardEvent('keydown', { key: 'k', metaKey: true, cancelable: true });
    window.dispatchEvent(idle);
    expect(idle.defaultPrevented).toBe(false);

    field.click(); flushSync();
    const recording = new KeyboardEvent('keydown', { key: 'k', metaKey: true, cancelable: true });
    window.dispatchEvent(recording);
    expect(recording.defaultPrevented).toBe(true);
  });

  it('does not start recording while disabled', () => {
    const { field } = setup({ disabled: true });
    expect(field.disabled).toBe(true);
    field.click(); flushSync();
    expect(field.textContent).not.toContain('Type shortcut…');
  });
});

describe('SearchField', () => {
  it('reports search input', () => {
    const oninput = vi.fn();
    const { target } = render(SearchField, { label: 'Search history', oninput });
    const input = target.querySelector<HTMLInputElement>('input[type="search"][aria-label="Search history"]')!;
    input.value = 'build'; input.dispatchEvent(new Event('input', { bubbles: true }));
    expect(oninput).toHaveBeenCalledWith('build');
  });

  it('starts from the value and placeholder it is given', () => {
    const { target } = render(SearchField, { label: 'Search dictionary', value: 'tauri', placeholder: 'Search' });
    const input = target.querySelector<HTMLInputElement>('input[type="search"]')!;
    expect(input.value).toBe('tauri');
    expect(input.placeholder).toBe('Search');
  });
});

describe('Pane', () => {
  it('gives the pane a draggable toolbar, a heading, and a labelled back button', () => {
    const onback = vi.fn();
    const { target } = render(PaneHarness, { onback });
    const toolbar = target.querySelector('header.toolbar')!;
    expect(toolbar.getAttribute('data-tauri-drag-region')).toBe('deep');
    expect(toolbar.querySelector('h1')?.textContent).toBe('Shorten');
    target.querySelector<HTMLButtonElement>('[aria-label="Back to AI Processing"]')!.click();
    expect(onback).toHaveBeenCalledTimes(1);
  });

  it('names the pane after its heading and puts its actions in the toolbar', () => {
    const { target } = render(Pane, {
      title: 'Dictionary',
      actions: snippet('<button type="button">Add</button>'),
      children: snippet('<p>Words</p>'),
    });
    const pane = target.querySelector('section.pane.ui-v2')!;
    const heading = pane.querySelector('h1')!;
    expect(pane.getAttribute('aria-labelledby')).toBe(heading.id);
    expect(pane.querySelector('header.toolbar button')?.textContent).toBe('Add');
    expect(pane.querySelector('.pane-body')?.textContent).toBe('Words');
  });

  it('has no back button when it cannot go back', () => {
    const { target } = render(Pane, { title: 'General', children: snippet('<p>Settings</p>') });
    expect(target.querySelector('header.toolbar button')).toBeNull();
  });

  it('shows its status in a toast outside the scrolling body and clears it on dismiss', () => {
    const status = createStatus({ timeout: vi.fn() } as never);
    const { target } = render(Pane, { title: 'General', status, children: snippet('<p>Settings</p>') });
    expect(target.querySelector('.toast')).toBeNull();

    status.fail('Failed to save: boom'); flushSync();
    expect(target.querySelector('.pane > .toast-error')?.textContent).toContain('Failed to save: boom');
    expect(target.querySelector('.pane-body .toast')).toBeNull();

    target.querySelector<HTMLButtonElement>('[aria-label="Dismiss"]')!.click(); flushSync();
    expect(status.error).toBe('');
    expect(target.querySelector('.toast')).toBeNull();
  });
});
