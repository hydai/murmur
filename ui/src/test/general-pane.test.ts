import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync } from 'svelte';
import GeneralConfig from '../components/settings/GeneralConfig.svelte';
import { MISSING_MODIFIER_MESSAGE } from '../lib/shortcut';
import { button, render, settle, unmountAll } from './helpers';

const mocks = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('../lib/tauri', () => ({ safeInvoke: mocks.invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

/** What get_config returns on a fresh install. */
const CONFIG = {
  hotkey: 'Ctrl+`',
  output_mode: 'clipboard',
  save_history: true,
  chinese_conversion: 'traditional',
};

/** Every command succeeds, and get_config answers with `config`. */
function backend(config: Record<string, unknown> = CONFIG) {
  mocks.invoke.mockImplementation(async (command: string) => (command === 'get_config' ? config : undefined));
}

/** Like `backend`, except `command` rejects with `error`. The page logs that, so the log is silenced. */
function backendRejecting(command: string, error: Error) {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  mocks.invoke.mockImplementation(async (name: string) => {
    if (name === command) throw error;
    return name === 'get_config' ? CONFIG : undefined;
  });
}

beforeEach(() => {
  backend();
});

afterEach(async () => {
  await unmountAll();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

async function open() {
  const { target } = render(GeneralConfig, {});
  await settle();
  return target;
}

const keycaps = (target: Element) => [...target.querySelectorAll('kbd')].map(key => key.textContent);
const inUse = (target: Element) => [...target.querySelectorAll('[aria-current="true"]')];
const errorToast = (target: Element) => target.querySelector('.toast-error')?.textContent;
const chineseSelect = (target: Element) =>
  target.querySelector<HTMLSelectElement>('select[aria-label="Chinese characters"]')!;
const historySwitch = (target: Element) =>
  target.querySelector<HTMLButtonElement>('[role="switch"][aria-label="Save transcription history"]')!;

async function choose(select: HTMLSelectElement, value: string) {
  select.value = value;
  select.dispatchEvent(new Event('change', { bubbles: true }));
  await settle();
}

/** Click the shortcut field and press `init`, the way a user records a shortcut. */
async function typeShortcut(target: Element, init: KeyboardEventInit) {
  target.querySelector<HTMLButtonElement>('.shortcut')!.click();
  flushSync();
  window.dispatchEvent(new KeyboardEvent('keydown', init));
  await settle();
}

describe('General pane', () => {
  it('lays the settings out in four groups, with the copy the spec gives', async () => {
    const target = await open();
    expect(target.querySelector('h1')?.textContent).toBe('General');
    expect([...target.querySelectorAll('.group-title')].map(title => title.textContent))
      .toEqual(['Recording', 'After Transcribing', 'Text', 'History']);
    expect([...target.querySelectorAll('.row-text')].map(text => [
      text.querySelector('.row-label')?.textContent,
      text.querySelector('.row-detail')?.textContent,
    ])).toEqual([
      ['Shortcut', 'Starts and stops recording'],
      ['Copy to clipboard', 'Paste it yourself with ⌘V'],
      ['Type it out', 'Murmur types the text at your cursor'],
      ['Type it out and copy', 'Also keeps a copy on the clipboard'],
      ['Chinese characters', 'Translations into Simplified Chinese are kept'],
      ['Save transcription history', 'Kept only on this Mac'],
    ]);
    expect([...chineseSelect(target).options].map(option => [option.value, option.textContent]))
      .toEqual([['traditional', 'Traditional (Taiwan)'], ['none', "Don't convert"]]);
  });

  it('names the shortcut field after its row, not just the keys it shows', async () => {
    const target = await open();
    expect(target.querySelector('.shortcut')?.textContent).toMatch(/^Shortcut:/);
  });

  it('shows the settings the backend has stored', async () => {
    backend({ hotkey: 'Cmd+Shift+K', output_mode: 'both', save_history: false, chinese_conversion: 'none' });
    const target = await open();
    expect(keycaps(target)).toEqual(['⇧', '⌘', 'K']);
    expect(inUse(target)).toHaveLength(1);
    expect(inUse(target)[0].textContent).toContain('Also keeps a copy on the clipboard');
    expect(chineseSelect(target).value).toBe('none');
    expect(historySwitch(target).getAttribute('aria-checked')).toBe('false');
  });

  it('falls back to the defaults for settings an older config does not have', async () => {
    backend({ hotkey: 'Ctrl+`', output_mode: 'keyboard' });
    const target = await open();
    expect(chineseSelect(target).value).toBe('traditional');
    expect(historySwitch(target).getAttribute('aria-checked')).toBe('true');
  });

  it('says so when the settings cannot be loaded', async () => {
    backendRejecting('get_config', new Error('unavailable'));
    const target = await open();
    expect(errorToast(target)).toContain('Failed to load settings');
  });

  it('marks the output mode in use and switches without a success toast', async () => {
    const target = await open();
    // Found by their detail text: 'Type it out' is also the start of 'Type it out and copy'.
    expect(inUse(target)).toHaveLength(1);
    expect(inUse(target)[0].textContent).toContain('Paste it yourself with ⌘V');
    button(target, 'Murmur types the text at your cursor').click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('set_output_mode', { mode: 'keyboard' });
    expect(inUse(target)).toHaveLength(1);
    expect(inUse(target)[0].textContent).toContain('Murmur types the text at your cursor');
    expect(target.querySelector('.toast-success')).toBeNull();
  });

  it('keeps the output mode when the backend rejects the change', async () => {
    backendRejecting('set_output_mode', new Error('denied'));
    const target = await open();
    button(target, 'Murmur types the text at your cursor').click(); await settle();
    expect(inUse(target)).toHaveLength(1);
    expect(inUse(target)[0].textContent).toContain('Paste it yourself with ⌘V');
    expect(errorToast(target)).toContain('Failed to set output mode');
  });

  it('saves a recorded shortcut', async () => {
    const target = await open();
    expect(keycaps(target)).toEqual(['⌃', '`']);
    await typeShortcut(target, { key: 'k', metaKey: true, shiftKey: true });
    expect(mocks.invoke).toHaveBeenCalledWith('set_hotkey', { hotkey: 'Cmd+Shift+K' });
    expect(keycaps(target)).toEqual(['⇧', '⌘', 'K']);
  });

  it('explains a shortcut without a modifier under the group and does not save it', async () => {
    const target = await open();
    await typeShortcut(target, { key: 'k' });
    const explanation = target.querySelector('.group-error');
    expect(explanation?.textContent).toBe(MISSING_MODIFIER_MESSAGE);
    expect(explanation?.closest('.group')?.querySelector('.group-title')?.textContent).toBe('Recording');
    expect(mocks.invoke).not.toHaveBeenCalledWith('set_hotkey', expect.anything());
    expect(keycaps(target)).toEqual(['⌃', '`']);
  });

  it('keeps the old shortcut when the backend rejects the new one', async () => {
    backendRejecting('set_hotkey', new Error('Shortcut already in use'));
    const target = await open();
    await typeShortcut(target, { key: 'k', metaKey: true, shiftKey: true });
    expect(keycaps(target)).toEqual(['⌃', '`']);
    expect(errorToast(target)).toContain('Failed to set shortcut');
    expect(errorToast(target)).toContain('Shortcut already in use');
    // The group's own message is for a key press that was not a shortcut.
    expect(target.querySelector('.group-error')).toBeNull();
  });

  it('drops the explanation once the next shortcut is recorded, even one the backend refuses', async () => {
    backendRejecting('set_hotkey', new Error('Shortcut already in use'));
    const target = await open();
    await typeShortcut(target, { key: 'k' });
    expect(target.querySelector('.group-error')).not.toBeNull();
    // Leaving it up would tell someone who did add a modifier that they had not.
    await typeShortcut(target, { key: 'k', metaKey: true });
    expect(target.querySelector('.group-error')).toBeNull();
    expect(errorToast(target)).toContain('Failed to set shortcut');
  });

  it('keeps the Chinese conversion when the backend rejects the change', async () => {
    backendRejecting('set_chinese_conversion', new Error('denied'));
    const target = await open();
    const select = chineseSelect(target);
    await choose(select, 'none');
    expect(mocks.invoke).toHaveBeenCalledWith('set_chinese_conversion', { mode: 'none' });
    // The native element showed the pick until the page answered, so it has to be put back.
    expect(select.value).toBe('traditional');
    expect(errorToast(target)).toContain('Failed to set Chinese conversion');
  });

  it('keeps the history setting when the backend rejects the change', async () => {
    backendRejecting('set_save_history', new Error('disk full'));
    const target = await open();
    historySwitch(target).click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('set_save_history', { enabled: false });
    expect(historySwitch(target).getAttribute('aria-checked')).toBe('true');
    expect(errorToast(target)).toContain('Failed to update history setting');
  });

  it('confirms nothing for a change the screen already shows', async () => {
    const target = await open();
    // Each step is checked on its own: the next change clears whatever the last one left.
    await typeShortcut(target, { key: 'k', metaKey: true });
    expect(mocks.invoke).toHaveBeenCalledWith('set_hotkey', { hotkey: 'Cmd+K' });
    expect(target.querySelector('.toast')).toBeNull();
    await choose(chineseSelect(target), 'none');
    expect(mocks.invoke).toHaveBeenCalledWith('set_chinese_conversion', { mode: 'none' });
    expect(target.querySelector('.toast')).toBeNull();
    historySwitch(target).click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('set_save_history', { enabled: false });
    expect(target.querySelector('.toast')).toBeNull();
  });
});
