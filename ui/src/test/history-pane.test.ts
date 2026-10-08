import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import userEvent from '@testing-library/user-event';
import HistoryPanel from '../components/history/HistoryPanel.svelte';
import SettingsPanel from '../components/settings/SettingsPanel.svelte';
import type { PaneId } from '../components/settings/navigation';
import {
  button, dialog, dialogTitle, errorToast, fill, render, settle, successToast, unmountAll,
} from './helpers';

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), writeText: vi.fn(), listen: vi.fn() }));
vi.mock('../lib/tauri', () => ({ safeInvoke: mocks.invoke }));
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText: mocks.writeText }));
// What the settings window imports besides History, for the one test that mounts it whole.
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }));
vi.mock('@tauri-apps/api/app', () => ({ getVersion: async () => '1.0.0' }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ startDragging: vi.fn() }) }));
vi.mock('@tauri-apps/plugin-updater', () => ({ check: vi.fn() }));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: vi.fn() }));

/** An entry as `get_history` lists it. */
interface Entry {
  id: string;
  timestamp_ms: number;
  raw_text?: string;
  final_text: string;
  command_name?: string;
  processing_time_ms: number;
}

let serial = 0;
/** An entry from just now, unless `extra` says otherwise. */
const entry = (text: string, extra: Partial<Entry> = {}): Entry => ({
  id: `entry-${++serial}`,
  final_text: text,
  timestamp_ms: Date.now(),
  processing_time_ms: 0,
  ...extra,
});

/** `count` entries named `Entry 0`, `Entry 1`, … newest first, like the real history. */
const many = (count: number) => Array.from({ length: count }, (_, i) => entry(`Entry ${i}`, { id: `${i}` }));

/**
 * A backend that remembers what it is told, the way the real one does: `get_history` pages the
 * list, `search_history` filters it, a delete removes the entry (and refuses one it does not
 * have), clear empties it, and `get_config` says whether history is being saved. Every other
 * command succeeds. `options.config` is what `get_config` answers, whatever it is, `undefined` included.
 */
function backend(initial: Entry[] = [], options: { config?: unknown } = {}) {
  const config = 'config' in options ? options.config : { save_history: true };
  const state = { entries: initial.map(item => ({ ...item })) };
  mocks.invoke.mockImplementation(async (command: string, args?: any) => {
    switch (command) {
      case 'get_config':
        return config;
      case 'get_history':
        return state.entries.slice(args.offset, args.offset + args.limit).map(item => ({ ...item }));
      case 'search_history': {
        const query = args.query.toLowerCase();
        return state.entries.filter(item => item.final_text.toLowerCase().includes(query)).map(item => ({ ...item }));
      }
      case 'delete_history_entry': {
        const index = state.entries.findIndex(item => item.id === args.id);
        if (index < 0) throw new Error(`History entry '${args.id}' not found`);
        state.entries.splice(index, 1);
        return undefined;
      }
      case 'clear_history':
        state.entries.length = 0;
        return undefined;
      default: return undefined;
    }
  });
  return state;
}

/** Like `backend`, except `command` rejects with `error`. */
function failing(command: string, error: Error, initial: Entry[] = []) {
  backend(initial);
  const working = mocks.invoke.getMockImplementation()!;
  mocks.invoke.mockImplementation(async (name: string, args?: unknown) => {
    if (name === command) throw error;
    return working(name, args);
  });
}

/**
 * `command` is answered, as `backend` would, only once the returned `release` is called, or
 * with `error` if there is one; everything else is as `backend` made it.
 */
function holding(command: string, error?: Error) {
  let release!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  const working = mocks.invoke.getMockImplementation()!;
  mocks.invoke.mockImplementation(async (name: string, args?: unknown) => {
    if (name === command) {
      await held;
      if (error) throw error;
    }
    return working(name, args);
  });
  return release;
}

beforeEach(() => {
  mocks.writeText.mockReset().mockResolvedValue(undefined);
  mocks.listen.mockReset().mockResolvedValue(() => {});
  backend();
});

afterEach(async () => {
  await unmountAll();
  vi.useRealTimers();
  vi.restoreAllMocks();
  // One test opens the settings window the way a new window would, through its URL.
  window.history.replaceState(null, '', '/');
});

async function open(
  initial?: Entry[],
  options?: { config?: unknown },
  props: { onnavigate?: (pane: PaneId) => void } = {},
) {
  if (initial) backend(initial, options);
  const { target } = render(HistoryPanel, props);
  await settle();
  return target;
}

