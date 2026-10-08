import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync } from 'svelte';
import userEvent from '@testing-library/user-event';
import App from '../App.svelte';
import SettingsPanel from '../components/settings/SettingsPanel.svelte';
import { initialRoute } from '../components/settings/navigation';
import { resetDrafts } from '../components/settings/promptDrafts.svelte';
import { VOICE_COMMANDS } from '../lib/voiceCommands';
import { button, fill, render, settle, unmountAll } from './helpers';

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

/** The config the panes read when they mount. */
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

/** A prompt as `get_prompts` lists it; the backend has one for each voice command. */
const prompt = (name: string) => ({
  name,
  description: `About ${name}.`,
  required_placeholders: [] as string[],
  content: `content of ${name}`,
  is_override: false,
});

/** What the backend keeps of the prompts, which `set_prompt` marks as edited the way the real one does. */
let prompts: ReturnType<typeof prompt>[];

beforeEach(() => {
  listeners = new Map();
  // An unsaved prompt draft belongs to the window, so one test's would be on show in the next.
  resetDrafts();
  prompts = VOICE_COMMANDS.map((command) => prompt(command.prompt));
  mocks.invoke.mockReset().mockImplementation(async (command: string, args?: any) => {
    switch (command) {
      case 'get_config': return CONFIG;
      case 'get_stt_providers':
      case 'get_llm_processors':
      case 'get_history': return [];
      case 'get_prompts': return prompts.map((entry) => ({ ...entry }));
      case 'set_prompt':
        prompts = prompts.map((entry) => (entry.name === args.params.name
          ? { ...entry, content: args.params.content, is_override: true }
          : entry));
        return undefined;
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

/** The sidebar entry called `label`. */
function navItem(target: Element, label: string): HTMLElement {
  const found = navItems(target).find(item => item.textContent?.trim() === label);
  expect(found, `sidebar item ${label}`).toBeDefined();
  return found!;
}

/** What the open pane shows, the sidebar apart. */
const pane = (target: Element) => target.querySelector('main')!;
const heading = (target: Element) => pane(target).querySelector('h1')?.textContent;
const groupTitles = (target: Element) =>
  [...pane(target).querySelectorAll('.group-title')].map(title => title.textContent);
const editor = (target: Element) => pane(target).querySelector<HTMLTextAreaElement>('textarea')!;
const called = (command: string) => mocks.invoke.mock.calls.filter(([name]) => name === command);

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

  it('opens History for the history URLs', () => {
    // The URL the menu bar's History gives a window that is not open yet.
    expect(initialRoute('?view=settings&pane=history')).toEqual({ pane: 'history', checkUpdate: false });
    // The old history window's URL lands on the same pane, whatever else it carries.
    expect(initialRoute('?view=history')).toEqual({ pane: 'history', checkUpdate: false });
    expect(initialRoute('?view=history&action=check-update')).toEqual({ pane: 'history', checkUpdate: false });
    expect(initialRoute('?view=history&pane=bogus&action=check-update')).toEqual({ pane: 'history', checkUpdate: false });
    // Only a pane that exists beats it.
    expect(initialRoute('?view=history&pane=dictionary')).toEqual({ pane: 'dictionary', checkUpdate: false });
    expect(initialRoute('?view=history&pane=about&action=check-update')).toEqual({ pane: 'about', checkUpdate: true });
  });

  it('lists the panes in sidebar order and marks the active one', async () => {
    const { target } = render(SettingsPanel, {});
    await settle();
    const items = navItems(target);
    expect(items.map(i => i.textContent?.trim())).toEqual(['General', 'Transcription', 'AI Processing', 'Dictionary', 'History', 'About']);
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

  it('opens on History when its window URL names it', async () => {
    window.history.replaceState(null, '', '/?view=settings&pane=history');
    const { target } = render(SettingsPanel, {});
    await settle();
    expect(target.querySelector('.nav-item[aria-current="page"]')?.textContent).toContain('History');
    expect(mocks.invoke).toHaveBeenCalledWith('get_history', expect.anything());
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

  it('follows a navigate event to History', async () => {
    const { target } = render(SettingsPanel, {});
    await settle();
    // The page loads its entries once it is shown, not with the window.
    expect(mocks.invoke).not.toHaveBeenCalledWith('get_history', expect.anything());
    emit('navigate', { pane: 'history' });
    await settle();
    expect(target.querySelector('.nav-item[aria-current="page"]')?.textContent).toContain('History');
    expect(mocks.invoke).toHaveBeenCalledWith('get_history', expect.anything());
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

  it('shows the shell, on History, for the old history window URL', async () => {
    window.history.replaceState(null, '', '/?view=history');
    const { target } = render(App, {});
    await settle();
    expect(navItems(target)).toHaveLength(6);
    expect(target.querySelector('.nav-item[aria-current="page"]')?.textContent).toContain('History');
  });

  it('lets the empty sidebar drag the window', async () => {
    const { target } = render(SettingsPanel, {});
    expect(target.querySelector('nav[aria-label="Settings"]')?.getAttribute('data-tauri-drag-region')).toBe('deep');
  });
});

describe('settings shell: the sidebar item of the pane that is shown', () => {
  it('leaves a prompt editor for the AI Processing list, with the focus on the sidebar item', async () => {
    const { target } = render(SettingsPanel, {});
    await settle();
    const user = userEvent.setup();
    await user.click(navItem(target, 'AI Processing')); await settle();
    button(pane(target), 'Shorten').click(); await settle();
    expect(heading(target)).toBe('Shorten');
    const processorLoads = called('get_llm_processors').length;

    // The user is typing in the editor when they press the item.
    editor(target).focus();
    const item = navItem(target, 'AI Processing');
    await user.click(item); await settle();
    expect(heading(target)).toBe('AI Processing');
    expect(groupTitles(target)).toContain('Voice Commands');
    expect(pane(target).querySelector('textarea')).toBeNull();
    expect(item.getAttribute('aria-current')).toBe('page');
    // Back would put the focus on the Shorten row. Here the user is on the sidebar, and stays there.
    expect(document.activeElement).toBe(item);
    // The pane was not rebuilt: what it knew of the processors is not asked for again.
    expect(called('get_llm_processors')).toHaveLength(processorLoads);
  });

  it('leaves the Diagnostics Log for About from the keyboard, and About keeps what it knew', async () => {
    mocks.check.mockResolvedValue(null);
    const { target } = render(SettingsPanel, {});
    await settle();
    navItem(target, 'About').click(); await settle();
    button(pane(target), 'Check for Updates').click(); await settle();
    button(pane(target), 'Diagnostics Log').click(); await settle();
    expect(heading(target)).toBe('Diagnostics Log');

    const item = navItem(target, 'About');
    item.focus();
    await userEvent.setup().keyboard('{Enter}'); await settle();
    expect(heading(target)).toBe('About');
    expect(groupTitles(target)).toEqual(['Software Update', 'Links', 'Troubleshooting']);
    // Back would put the focus on the Diagnostics Log row.
    expect(document.activeElement).toBe(item);
    // The page was not rebuilt: the update check it had run is still its answer, and was not run again.
    expect(pane(target).textContent).toContain('Murmur is up to date');
    expect(mocks.check).toHaveBeenCalledTimes(1);
  });

  it('leaves History as it is, with the search that was typed and what it found', async () => {
    vi.useFakeTimers();
    const entries = ['apple pie', 'banana'].map((text, index) => ({
      id: `${index}`, final_text: text, timestamp_ms: Date.now(), processing_time_ms: 0,
    }));
    const working = mocks.invoke.getMockImplementation()!;
    mocks.invoke.mockImplementation(async (command: string, args?: any) => {
      switch (command) {
        case 'get_history': return entries;
        case 'search_history': return entries.filter(entry => entry.final_text.includes(args.query));
        default: return working(command, args);
      }
    });
    const { target } = render(SettingsPanel, {});
    await settle();
    navItem(target, 'History').click(); await settle();
    const search = pane(target).querySelector<HTMLInputElement>('input[type="search"]')!;
    const shown = () => [...pane(target).querySelectorAll('.entry-text')].map(text => text.textContent);
    fill(search, 'app');
    await vi.advanceTimersByTimeAsync(300); await settle();
    expect(shown()).toEqual(['apple pie']);
    const loads = [called('get_history').length, called('search_history').length];

    navItem(target, 'History').click(); await settle();
    // Past the search's own delay, so a reload that waited for it would show.
    await vi.advanceTimersByTimeAsync(1000); await settle();
    expect(pane(target).querySelector('input[type="search"]')).toBe(search);
    expect(search.value).toBe('app');
    expect(shown()).toEqual(['apple pie']);
    expect([called('get_history').length, called('search_history').length]).toEqual(loads);
  });

  it('leaves a pane that is at its top level alone, and a page opened afterwards is not closed', async () => {
    mocks.check.mockResolvedValue(null);
    const { target } = render(SettingsPanel, {});
    await settle();

    navItem(target, 'AI Processing').click(); await settle();
    const reads = () => ['get_config', 'get_llm_processors', 'get_prompts'].map(command => called(command).length);
    const before = reads();
    navItem(target, 'AI Processing').click(); await settle();
    expect(reads()).toEqual(before);
    expect(groupTitles(target)).toContain('Voice Commands');
    button(pane(target), 'Shorten').click(); await settle();
    expect(heading(target)).toBe('Shorten');

    navItem(target, 'About').click(); await settle();
    button(pane(target), 'Check for Updates').click(); await settle();
    navItem(target, 'About').click(); await settle();
    expect(pane(target).textContent).toContain('Murmur is up to date');
    expect(mocks.check).toHaveBeenCalledTimes(1);
    button(pane(target), 'Diagnostics Log').click(); await settle();
    expect(heading(target)).toBe('Diagnostics Log');
  });

  it('keeps a prompt draft that was typed before the editor was left from the sidebar', async () => {
    const { target } = render(SettingsPanel, {});
    await settle();
    navItem(target, 'AI Processing').click(); await settle();
    button(pane(target), 'Shorten').click(); await settle();
    fill(editor(target), 'work in progress'); await settle();

    navItem(target, 'AI Processing').click(); await settle();
    expect(heading(target)).toBe('AI Processing');
    expect(button(pane(target), 'Shorten').closest('.row')?.textContent).toContain('Unsaved');
    button(pane(target), 'Shorten').click(); await settle();
    expect(editor(target).value).toBe('work in progress');
  });

  it('marks a prompt that was saved in the editor as Edited once the editor is left from the sidebar', async () => {
    const { target } = render(SettingsPanel, {});
    await settle();
    navItem(target, 'AI Processing').click(); await settle();
    button(pane(target), 'Shorten').click(); await settle();
    fill(editor(target), 'edited'); await settle();
    button(pane(target), 'Save').click(); await settle();

    navItem(target, 'AI Processing').click(); await settle();
    expect(heading(target)).toBe('AI Processing');
    const row = button(pane(target), 'Shorten').closest('.row');
    expect(row?.textContent).toContain('Edited');
    expect(row?.textContent).not.toContain('Unsaved');
  });

  it('still switches to another pane from a page below, and shows a pane afresh when it is chosen again', async () => {
    const { target } = render(SettingsPanel, {});
    await settle();
    navItem(target, 'AI Processing').click(); await settle();
    button(pane(target), 'Shorten').click(); await settle();
    expect(heading(target)).toBe('Shorten');

    navItem(target, 'Dictionary').click(); await settle();
    expect(navItem(target, 'Dictionary').getAttribute('aria-current')).toBe('page');
    expect(heading(target)).toBe('Dictionary');

    navItem(target, 'AI Processing').click(); await settle();
    expect(heading(target)).toBe('AI Processing');
    expect(groupTitles(target)).toContain('Voice Commands');
  });
});
