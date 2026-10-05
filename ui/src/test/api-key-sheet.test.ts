import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushSync } from 'svelte';
import userEvent from '@testing-library/user-event';
import ApiKeySheet from '../components/settings/ApiKeySheet.svelte';
import { button, render, settle, unmountAll } from './helpers';

afterEach(async () => {
  await unmountAll();
  vi.useRealTimers();
});

/** The sheet, open for adding a key to OpenAI Whisper unless a test says otherwise. */
function open(props: { mode?: 'add' | 'change'; busy?: boolean; error?: string } = {}) {
  const onsave = vi.fn();
  const onclose = vi.fn();
  const { target } = render(ApiKeySheet, {
    providerName: 'OpenAI Whisper', mode: 'add', onsave, onclose, ...props,
  });
  return { target, onsave, onclose };
}

const field = (target: Element) => target.querySelector<HTMLInputElement>('input')!;
const submit = (target: Element) => target.querySelector<HTMLButtonElement>('button[type="submit"]')!;
const toggle = (target: Element) => target.querySelector<HTMLButtonElement>('button[aria-label$="API key"]')!;
const explanation = (target: Element) => target.querySelector('.sheet-error')?.textContent;

function fill(input: HTMLInputElement, value: string) {
  input.value = value;
  input.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
}

describe('ApiKeySheet', () => {
  it('is titled for adding a key and offers Save & Use', () => {
    const { target } = open({ mode: 'add' });
    const dialog = target.querySelector('[role="dialog"]')!;
    expect(target.querySelector(`#${dialog.getAttribute('aria-labelledby')}`)?.textContent)
      .toBe('Add API Key for OpenAI Whisper');
    expect(submit(target).textContent?.trim()).toBe('Save & Use');
  });

  it('is titled for changing a key and offers Save', () => {
    const { target } = open({ mode: 'change' });
    expect(target.querySelector('h2')?.textContent).toBe('Change API Key for OpenAI Whisper');
    expect(submit(target).textContent?.trim()).toBe('Save');
  });

  it('names the service it was given, whichever it is', () => {
    const { target } = render(ApiKeySheet, {
      providerName: 'Claude API', mode: 'change', onsave: vi.fn(), onclose: vi.fn(),
    });
    expect(target.querySelector('h2')?.textContent).toBe('Change API Key for Claude API');
  });

  it('has one labelled key field, hidden and focused when it opens', () => {
    const { target } = open();
    const input = field(target);
    expect(target.querySelectorAll('input')).toHaveLength(1);
    expect(target.querySelector('label')?.textContent).toBe('API Key');
    expect(input.id).not.toBe('');
    expect(target.querySelector('label')?.htmlFor).toBe(input.id);
    expect(input.type).toBe('password');
    // The sheet focuses its first field, so the key can be pasted at once.
    expect(document.activeElement).toBe(input);
  });

  it('keeps the field and its show/hide button together, apart from the sheet\'s own column', () => {
    const { target } = open();
    // Sheet makes every input full width, so the button needs a row of its own to sit beside the field.
    expect(field(target).parentElement).toBe(toggle(target).parentElement);
    expect(field(target).parentElement).not.toBe(target.querySelector('.sheet-fields'));
  });

  it('shows and hides the key with a button that says what it will do', async () => {
    const { target } = open();
    const input = field(target);
    fill(input, 'sk-secret');
    expect(toggle(target).getAttribute('aria-label')).toBe('Show API key');
    // Inside a form, a button without a type would submit it.
    expect(toggle(target).type).toBe('button');

    toggle(target).click(); await settle();
    expect(input.type).toBe('text');
    expect(toggle(target).getAttribute('aria-label')).toBe('Hide API key');
    // Revealing the key must not touch what was typed.
    expect(input.value).toBe('sk-secret');

    toggle(target).click(); await settle();
    expect(input.type).toBe('password');
    expect(toggle(target).getAttribute('aria-label')).toBe('Show API key');
    expect(input.value).toBe('sk-secret');
  });

  it('hands the key to onsave exactly as typed', async () => {
    const { target, onsave } = open();
    // Untrimmed, as the page passed it on before the sheet was a component of its own.
    // Trimming would change what is stored, so that has to be a decision, not a side effect.
    fill(field(target), '  sk-test  ');
    submit(target).click(); await settle();
    expect(onsave).toHaveBeenCalledTimes(1);
    expect(onsave).toHaveBeenCalledWith('  sk-test  ');
  });

  it('submits with Enter in the field', async () => {
    const { target, onsave } = open();
    expect(document.activeElement).toBe(field(target));
    await userEvent.setup().keyboard('sk-typed{Enter}');
    await settle();
    expect(onsave).toHaveBeenCalledTimes(1);
    expect(onsave).toHaveBeenCalledWith('sk-typed');
  });

  it.each([['nothing', ''], ['only spaces', '   ']])(
    'explains a key of %s inside the sheet, and does not call onsave',
    async (_what, typed) => {
      const { target, onsave } = open();
      fill(field(target), typed);
      submit(target).click(); await settle();
      expect(explanation(target)).toBe('API key cannot be empty');
      expect(onsave).not.toHaveBeenCalled();
      // The sheet stays up for the next try.
      expect(target.querySelector('[role="dialog"]')).not.toBeNull();
    },
  );

  it('drops its explanation once the key is edited', async () => {
    const { target } = open();
    submit(target).click(); await settle();
    expect(explanation(target)).toBe('API key cannot be empty');
    fill(field(target), 'k');
    expect(explanation(target)).toBeUndefined();
  });

  it('shows the error it is given, and its own explanation takes the place while it is showing', async () => {
    const { target, onsave } = open({ error: 'Keychain locked' });
    expect(explanation(target)).toBe('Keychain locked');
    // Only one message fits under the field, and the newest is about what was just submitted.
    submit(target).click(); await settle();
    expect(explanation(target)).toBe('API key cannot be empty');
    expect(onsave).not.toHaveBeenCalled();
  });

  it('shows no error when it has none', () => {
    const { target } = open();
    expect(target.querySelector('.sheet-error')).toBeNull();
  });

  it('says Saving… and does nothing while busy', async () => {
    const { target, onsave } = open({ busy: true });
    const save = submit(target);
    expect(save.textContent?.trim()).toBe('Saving…');
    expect(save.disabled).toBe(true);
    fill(field(target), 'sk-test');
    // A disabled button already stops Enter; this is the sheet itself refusing a second submit.
    const second = new Event('submit', { bubbles: true, cancelable: true });
    target.querySelector('form')!.dispatchEvent(second);
    await settle();
    expect(second.defaultPrevented).toBe(true);
    expect(onsave).not.toHaveBeenCalled();
  });

  it('closes with Cancel and with Escape, saving nothing', async () => {
    const { target, onsave, onclose } = open();
    fill(field(target), 'sk-secret');
    button(target, 'Cancel').click(); await settle();
    expect(onclose).toHaveBeenCalledTimes(1);
    await userEvent.setup().keyboard('{Escape}');
    expect(onclose).toHaveBeenCalledTimes(2);
    expect(onsave).not.toHaveBeenCalled();
  });

  it('keeps Cancel from submitting', async () => {
    const { target, onsave } = open();
    fill(field(target), 'sk-secret');
    expect(button(target, 'Cancel').type).toBe('button');
    button(target, 'Cancel').click(); await settle();
    expect(onsave).not.toHaveBeenCalled();
  });
});