const called = (command: string) => mocks.invoke.mock.calls.filter(([name]) => name === command);
const searchField = (target: Element) => target.querySelector<HTMLInputElement>('input[type="search"]')!;
const clearButton = (target: Element) => button(target.querySelector('header.toolbar')!, 'Clear');
const hasButton = (root: Element, text: string) => [...root.querySelectorAll('button')].some(b => b.textContent?.includes(text));

const cards = (target: Element) => [...target.querySelectorAll<HTMLElement>('.entry-card')];
const textsOf = (target: Element) => [...target.querySelectorAll('.entry-text')].map(text => text.textContent);
const daysOf = (target: Element) => [...target.querySelectorAll('h2.day')].map(heading => heading.textContent);
const copyOf = (card: Element) => card.querySelector<HTMLButtonElement>('button[aria-label="Copy"]')!;
const deleteOf = (card: Element) => card.querySelector<HTMLButtonElement>('button[aria-label="Delete"]')!;
/** Runs of spaces are one space on screen, and the time of day can hold a no-break one. */
const plain = (text: string | null | undefined) => (text ?? '').replace(/\s+/g, ' ').trim();
/** What an entry says about itself under its text, as it reads. */
const metaOf = (card: Element) => plain(card.querySelector('.entry-meta')?.textContent);
const clock = (ms: number) => plain(new Date(ms).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' }));

/** A sheet's buttons as [name, type], left to right. */
const buttonsOf = (container: Element) =>
  [...container.querySelectorAll('button')].map(b => [b.textContent?.trim(), b.type]);

/**
 * Leaves the focus on the page, as a browser does when the button that holds it is disabled or
 * removed. jsdom keeps a disabled button focused and ignores blur() on it, so an element is
 * focused and then removed instead.
 */
function loseFocus() {
  const stray = document.body.appendChild(document.createElement('button'));
  stray.focus();
  stray.remove();
}

/** What a form does when Enter is pressed in a field. */
function submit(target: Element) {
  dialog(target)!.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
}

describe('History pane: toolbar and entries', () => {
  it('is titled History, with a search field and a Clear… button in its toolbar', async () => {
    const target = await open([entry('Hello')]);
    expect(target.querySelector('h1')?.textContent).toBe('History');
    const toolbar = target.querySelector('header.toolbar')!;
    const search = toolbar.querySelector<HTMLInputElement>('input[type="search"]')!;
    expect(search.getAttribute('aria-label')).toBe('Search history');
    expect(search.placeholder).toBe('Search');
    const clear = button(toolbar, 'Clear');
    expect(clear.textContent?.trim()).toBe('Clear…');
    expect(clear.type).toBe('button');
    expect(clear.classList.contains('btn')).toBe(true);
    expect(clear.classList.contains('btn-small')).toBe(true);
    expect(clear.disabled).toBe(false);
  });

  it('shows each entry with its text, line breaks kept', async () => {
    const target = await open([entry('First line\nSecond line'), entry('Another')]);
    expect(cards(target)).toHaveLength(2);
    expect(textsOf(target)).toEqual(['First line\nSecond line', 'Another']);
    // Each entry is its own root, directly in the group, so that the group's separators fall between entries.
    for (const card of cards(target)) expect(card.parentElement?.classList.contains('group-body')).toBe(true);
  });

  it('puts the time, the voice command and the processing time under the text', async () => {
    const stamp = new Date(2026, 9, 5, 9, 15).getTime();
    const target = await open([
      entry('Sure, I will send it.', { timestamp_ms: stamp, processing_time_ms: 2100, command_name: 'reply' }),
      entry('A plain note', { timestamp_ms: stamp, processing_time_ms: 900 }),
    ]);
    const [withCommand, withoutCommand] = cards(target);
    expect(metaOf(withCommand)).toBe(`${clock(stamp)} · reply · 2.1 s`);
    expect(withCommand.querySelector('.entry-command')?.textContent).toBe('reply');
    expect(metaOf(withoutCommand)).toBe(`${clock(stamp)} · 0.9 s`);
    expect(withoutCommand.querySelector('.entry-command')).toBeNull();
  });

  it('names the icons of Copy and Delete, and uses no emoji', async () => {
    const target = await open([entry('Hello')]);
    const copy = copyOf(cards(target)[0]);
    const remove = deleteOf(cards(target)[0]);
    expect([copy.title, remove.title]).toEqual(['Copy', 'Delete']);
    expect([copy.type, remove.type]).toEqual(['button', 'button']);
    // An icon and nothing else: the name is the label.
    expect([copy.textContent?.trim(), remove.textContent?.trim()]).toEqual(['', '']);
    expect(copy.querySelector('svg')?.getAttribute('class')).toContain('lucide-copy');
    expect(remove.querySelector('svg')?.getAttribute('class')).toContain('lucide-trash-2');
    expect(target.textContent).not.toMatch(/\p{Extended_Pictographic}/u);
  });

  it('keeps Copy and Delete in the page for every entry, so the keyboard can reach them', async () => {
    const target = await open([entry('One'), entry('Two')]);
    for (const card of cards(target)) {
      for (const action of [copyOf(card), deleteOf(card)]) {
        action.focus();
        expect(document.activeElement).toBe(action);
      }
    }
  });

  it('reads the history setting once, next to the list', async () => {
    await open([entry('Hello')]);
    expect(called('get_config')).toHaveLength(1);
    expect(called('get_history')).toHaveLength(1);
  });
});

describe('History pane: empty and loading', () => {
  it('shows neither the list nor the empty state before history has loaded', async () => {
    backend([entry('Hello')]);
    const release = holding('get_history');
    const target = await open();
    // An empty history is only known once the backend has said so.
    expect(target.querySelector('.empty')).toBeNull();
    expect(target.querySelector('.day')).toBeNull();
    release(); await settle();
    expect(textsOf(target)).toEqual(['Hello']);
    expect(target.querySelector('.empty')).toBeNull();
  });

  it('says so when history cannot be loaded, and does not claim it is empty', async () => {
    failing('get_history', new Error('unavailable'));
    const target = await open();
    expect(errorToast(target)).toContain('Failed to load history');
    expect(errorToast(target)).toContain('unavailable');
    expect(target.querySelector('.empty')).toBeNull();
  });

  it('says there is no history yet, and has nothing to clear', async () => {
    const target = await open();
    expect(target.querySelector('.empty-title')?.textContent).toBe('No transcription history yet.');
    expect(target.querySelector('.empty-hint')?.textContent).toBe('Completed transcriptions will appear here.');
    expect(target.querySelector('.day')).toBeNull();
    expect(clearButton(target).disabled).toBe(true);
  });

  it('says nothing matches a search', async () => {
    vi.useFakeTimers();
    const target = await open([entry('apple pie')]);
    fill(searchField(target), 'zzz');
    await vi.advanceTimersByTimeAsync(300); await settle();
    expect(target.querySelector('.empty')?.textContent?.trim()).toBe('No transcriptions match your search.');
    expect(target.querySelector('.entry-card')).toBeNull();
  });
});

describe('History pane: saving is off', () => {
  it('shows saving is off and links to General', async () => {
    const onnavigate = vi.fn();
    const target = await open([entry('Entry')], { config: { save_history: false } }, { onnavigate });
    button(target, 'Turn On in General').click();
    expect(onnavigate).toHaveBeenCalledWith('general');
    expect(target.querySelector('.entry-text')?.textContent).toBe('Entry');
  });

  it('puts the notice in a group of its own above the entries', async () => {
    const target = await open([entry('Entry')], { config: { save_history: false } });
    const notice = target.querySelector('.group .row')!;
    expect(notice.querySelector('.row-label')?.textContent).toBe('History saving is off');
    expect(notice.querySelector('.row-detail')?.textContent).toBe("New transcriptions aren't saved.");
    const turnOn = button(notice, 'Turn On in General');
    expect(turnOn.type).toBe('button');
    expect(turnOn.classList.contains('btn-small')).toBe(true);
    // The first group of the pane, before the day heading.
    expect(notice.compareDocumentPosition(target.querySelector('h2.day')!) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(target.querySelectorAll('.group')).toHaveLength(2);
  });

  it('shows no notice while history is being saved', async () => {
    const target = await open([entry('Entry')], { config: { save_history: true } });
    expect(target.textContent).not.toContain('History saving is off');
    expect(hasButton(target, 'Turn On in General')).toBe(false);
  });

  it('treats a configuration it cannot read as history being saved, and still lists the entries', async () => {
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    for (const config of [undefined, null, 'oops', ['not', 'a', 'config'], {}]) {
      const target = await open([entry('Entry')], { config });
      expect(target.textContent, JSON.stringify(config)).not.toContain('History saving is off');
      expect(textsOf(target)).toEqual(['Entry']);
    }
    // A read that fails is no different.
    backend([entry('Entry')]);
    const working = mocks.invoke.getMockImplementation()!;
    mocks.invoke.mockImplementation(async (name: string, args?: unknown) => {
      if (name === 'get_config') throw new Error('unavailable');
      return working(name, args);
    });
    const target = await open();
    expect(target.textContent).not.toContain('History saving is off');
    expect(textsOf(target)).toEqual(['Entry']);
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('does not hold the list back while the setting is being read', async () => {
    backend([entry('Entry')], { config: { save_history: false } });
    const release = holding('get_config');
    const target = await open();
    expect(textsOf(target)).toEqual(['Entry']);
    expect(target.textContent).not.toContain('History saving is off');
    release(); await settle();
    expect(target.textContent).toContain('History saving is off');
  });

  it('shows the notice even when there is nothing yet to list', async () => {
    const target = await open([], { config: { save_history: false } });
    expect(target.textContent).toContain('History saving is off');
    expect(target.querySelector('.empty-title')?.textContent).toBe('No transcription history yet.');
  });

  it('takes the settings window to General', async () => {
    window.history.replaceState(null, '', '/?view=settings&pane=history');
    backend([entry('Entry')], {
      config: { hotkey: 'Ctrl+`', output_mode: 'clipboard', save_history: false, chinese_conversion: 'traditional' },
    });
    const { target } = render(SettingsPanel, {});
    await settle();
    expect(target.querySelector('.nav-item[aria-current="page"]')?.textContent).toContain('History');
    button(target, 'Turn On in General').click(); await settle();
    expect(target.querySelector('.nav-item[aria-current="page"]')?.textContent).toContain('General');
    expect(target.querySelector('h1')?.textContent).toBe('General');
  });
});

describe('History pane: days', () => {
  it('groups entries under day headings', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 9, 5, 10, 0));
    const target = await open([
      entry('This morning', { timestamp_ms: new Date(2026, 9, 5, 9, 30).getTime() }),
      entry('Earlier today', { timestamp_ms: new Date(2026, 9, 5, 8, 0).getTime() }),
      entry('Last night', { timestamp_ms: new Date(2026, 9, 4, 23, 0).getTime() }),
    ]);
    expect(daysOf(target)).toEqual(['Today', 'Yesterday']);
    // One group under each heading, in the order of the list.
    const groups = [...target.querySelectorAll('h2.day')].map(heading => [
      heading.textContent,
      [...heading.nextElementSibling!.querySelectorAll('.entry-text')].map(text => text.textContent),
    ]);
    expect(groups).toEqual([
      ['Today', ['This morning', 'Earlier today']],
      ['Yesterday', ['Last night']],
    ]);
    expect(target.querySelector('h2.day')!.nextElementSibling?.classList.contains('group')).toBe(true);
  });

  it('names the days and the times in English, whatever language the system is set to', async () => {
    // A Traditional Chinese Mac: what these two answer when they are not told a language.
    const dates = Date.prototype.toLocaleDateString;
    const times = Date.prototype.toLocaleTimeString;
    vi.spyOn(Date.prototype, 'toLocaleDateString').mockImplementation(function (
      this: Date, locales?: Intl.LocalesArgument, options?: Intl.DateTimeFormatOptions,
    ) {
      return dates.call(this, locales ?? 'zh-TW', options);
    });
    vi.spyOn(Date.prototype, 'toLocaleTimeString').mockImplementation(function (
      this: Date, locales?: Intl.LocalesArgument, options?: Intl.DateTimeFormatOptions,
    ) {
      return times.call(this, locales ?? 'zh-TW', options);
    });
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 9, 5, 10, 0)); // a Monday
    const target = await open([
      entry('Just now', { timestamp_ms: new Date(2026, 9, 5, 9, 15).getTime() }),
      entry('Four days ago', { timestamp_ms: new Date(2026, 9, 1, 14, 30).getTime() }),
      entry('Thirty days ago', { timestamp_ms: new Date(2026, 8, 5, 8, 5).getTime() }),
    ]);
    // The weekday for the one 4 days old, "Mon D, YYYY" for the one 30 days old.
    expect(daysOf(target)).toEqual(['Today', 'Thursday', 'Sep 5, 2026']);
    expect(cards(target).map(card => metaOf(card))).toEqual([
      '09:15 AM · 0.0 s',
      '02:30 PM · 0.0 s',
      '08:05 AM · 0.0 s',
    ]);
  });

  it('takes the heading of a day away with its last entry', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 9, 5, 10, 0));
    const target = await open([
      entry('This morning', { id: 'a', timestamp_ms: new Date(2026, 9, 5, 9, 30).getTime() }),
      entry('Last night', { id: 'b', timestamp_ms: new Date(2026, 9, 4, 23, 0).getTime() }),
    ]);
    deleteOf(cards(target)[1]).click(); await settle();
    expect(daysOf(target)).toEqual(['Today']);
    expect(textsOf(target)).toEqual(['This morning']);
  });
});

