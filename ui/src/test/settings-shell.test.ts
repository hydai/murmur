import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync } from 'svelte';
import userEvent from '@testing-library/user-event';
import SettingsPanel from '../components/settings/SettingsPanel.svelte';
import { initialRoute } from '../components/settings/navigation';
import { render, settle, unmountAll } from './helpers';

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(), listen: vi.fn(), check: vi.fn(), startDragging: vi.fn(), writeText: vi.fn(),
}));
vi.mock('../lib/tauri', () => ({ safeInvoke: mocks.invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }));
vi.mock('@tauri-apps/api/app', () => ({ getVersion: async () => '1.0.0' }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ startDragging: mocks.startDragging }) }));
vi.mock('@tauri-apps/plugin-updater', () => ({ check: mocks.check }));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: vi.fn() }));
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText: mocks.writeText }));

let listeners: Map<string, (event: { payload: unknown }) => void>;

function emit(name: string, payload: unknown) {
  const listener = listeners.get(name);
  expect(listener, `${name} listener`).toBeDefined();
  listener!({ payload });
  flushSync();
}

/** What the legacy pages read when they mount. */
const CONFIG = {
  hotkey: 'Ctrl+`',
  output_mode: 'clipboard',
  save_history: true,
  chinese_conversion: 'traditional',
  stt_provider: 'openai',
  llm_processor: 'claude_api',
  llm_model: null,
  apple_stt_locale: 'auto',
  elevenlabs_language: 'auto',
  http_stt_config: null,
  http_llm_config: null,
};

beforeEach(() => {
  listeners = new Map();
  mocks.invoke.mockReset().mockImplementation(async (command: string) => {
    switch (command) {
      case 'get_config': return CONFIG;
      case 'get_stt_providers':
      case 'get_llm_processors':
      case 'get_prompts':
      case 'get_diagnostic_logs': return [];
      case 'get_dictionary': return { entries: [] };
      default: return undefined;
    }
  });
  mocks.check.mockReset();
  mocks.listen.mockReset().mockImplementation(async (name, callback) => {
    listeners.set(name, callback);
    return () => listeners.delete(name);
  });
  mocks.startDragging.mockResolvedValue(undefined);
  mocks.writeText.mockReset().mockResolvedValue(undefined);
});

afterEach(async () => {
  await unmountAll();
  vi.useRealTimers();
  // Some tests open the shell the way a new window would, through its URL.
  window.history.replaceState(null, '', '/');
});

/** The sidebar entries, in order. */
const navItems = (target: Element) =>
  [...target.querySelectorAll<HTMLElement>('nav[aria-label="Settings"] .nav-item')];

describe('settings shell', () => {
  it('resolves the initial pane from the window URL', () => {
    expect(initialRoute('?view=settings')).toEqual({ pane: 'general', checkUpdate: false });
    expect(initialRoute('?view=settings&pane=dictionary')).toEqual({ pane: 'dictionary', checkUpdate: false });
    expect(initialRoute('?view=settings&action=check-update')).toEqual({ pane: 'about', checkUpdate: true });
    expect(initialRoute('?view=settings&pane=bogus')).toEqual({ pane: 'general', checkUpdate: false });
    // The URL the menu bar's Check for Updates gives a window that is not open yet.
    expect(initialRoute('?view=settings&pane=about&action=check-update')).toEqual({ pane: 'about', checkUpdate: true });
    // An unknown pane drops the whole route, as it does for a navigate event.
    expect(initialRoute('?view=settings&pane=bogus&action=check-update')).toEqual({ pane: 'general', checkUpdate: false });
  });

  it('lists the panes in sidebar order and marks the active one', async () => {
    const { target } = render(SettingsPanel, {});
    await settle();
    const items = navItems(target);
    expect(items.map(i => i.textContent?.trim())).toEqual(['General', 'Transcription', 'AI Processing', 'Dictionary', 'About']);
    expect(items[0].getAttribute('aria-current')).toBe('page');
  });

  it('switches panes from the sidebar with the keyboard', async () => {
    const { target } = render(SettingsPanel, {});
    await settle();
    const items = navItems(target);
    const dictionary = items.find(i => i.textContent?.includes('Dictionary'))!;
    dictionary.focus();
    await userEvent.setup().keyboard('{Enter}');
    await settle();
    expect(dictionary.getAttribute('aria-current')).toBe('page');
    expect(items[0].getAttribute('aria-current')).toBeNull();
  });

  it('opens on the pane its window URL names', async () => {
    window.history.replaceState(null, '', '/?view=settings&pane=dictionary');
    const { target } = render(SettingsPanel, {});
    await settle();
    expect(target.querySelector('.nav-item[aria-current="page"]')?.textContent).toContain('Dictionary');
    expect(mocks.check).not.toHaveBeenCalled();
  });

  it('runs the update check a freshly opened window asks for', async () => {
    window.history.replaceState(null, '', '/?view=settings&pane=about&action=check-update');
    const { target } = render(SettingsPanel, {});
    await settle();
    expect(target.querySelector('.nav-item[aria-current="page"]')?.textContent).toContain('About');
    expect(mocks.check).toHaveBeenCalledTimes(1);
  });

  it('follows a navigate event and runs the requested update check', async () => {
    const { target } = render(SettingsPanel, {});
    await settle();
    emit('navigate', { pane: 'about', action: 'check-update' });
    await settle();
    expect(target.querySelector('.nav-item[aria-current="page"]')?.textContent).toContain('About');
    expect(mocks.check).toHaveBeenCalledTimes(1);
  });

  it('ignores a navigate event for an unknown pane', async () => {
    const { target } = render(SettingsPanel, {});
    await settle();
    emit('navigate', { pane: 'bogus', action: 'check-update' });
    await settle();
    expect(target.querySelector('.nav-item[aria-current="page"]')?.textContent).toContain('General');
    // The action goes with it: opening About later must not run a check nobody asked for.
    navItems(target).find(i => i.textContent?.includes('About'))!.click();
    await settle();
    expect(target.querySelector('.nav-item[aria-current="page"]')?.textContent).toContain('About');
    expect(mocks.check).not.toHaveBeenCalled();
  });

  it('lets the empty sidebar drag the window', async () => {
    const { target } = render(SettingsPanel, {});
    expect(target.querySelector('nav[aria-label="Settings"]')?.getAttribute('data-tauri-drag-region')).toBe('deep');
  });
});