describe('History pane: the original', () => {
  it('toggles the original transcription', async () => {
    const target = await open([entry('Final text', { raw_text: 'raw text' })]);
    expect(target.querySelector('.entry-original')).toBeNull();
    button(target, 'Show original').click(); await settle();
    const original = target.querySelector('.entry-original')!;
    expect(original.textContent).toContain('Original');
    expect(original.textContent).toContain('raw text');
    expect(hasButton(target, 'Show original')).toBe(false);
    button(target, 'Hide original').click(); await settle();
    expect(target.querySelector('.entry-original')).toBeNull();
    expect(hasButton(target, 'Show original')).toBe(true);
    expect(hasButton(target, 'Hide original')).toBe(false);
  });

  it('shows the toggle after the processing time, in the line under the text', async () => {
    const stamp = new Date(2026, 9, 5, 9, 15).getTime();
    const target = await open([
      entry('Final text', { timestamp_ms: stamp, processing_time_ms: 1100, command_name: 'shorten', raw_text: 'raw' }),
    ]);
    expect(metaOf(cards(target)[0])).toBe(`${clock(stamp)} · shorten · 1.1 s · Show original`);
    expect(button(cards(target)[0], 'Show original').type).toBe('button');
  });

  it('offers no original when it is missing, blank, or the same as the final text', async () => {
    const target = await open([
      entry('Without'),
      entry('Blank', { raw_text: '   ' }),
      entry('Same', { raw_text: 'Same' }),
      entry('Same but for spaces', { raw_text: ' Same but for spaces\n' }),
    ]);
    expect(cards(target)).toHaveLength(4);
    expect(hasButton(target, 'Show original')).toBe(false);
  });

  it('opens one original at a time', async () => {
    const target = await open([
      entry('First', { raw_text: 'first original' }),
      entry('Second', { raw_text: 'second original' }),
    ]);
    const [first, second] = cards(target);
    button(first, 'Show original').click(); await settle();
    button(second, 'Show original').click(); await settle();
    expect(target.querySelectorAll('.entry-original')).toHaveLength(1);
    expect(second.querySelector('.entry-original')?.textContent).toContain('second original');
    expect(hasButton(first, 'Hide original')).toBe(false);
  });
});

describe('History pane: copying', () => {
  it('confirms a copy with a toast', async () => {
    const target = await open([entry('Final text')]);
    target.querySelector<HTMLButtonElement>('[aria-label="Copy"]')!.click(); await settle();
    expect(mocks.writeText).toHaveBeenCalledWith('Final text');
    expect(target.querySelector('.toast-success')?.textContent).toContain('Copied');
  });

  it('copies the final text, not the original', async () => {
    const target = await open([entry('Final text', { raw_text: 'raw text' })]);
    copyOf(cards(target)[0]).click(); await settle();
    expect(mocks.writeText).toHaveBeenCalledTimes(1);
    expect(mocks.writeText).toHaveBeenCalledWith('Final text');
  });

  it('says why the text could not be copied, and the failure goes with the next action', async () => {
    mocks.writeText.mockRejectedValueOnce(new Error('denied'));
    const target = await open([entry('Final text')]);
    copyOf(cards(target)[0]).click(); await settle();
    expect(errorToast(target)).toContain('Failed to copy');
    expect(errorToast(target)).toContain('denied');
    expect(target.querySelector('.toast-success')).toBeNull();
    // A copy that works must not be hidden behind the one that did not.
    copyOf(cards(target)[0]).click(); await settle();
    expect(errorToast(target)).toBeUndefined();
    expect(successToast(target)).toBe('Copied');
  });

  it('lets the confirmation go away by itself after two seconds', async () => {
    vi.useFakeTimers();
    const target = await open([entry('Final text')]);
    copyOf(cards(target)[0]).click(); await settle();
    expect(successToast(target)).toBe('Copied');
    vi.advanceTimersByTime(1900); await settle();
    expect(successToast(target)).toBe('Copied');
    vi.advanceTimersByTime(200); await settle();
    expect(target.querySelector('.toast')).toBeNull();
  });
});

describe('History pane: deleting an entry', () => {
  it('deletes an entry, takes it off the list, and says so', async () => {
    const target = await open([entry('First', { id: 'a' }), entry('Second', { id: 'b' })]);
    deleteOf(cards(target)[0]).click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('delete_history_entry', { id: 'a' });
    expect(textsOf(target)).toEqual(['Second']);
    expect(successToast(target)).toBe('Deleted');
    // No question first: the entry is one line of history.
    expect(dialog(target)).toBeNull();
  });

  it('keeps the entry, and says why, when it cannot be deleted', async () => {
    failing('delete_history_entry', new Error('disk full'), [entry('First', { id: 'a' })]);
    const target = await open();
    deleteOf(cards(target)[0]).click(); await settle();
    expect(errorToast(target)).toContain('Failed to delete');
    expect(errorToast(target)).toContain('disk full');
    expect(target.querySelector('.toast-success')).toBeNull();
    expect(textsOf(target)).toEqual(['First']);
  });

  it('deletes once when it is pressed again while the first is still being deleted', async () => {
    backend([entry('First', { id: 'a' }), entry('Second', { id: 'b' })]);
    const release = holding('delete_history_entry');
    const target = await open();
    const remove = deleteOf(cards(target)[0]);
    remove.click(); await settle();
    remove.click(); await settle();
    // The search field waits too: a search now would race the delete.
    expect(searchField(target).disabled).toBe(true);
    release(); await settle();
    expect(called('delete_history_entry')).toHaveLength(1);
    expect(errorToast(target)).toBeUndefined();
    expect(textsOf(target)).toEqual(['Second']);
    expect(searchField(target).disabled).toBe(false);
  });

  it('puts the focus on the search field when a delete from the keyboard takes its entry away', async () => {
    const target = await open([entry('First', { id: 'a' }), entry('Second', { id: 'b' })]);
    const user = userEvent.setup();
    deleteOf(cards(target)[0]).focus();
    await user.keyboard('{Enter}'); await settle();
    expect(textsOf(target)).toEqual(['Second']);
    expect(document.activeElement).toBe(searchField(target));
  });

  it('leaves the focus alone when the pointer deleted the entry', async () => {
    const target = await open([entry('First', { id: 'a' }), entry('Second', { id: 'b' })]);
    // A press on a button does not focus it in Safari, so nothing in the list holds the focus.
    deleteOf(cards(target)[0]).click(); await settle();
    expect(textsOf(target)).toEqual(['Second']);
    expect(document.activeElement).toBe(document.body);
  });

  it('keeps the focus on the button of an entry that could not be deleted', async () => {
    failing('delete_history_entry', new Error('disk full'), [entry('First', { id: 'a' })]);
    const target = await open();
    const user = userEvent.setup();
    const remove = deleteOf(cards(target)[0]);
    remove.focus();
    await user.keyboard('{Enter}'); await settle();
    expect(errorToast(target)).toContain('disk full');
    expect(document.activeElement).toBe(remove);
  });
});

describe('History pane: clearing', () => {
  it('asks first, in a sheet with Cancel before a destructive Clear History', async () => {
    const target = await open([entry('Hello')]);
    clearButton(target).click(); await settle();
    expect(dialogTitle(target)).toBe('Clear all history?');
    expect(dialog(target)!.textContent).toContain("This deletes every saved transcription and can't be undone.");
    expect(buttonsOf(dialog(target)!)).toEqual([['Cancel', 'button'], ['Clear History', 'submit']]);
    expect(button(dialog(target)!, 'Clear History').classList.contains('btn-destructive')).toBe(true);
    // The safe answer has the focus.
    expect(document.activeElement).toBe(button(dialog(target)!, 'Cancel'));
    // Asking is not clearing.
    expect(called('clear_history')).toHaveLength(0);
    expect(target.querySelector('.pane [role="dialog"]')).not.toBeNull();
  });

  it('clears every entry after confirming, and says so', async () => {
    const target = await open([entry('One'), entry('Two')]);
    clearButton(target).click(); await settle();
    button(dialog(target)!, 'Clear History').click(); await settle();
    expect(called('clear_history')).toHaveLength(1);
    expect(dialog(target)).toBeNull();
    expect(target.querySelector('.entry-card')).toBeNull();
    expect(successToast(target)).toBe('History cleared');
    expect(target.querySelector('.empty-title')?.textContent).toBe('No transcription history yet.');
    expect(clearButton(target).disabled).toBe(true);
  });

  it('puts the focus on the search field, the button that opened the sheet being disabled now', async () => {
    const target = await open([entry('One')]);
    const user = userEvent.setup();
    await user.click(clearButton(target)); await settle();
    await user.click(button(dialog(target)!, 'Clear History')); await settle();
    expect(dialog(target)).toBeNull();
    expect(document.activeElement).toBe(searchField(target));
  });

  it('closes on Cancel without clearing, and puts the focus back on Clear…', async () => {
    const target = await open([entry('One')]);
    const user = userEvent.setup();
    const opener = clearButton(target);
    await user.click(opener); await settle();
    await user.click(button(dialog(target)!, 'Cancel')); await settle();
    expect(dialog(target)).toBeNull();
    expect(called('clear_history')).toHaveLength(0);
    expect(textsOf(target)).toEqual(['One']);
    expect(document.activeElement).toBe(opener);
  });

  it('closes on Escape without clearing', async () => {
    const target = await open([entry('One')]);
    const user = userEvent.setup();
    const opener = clearButton(target);
    await user.click(opener); await settle();
    await user.keyboard('{Escape}'); await settle();
    expect(dialog(target)).toBeNull();
    expect(called('clear_history')).toHaveLength(0);
    expect(document.activeElement).toBe(opener);
  });

  it('cancels, never clears, on the Enter that follows opening the sheet', async () => {
    const target = await open([entry('One')]);
    const user = userEvent.setup();
    await user.click(clearButton(target)); await settle();
    // The focus starts on Cancel, so the first Enter is the safe answer.
    await user.keyboard('{Enter}'); await settle();
    expect(dialog(target)).toBeNull();
    expect(called('clear_history')).toHaveLength(0);
  });

  it('clears on Enter once the focus is on Clear History', async () => {
    const target = await open([entry('One')]);
    const user = userEvent.setup();
    await user.click(clearButton(target)); await settle();
    await user.tab();
    expect(document.activeElement).toBe(button(dialog(target)!, 'Clear History'));
    await user.keyboard('{Enter}'); await settle();
    expect(called('clear_history')).toHaveLength(1);
  });

  it('keeps the sheet and the entries, and says why, when history cannot be cleared', async () => {
    failing('clear_history', new Error('disk full'), [entry('One')]);
    const target = await open();
    clearButton(target).click(); await settle();
    button(dialog(target)!, 'Clear History').click(); await settle();
    expect(errorToast(target)).toContain('Failed to clear history');
    expect(errorToast(target)).toContain('disk full');
    expect(target.querySelector('.toast-success')).toBeNull();
    expect(dialogTitle(target)).toBe('Clear all history?');
    expect(textsOf(target)).toEqual(['One']);
    // It can be tried again.
    expect(button(dialog(target)!, 'Clear History').disabled).toBe(false);
  });

  it('takes the failure with it when the sheet it came from is cancelled', async () => {
    failing('clear_history', new Error('disk full'), [entry('One')]);
    const target = await open();
    clearButton(target).click(); await settle();
    button(dialog(target)!, 'Clear History').click(); await settle();
    expect(errorToast(target)).toContain('disk full');
    button(dialog(target)!, 'Cancel').click(); await settle();
    expect(dialog(target)).toBeNull();
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('clears the toast of an earlier action when the sheet is opened', async () => {
    const target = await open([entry('One')]);
    copyOf(cards(target)[0]).click(); await settle();
    expect(successToast(target)).toBe('Copied');
    clearButton(target).click(); await settle();
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('clears once when it is confirmed again while the first is still being cleared', async () => {
    backend([entry('One')]);
    const release = holding('clear_history');
    const target = await open();
    clearButton(target).click(); await settle();
    const confirm = button(dialog(target)!, 'Clear History');
    confirm.click(); await settle();
    expect(confirm.disabled).toBe(true);
    submit(target); await settle();
    release(); await settle();
    expect(called('clear_history')).toHaveLength(1);
    expect(dialog(target)).toBeNull();
    expect(errorToast(target)).toBeUndefined();
    expect(successToast(target)).toBe('History cleared');
  });

  it('still closes on Escape after a failed clear took the focus out of the sheet', async () => {
    backend([entry('One')]);
    const release = holding('clear_history', new Error('disk full'));
    const target = await open();
    const user = userEvent.setup();
    await user.click(clearButton(target)); await settle();
    await user.click(button(dialog(target)!, 'Clear History')); await settle();
    // The button that held the focus is disabled while the request is out, and the focus goes to the page.
    loseFocus();
    release(); await settle();
    expect(errorToast(target)).toContain('disk full');
    expect(document.activeElement).toBe(document.body);
    await user.keyboard('{Escape}'); await settle();
    expect(dialog(target)).toBeNull();
    expect(called('clear_history')).toHaveLength(1);
  });
});

describe('History pane: paging', () => {
  it('offers Load More when a full page came back', async () => {
    const target = await open(many(50));
    expect(cards(target)).toHaveLength(50);
    const more = button(target, 'Load More');
    expect(more.type).toBe('button');
    expect(more.disabled).toBe(false);
  });

  it('offers no Load More when the page was not full', async () => {
    const target = await open(many(49));
    expect(cards(target)).toHaveLength(49);
    expect(hasButton(target, 'Load More')).toBe(false);
  });

  it('loads more as a bigger snapshot from the start', async () => {
    const target = await open(many(120));
    button(target, 'Load More').click(); await settle();
    expect(called('get_history').map(([, args]) => args)).toEqual([
      { offset: 0, limit: 50 },
      { offset: 0, limit: 100 },
    ]);
    expect(cards(target)).toHaveLength(100);
    expect(textsOf(target)[99]).toBe('Entry 99');
    // The next page is still to come.
    expect(hasButton(target, 'Load More')).toBe(true);
  });

  it('stops at 500 entries', async () => {
    const target = await open(many(600));
    for (let pages = 0; pages < 20 && hasButton(target, 'Load More'); pages++) {
      button(target, 'Load More').click(); await settle();
    }
    expect(cards(target)).toHaveLength(500);
    expect(hasButton(target, 'Load More')).toBe(false);
    expect(Math.max(...called('get_history').map(([, args]) => (args as { limit: number }).limit))).toBe(500);
  });

  it('asks for the same snapshot again when more could not be loaded', async () => {
    backend(many(120));
    const working = mocks.invoke.getMockImplementation()!;
    let failNext = false;
    mocks.invoke.mockImplementation(async (name: string, args?: unknown) => {
      if (name === 'get_history' && failNext) { failNext = false; throw new Error('Temporary read failure'); }
      return working(name, args);
    });
    const target = await open();
    failNext = true;
    button(target, 'Load More').click(); await settle();
    expect(errorToast(target)).toContain('Failed to load history');
    expect(cards(target)).toHaveLength(50);
    button(target, 'Load More').click(); await settle();
    expect(called('get_history').map(([, args]) => args)).toEqual([
      { offset: 0, limit: 50 },
      { offset: 0, limit: 100 },
      { offset: 0, limit: 100 },
    ]);
    expect(cards(target)).toHaveLength(100);
    expect(errorToast(target)).toBeUndefined();
  });

  it('loads once when Load More is pressed again while it is loading', async () => {
    const target = await open(many(120));
    // Only the next request is held; the first page is already on screen.
    const release = holding('get_history');
    const more = button(target, 'Load More');
    more.click(); await settle();
    more.click(); await settle();
    release(); await settle();
    expect(called('get_history')).toHaveLength(2);
    expect(cards(target)).toHaveLength(100);
  });
});

describe('History pane: searching', () => {
  it('searches 300 ms after typing stops, and lists everything again when the search is emptied', async () => {
    vi.useFakeTimers();
    const target = await open([entry('apple pie'), entry('banana')]);
    fill(searchField(target), 'app');
    await vi.advanceTimersByTimeAsync(299); await settle();
    expect(called('search_history')).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(2); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('search_history', { query: 'app' });
    expect(textsOf(target)).toEqual(['apple pie']);
    fill(searchField(target), ''); await vi.advanceTimersByTimeAsync(300); await settle();
    expect(called('get_history')).toHaveLength(2);
    expect(textsOf(target)).toEqual(['apple pie', 'banana']);
  });

  it('treats a search of only spaces as no search', async () => {
    vi.useFakeTimers();
    const target = await open([entry('apple pie')]);
    fill(searchField(target), '   ');
    await vi.advanceTimersByTimeAsync(300); await settle();
    expect(called('search_history')).toHaveLength(0);
    expect(textsOf(target)).toEqual(['apple pie']);
  });

  it('offers no Load More among search results', async () => {
    vi.useFakeTimers();
    const target = await open(many(50));
    expect(hasButton(target, 'Load More')).toBe(true);
    fill(searchField(target), 'Entry');
    await vi.advanceTimersByTimeAsync(300); await settle();
    expect(called('search_history')).toHaveLength(1);
    expect(hasButton(target, 'Load More')).toBe(false);
  });
});
