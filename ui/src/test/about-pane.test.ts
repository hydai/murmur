import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync } from 'svelte';
import userEvent from '@testing-library/user-event';
import AboutSection from '../components/settings/AboutSection.svelte';
import SettingsPanel from '../components/settings/SettingsPanel.svelte';
import { formatLogTimestamp } from '../components/settings/diagnostics';
import { button, errorToast, render, settle, successToast, unmountAll } from './helpers';

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(), listen: vi.fn(), getVersion: vi.fn(), check: vi.fn(), relaunch: vi.fn(), writeText: vi.fn(),
}));
vi.mock('../lib/tauri', () => ({ safeInvoke: mocks.invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }));
vi.mock('@tauri-apps/api/app', () => ({ getVersion: mocks.getVersion }));
vi.mock('@tauri-apps/plugin-updater', () => ({ check: mocks.check }));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: mocks.relaunch }));
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText: mocks.writeText }));

/** An entry as `get_diagnostic_logs` lists it. */
interface Entry {
  timestamp_ms: number;
  level: string;
  target: string;
  message: string;
}

const NOW = 1_700_000_000_000;
/** A warning from `lt_stt::custom` at `NOW`, unless `extra` says otherwise. */
const entry = (message: string, extra: Partial<Entry> = {}): Entry => ({
  timestamp_ms: NOW,
  level: 'warn',
  target: 'lt_stt::custom',
  message,
  ...extra,
});

/**
 * A backend that keeps the log the way the real one does: `get_diagnostic_logs` lists it,
 * oldest first, and `clear_diagnostic_logs` empties it. Every other command succeeds.
 */
function backend(initial: Entry[] = []) {
  const state = { entries: initial.map(item => ({ ...item })) };
  mocks.invoke.mockImplementation(async (command: string) => {
    switch (command) {
      case 'get_diagnostic_logs':
        return state.entries.map(item => ({ ...item }));
      case 'clear_diagnostic_logs':
        state.entries.length = 0;
        return undefined;
      default: return undefined;
    }
  });
  return state;
}

/** Like `backend`, except `command` rejects with `error`. The page logs that, so the log is silenced. */
function failing(command: string, error: Error, initial: Entry[] = []) {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  backend(initial);
  const working = mocks.invoke.getMockImplementation()!;
  mocks.invoke.mockImplementation(async (name: string, args?: unknown) => {
    if (name === command) throw error;
    return working(name, args);
  });
}

/** `command` is answered, as `backend` would, only once the returned `release` is called. */
function holding(command: string) {
  let release!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  const working = mocks.invoke.getMockImplementation()!;
  mocks.invoke.mockImplementation(async (name: string, args?: unknown) => {
    if (name === command) await held;
    return working(name, args);
  });
  return release;
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((yes) => { resolve = yes; });
  return { promise, resolve };
}

/** What `check()` answers when an update is out. */
function update(extra: Record<string, unknown> = {}) {
  return {
    version: '2.0.0',
    body: 'New release',
    close: vi.fn().mockResolvedValue(undefined),
    downloadAndInstall: vi.fn().mockResolvedValue(undefined),
    ...extra,
  };
}

let listeners: Map<string, (event: { payload: unknown }) => void>;

/** Sends an event to whoever listens for it, as the backend would. */
function emit(name: string, payload: unknown) {
  const listener = listeners.get(name);
  expect(listener, `${name} listener`).toBeDefined();
  listener!({ payload });
  flushSync();
}

beforeEach(() => {
  listeners = new Map();
  mocks.listen.mockReset().mockImplementation(async (name: string, callback: (event: { payload: unknown }) => void) => {
    listeners.set(name, callback);
    return () => listeners.delete(name);
  });
  mocks.getVersion.mockReset().mockResolvedValue('1.0.0');
  mocks.check.mockReset();
  mocks.relaunch.mockReset().mockResolvedValue(undefined);
  mocks.writeText.mockReset().mockResolvedValue(undefined);
  backend();
});

afterEach(async () => {
  await unmountAll();
  vi.useRealTimers();
  vi.restoreAllMocks();
  // One test opens the settings window the way a new window would, through its URL.
  window.history.replaceState(null, '', '/');
});

async function open() {
  const { target } = render(AboutSection, {});
  await settle();
  return target;
}

/** About with its Diagnostics Log open. */
async function openLog() {
  const target = await open();
  button(target, 'Diagnostics Log').click(); await settle();
  return target;
}

const heading = (target: Element) => target.querySelector('h1')?.textContent;
const called = (command: string) => mocks.invoke.mock.calls.filter(([name]) => name === command);
const backButton = (target: Element) =>
  target.querySelector<HTMLButtonElement>('[aria-label="Back to About"]')!;
const toolbar = (target: Element) => target.querySelector<HTMLElement>('header.toolbar .toolbar-actions')!;

function group(target: Element, title: string): HTMLElement {
  const found = [...target.querySelectorAll<HTMLElement>('.group')]
    .find(candidate => candidate.querySelector('.group-title')?.textContent === title);
  expect(found, `group ${title}`).toBeDefined();
  return found!;
}

const groupTitles = (target: Element) => [...target.querySelectorAll('.group-title')].map(title => title.textContent);

/** Every row of a group as [label, detail, trailing buttons]. */
const rowsOf = (container: Element) => [...container.querySelectorAll('.row')].map(row => [
  row.querySelector('.row-label')?.textContent,
  row.querySelector('.row-detail')?.textContent ?? null,
  [...row.querySelectorAll('.row-trailing button')].map(b => b.textContent?.trim()),
]);

/** Every entry on the page as [level, time, source, message], top to bottom. */
const logRows = (target: Element) => [...target.querySelectorAll('.log-row')].map(row => [
  row.querySelector('.level')?.textContent?.trim(),
  row.querySelector('.timestamp')?.textContent?.trim(),
  row.querySelector('.target')?.textContent?.trim(),
  row.querySelector('.message')?.textContent?.trim(),
]);

const isPrimary = (control: Element) => control.classList.contains('btn-primary');

describe('About pane', () => {
  it('shows the app name and version', async () => {
    const target = await open();
    expect(target.textContent).toContain('Murmur');
    expect(target.textContent).toContain('Version 1.0.0');
    expect(target.textContent).not.toContain('On-device processing');
    expect(target.textContent).not.toContain('Privacy-first voice typing');
  });

  it('puts the name and the version under the glyph, which is decoration', async () => {
    const target = await open();
    const glyph = target.querySelector<HTMLImageElement>('.glyph img')!;
    expect(glyph.getAttribute('alt')).toBe('');
    expect(glyph.getAttribute('src')).toContain('murmur-glyph');
    expect(target.querySelector('.app-name')?.textContent).toBe('Murmur');
    expect(target.querySelector('.app-version')?.textContent).toBe('Version 1.0.0');
  });

  it('says only Version until the version is known', async () => {
    const version = deferred<string>();
    mocks.getVersion.mockReturnValue(version.promise);
    const target = await open();
    expect(target.querySelector('.app-version')?.textContent).toBe('Version');
    expect(target.textContent).not.toContain('undefined');
    version.resolve('1.0.0'); await settle();
    expect(target.querySelector('.app-version')?.textContent).toBe('Version 1.0.0');
  });

  it('says only Version when the version cannot be read', async () => {
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    mocks.getVersion.mockRejectedValue(new Error('unavailable'));
    const target = await open();
    expect(target.querySelector('.app-version')?.textContent).toBe('Version');
  });

  it('has its three groups in order, under a heading and no back button', async () => {
    const target = await open();
    expect(heading(target)).toBe('About');
    expect(groupTitles(target)).toEqual(['Software Update', 'Links', 'Troubleshooting']);
    expect(target.querySelector('[aria-label^="Back"]')).toBeNull();
  });
});

describe('About pane: software update', () => {
  it('offers to check for updates, and does not check by itself', async () => {
    const target = await open();
    expect(rowsOf(group(target, 'Software Update'))).toEqual([['Software Update', null, ['Check for Updates']]]);
    expect(mocks.check).not.toHaveBeenCalled();
  });

  it('shows a spinner, and nothing to press, while it checks', async () => {
    mocks.check.mockReturnValue(deferred<unknown>().promise);
    const target = await open();
    button(target, 'Check for Updates').click(); await settle();
    const updates = group(target, 'Software Update');
    expect(rowsOf(updates)).toEqual([['Checking for updates…', null, []]]);
    expect(updates.querySelector('.spinner svg')).not.toBeNull();
    expect(updates.querySelector('button')).toBeNull();
  });

  it('says Murmur is up to date, and checks again on request', async () => {
    mocks.check.mockResolvedValue(null);
    const target = await open();
    button(target, 'Check for Updates').click(); await settle();
    expect(rowsOf(group(target, 'Software Update'))).toEqual([['Murmur is up to date', null, ['Check Again']]]);
    button(target, 'Check Again').click(); await settle();
    expect(mocks.check).toHaveBeenCalledTimes(2);
  });

  it('offers the update that is out, with its release notes under it', async () => {
    mocks.check.mockResolvedValue(update({ body: 'Fixes:\n- one\n- two' }));
    const target = await open();
    button(target, 'Check for Updates').click(); await settle();
    const updates = group(target, 'Software Update');
    expect(rowsOf(updates)).toEqual([['Version 2.0.0 is available', null, ['Download and Install']]]);
    expect(isPrimary(button(updates, 'Download and Install'))).toBe(true);
    // The line breaks are the release's own, and the page keeps them.
    expect(updates.querySelector('.release-notes')?.textContent).toBe('Fixes:\n- one\n- two');
  });

  it('has no release notes to show for a release that comes without any', async () => {
    mocks.check.mockResolvedValue(update({ body: null }));
    const target = await open();
    button(target, 'Check for Updates').click(); await settle();
    expect(rowsOf(group(target, 'Software Update'))[0][0]).toBe('Version 2.0.0 is available');
    expect(target.querySelector('.release-notes')).toBeNull();
  });

  it('can reach release notes that are longer than the box by keyboard', async () => {
    mocks.check.mockResolvedValue(update());
    const target = await open();
    button(target, 'Check for Updates').click(); await settle();
    const notes = target.querySelector<HTMLElement>('.release-notes')!;
    // A box that scrolls cannot be reached with Tab unless it takes the focus itself.
    expect(notes.tabIndex).toBe(0);
    expect(notes.getAttribute('aria-label')).toBe('Release notes');
  });

  it('shows how much of the update has come, with a bar in the same row', async () => {
    const installation = deferred<void>();
    let report!: (event: unknown) => void;
    mocks.check.mockResolvedValue(update({
      downloadAndInstall: vi.fn().mockImplementation(async (callback: (event: unknown) => void) => {
        report = callback;
        callback({ event: 'Started', data: { contentLength: 4096 } });
        callback({ event: 'Progress', data: { chunkLength: 1024 } });
        await installation.promise;
      }),
    }));
    const target = await open();
    button(target, 'Check for Updates').click(); await settle();
    button(target, 'Download and Install').click(); await settle();

    const updates = group(target, 'Software Update');
    expect(rowsOf(updates)).toEqual([['Downloading update…', '1 KB of 4 KB', []]]);
    const bar = updates.querySelector('.row [role="progressbar"]')!;
    expect(bar.getAttribute('aria-valuenow')).toBe('25');

    report({ event: 'Progress', data: { chunkLength: 3072 } }); await settle();
    expect(rowsOf(updates)[0][1]).toBe('4 KB of 4 KB');
    expect(bar.getAttribute('aria-valuenow')).toBe('100');
  });

  it('shows no size, and a bar that claims no share, when the download does not say how big it is', async () => {
    const installation = deferred<void>();
    mocks.check.mockResolvedValue(update({
      downloadAndInstall: vi.fn().mockImplementation(async (callback: (event: unknown) => void) => {
        callback({ event: 'Started', data: {} });
        callback({ event: 'Progress', data: { chunkLength: 1024 } });
        await installation.promise;
      }),
    }));
    const target = await open();
    button(target, 'Check for Updates').click(); await settle();
    button(target, 'Download and Install').click(); await settle();

    const updates = group(target, 'Software Update');
    expect(rowsOf(updates)).toEqual([['Downloading update…', null, []]]);
    expect(updates.querySelector('[role="progressbar"]')?.hasAttribute('aria-valuenow')).toBe(false);
  });

  it('asks to restart once the update is installed', async () => {
    mocks.check.mockResolvedValue(update());
    const target = await open();
    button(target, 'Check for Updates').click(); await settle();
    button(target, 'Download and Install').click(); await settle();
    const updates = group(target, 'Software Update');
    expect(rowsOf(updates)).toEqual([['Restart Murmur to finish updating', null, ['Restart Now']]]);
    expect(isPrimary(button(updates, 'Restart Now'))).toBe(true);
    expect(mocks.relaunch).not.toHaveBeenCalled();
    button(updates, 'Restart Now').click(); await settle();
    expect(mocks.relaunch).toHaveBeenCalledTimes(1);
  });

  it('offers to try again after a failed check', async () => {
    mocks.check.mockRejectedValue(new Error('network down'));
    const target = await open();
    button(target, 'Check for Updates').click(); await settle();
    expect(rowsOf(group(target, 'Software Update'))).toEqual([
      ["Couldn't check for updates", 'Error: network down', ['Try Again']],
    ]);

    mocks.check.mockResolvedValue(null);
    button(target, 'Try Again').click(); await settle();
    expect(rowsOf(group(target, 'Software Update'))).toEqual([['Murmur is up to date', null, ['Check Again']]]);
  });

  it('says the update could not be installed when the download fails, and checks again from the start on Try Again', async () => {
    const first = update({ downloadAndInstall: vi.fn().mockRejectedValue(new Error('disk full')) });
    mocks.check.mockResolvedValue(first);
    const target = await open();
    button(target, 'Check for Updates').click(); await settle();
    button(target, 'Download and Install').click(); await settle();
    // The check worked, so the headline does not say it failed; the reason and the button are the same.
    expect(rowsOf(group(target, 'Software Update'))).toEqual([
      ["Couldn't install the update", 'Error: disk full', ['Try Again']],
    ]);

    button(target, 'Try Again').click(); await settle();
    expect(mocks.check).toHaveBeenCalledTimes(2);
    // The update that failed is let go before the next check.
    expect(first.close).toHaveBeenCalledTimes(1);
  });

  it('runs the check it was asked for, once, and says that it did', async () => {
    mocks.check.mockResolvedValue(null);
    const consumed = vi.fn();
    const { target } = render(AboutSection, { pendingCheck: true, onCheckConsumed: consumed });
    await settle();
    expect(mocks.check).toHaveBeenCalledTimes(1);
    expect(consumed).toHaveBeenCalledTimes(1);
    expect(rowsOf(group(target, 'Software Update'))[0][0]).toBe('Murmur is up to date');
    // There was no log to leave, so a window opened for the check does not have its focus moved.
    expect(document.activeElement).toBe(document.body);
  });
});

describe('About pane: links and troubleshooting', () => {
  const LINKS = [
    ['Source Code on GitHub', 'https://github.com/hydai/murmur'],
    ['Release Notes', 'https://github.com/hydai/murmur/releases'],
    ['Report an Issue', 'https://github.com/hydai/murmur/issues'],
  ];

  it('lists the links, each with an external-link icon that is part of its button', async () => {
    const target = await open();
    const links = group(target, 'Links');
    expect(rowsOf(links).map(([label, detail]) => [label, detail])).toEqual(LINKS.map(([label]) => [label, null]));
    for (const row of links.querySelectorAll('.row')) {
      expect(row.querySelector('button.row-main .row-accessory svg')?.getAttribute('class'))
        .toContain('lucide-external-link');
      // A Row sets room aside for each part it is given, so nothing else may be handed over.
      expect(row.querySelector('.row-trailing')).toBeNull();
      expect(row.querySelector('.row-extra')).toBeNull();
    }
  });

  it('opens a link in a new window of the browser when its row is pressed', async () => {
    const opened = vi.spyOn(window, 'open').mockImplementation(() => null);
    const target = await open();
    for (const [label] of LINKS) button(group(target, 'Links'), label).click();
    expect(opened.mock.calls).toEqual(LINKS.map(([, url]) => [url, '_blank']));
  });

  it('has one row for the Diagnostics Log, which is a button chevron and all', async () => {
    const target = await open();
    const troubleshooting = group(target, 'Troubleshooting');
    expect(rowsOf(troubleshooting)).toEqual([['Diagnostics Log', 'Recent warnings and errors', []]]);
    expect(troubleshooting.querySelector('button.row-main .row-accessory svg')?.getAttribute('class'))
      .toContain('lucide-chevron-right');
    expect(troubleshooting.querySelector('.row-trailing')).toBeNull();
    expect(troubleshooting.querySelector('.row-extra')).toBeNull();
  });

  it('opens the diagnostics log and comes back', async () => {
    const target = await open();
    button(target, 'Diagnostics Log').click(); await settle();
    expect(target.querySelector('h1')?.textContent).toBe('Diagnostics Log');
    target.querySelector<HTMLButtonElement>('[aria-label="Back to About"]')!.click(); await settle();
    expect(target.querySelector('h1')?.textContent).toBe('About');
  });

  it('shows only the log while it is open, and About again when it is closed', async () => {
    const target = await openLog();
    expect(target.querySelectorAll('h1')).toHaveLength(1);
    expect(groupTitles(target)).not.toContain('Software Update');
    expect(target.querySelector('.glyph')).toBeNull();
    backButton(target).click(); await settle();
    expect(target.querySelectorAll('h1')).toHaveLength(1);
    expect(groupTitles(target)).toEqual(['Software Update', 'Links', 'Troubleshooting']);
    expect(backButton(target)).toBeNull();
  });

  it('puts the focus back on the Diagnostics Log row when it is closed', async () => {
    const target = await openLog();
    backButton(target).click(); await settle();
    expect(document.activeElement).toBe(button(target, 'Diagnostics Log'));
    expect(document.activeElement?.classList.contains('row-main')).toBe(true);
  });

  it('puts the focus back on the row when the log is closed from the keyboard', async () => {
    const target = await openLog();
    backButton(target).focus();
    await userEvent.setup().keyboard('{Enter}');
    await settle();
    expect(document.activeElement).toBe(button(target, 'Diagnostics Log'));
  });

  it('keeps what it knows about updates while the log is open', async () => {
    mocks.check.mockResolvedValue(update());
    const target = await open();
    button(target, 'Check for Updates').click(); await settle();
    button(target, 'Diagnostics Log').click(); await settle();
    backButton(target).click(); await settle();
    expect(rowsOf(group(target, 'Software Update'))[0][0]).toBe('Version 2.0.0 is available');
    expect(mocks.check).toHaveBeenCalledTimes(1);
  });

  it('leaves the log for About when the menu bar asks for an update check', async () => {
    mocks.check.mockResolvedValue(null);
    window.history.replaceState(null, '', '/?view=settings&pane=about');
    const { target } = render(SettingsPanel, {});
    await settle();
    button(target, 'Diagnostics Log').click(); await settle();
    expect(heading(target)).toBe('Diagnostics Log');
    expect(mocks.check).not.toHaveBeenCalled();

    // The window is open, so the request comes as an event rather than in its URL.
    emit('navigate', { pane: 'about', action: 'check-update' }); await settle();
    // The check and its answer are shown on About, so that is where the user is taken.
    expect(heading(target)).toBe('About');
    expect(mocks.check).toHaveBeenCalledTimes(1);
    expect(rowsOf(group(target, 'Software Update'))).toEqual([['Murmur is up to date', null, ['Check Again']]]);
    // As on Back, the keyboard carries on from the row that opened the log.
    expect(document.activeElement).toBe(button(target, 'Diagnostics Log'));
  });

  it('is what the About entry of the settings window shows', async () => {
    window.history.replaceState(null, '', '/?view=settings&pane=about');
    const { target } = render(SettingsPanel, {});
    await settle();
    expect(target.querySelector('main .pane h1')?.textContent).toBe('About');
    expect(groupTitles(target)).toEqual(['Software Update', 'Links', 'Troubleshooting']);
    // The log is only read once it is opened.
    expect(called('get_diagnostic_logs')).toHaveLength(0);
    button(target, 'Diagnostics Log').click(); await settle();
    expect(target.querySelector('main .pane h1')?.textContent).toBe('Diagnostics Log');
    expect(called('get_diagnostic_logs')).toHaveLength(1);
  });
});

describe('Diagnostics Log subpage', () => {
  it('is a page below About, with Refresh, Copy and Clear in its toolbar', async () => {
    const target = await openLog();
    expect(heading(target)).toBe('Diagnostics Log');
    expect(backButton(target)).not.toBeNull();
    const tools = [...toolbar(target).querySelectorAll('button')];
    expect(tools.map(tool => tool.textContent?.trim())).toEqual(['Refresh', 'Copy', 'Clear']);
    for (const tool of tools) {
      expect(tool.type).toBe('button');
      expect(tool.classList.contains('btn')).toBe(true);
      expect(tool.classList.contains('btn-small')).toBe(true);
    }
  });

  it('lists the entries newest first, each with its level, time, source and message', async () => {
    backend([
      entry('Custom STT request failed'),
      entry('Pipeline error', { timestamp_ms: NOW + 90_000, level: 'error', target: 'lt_pipeline' }),
    ]);
    const target = await openLog();
    expect(logRows(target)).toEqual([
      ['Error', formatLogTimestamp(NOW + 90_000), 'lt_pipeline', 'Pipeline error'],
      ['Warning', formatLogTimestamp(NOW), 'lt_stt::custom', 'Custom STT request failed'],
    ]);
    // In English, whatever the system's language is.
    for (const [, time] of logRows(target)) {
      expect(time).toMatch(/^\d{1,2}\/\d{1,2}\/\d{4}, \d{1,2}:\d{2}:\d{2}\s[AP]M$/);
    }
  });

  it('names the levels Warning and Error, and capitalizes any other', async () => {
    backend([
      entry('first', { level: 'info' }),
      entry('second', { level: 'debug' }),
      entry('third', { level: 'warn' }),
      entry('fourth', { level: 'error' }),
    ]);
    const target = await openLog();
    expect(logRows(target).map(([level]) => level)).toEqual(['Error', 'Warning', 'Debug', 'Info']);
  });

  it('marks a warning and an error apart from the other levels', async () => {
    backend([entry('first', { level: 'info' }), entry('second', { level: 'warn' }), entry('third', { level: 'error' })]);
    const target = await openLog();
    const marks = [...target.querySelectorAll('.log-row .level')]
      .map(level => [level.classList.contains('level-warn'), level.classList.contains('level-error')]);
    expect(marks).toEqual([[false, true], [true, false], [false, false]]);
  });

  it('keeps the line breaks of a message', async () => {
    backend([entry('first line\nsecond line')]);
    const target = await openLog();
    expect(target.querySelector('.log-row .message')?.textContent).toBe('first line\nsecond line');
  });

  it('says so when there is nothing to show, and has nothing to copy or clear', async () => {
    const target = await openLog();
    expect(target.querySelector('.log-row')).toBeNull();
    expect(target.textContent).toContain('No warnings or errors in this session.');
    expect(button(toolbar(target), 'Refresh').disabled).toBe(false);
    expect(button(toolbar(target), 'Copy').disabled).toBe(true);
    expect(button(toolbar(target), 'Clear').disabled).toBe(true);
  });

  it('turns Copy and Clear on once there is something to copy or clear', async () => {
    backend([entry('something')]);
    const target = await openLog();
    expect(button(toolbar(target), 'Copy').disabled).toBe(false);
    expect(button(toolbar(target), 'Clear').disabled).toBe(false);
    expect(target.textContent).not.toContain('No warnings or errors');
  });

  it('does not call the log empty before the backend has answered', async () => {
    backend([entry('something')]);
    const release = holding('get_diagnostic_logs');
    const target = await openLog();
    expect(target.textContent).not.toContain('No warnings or errors');
    expect(target.querySelector('.log-row')).toBeNull();
    release(); await settle();
    expect(logRows(target)).toHaveLength(1);
  });

  it('treats an answer without a list as an empty log', async () => {
    mocks.invoke.mockImplementation(async () => undefined);
    const target = await openLog();
    expect(target.querySelector('.toast')).toBeNull();
    expect(target.textContent).toContain('No warnings or errors in this session.');
  });

  it('says why the log could not be read, without calling it empty', async () => {
    failing('get_diagnostic_logs', new Error('unavailable'));
    const target = await openLog();
    expect(errorToast(target)).toContain('Failed to load diagnostics');
    expect(errorToast(target)).toContain('unavailable');
    expect(target.textContent).not.toContain('No warnings or errors');
  });

  it('reads the log again on Refresh, without a toast', async () => {
    const state = backend([entry('first')]);
    const target = await openLog();
    state.entries.push(entry('second', { timestamp_ms: NOW + 1000 }));
    button(toolbar(target), 'Refresh').click(); await settle();
    expect(logRows(target).map(row => row[3])).toEqual(['second', 'first']);
    expect(called('get_diagnostic_logs')).toHaveLength(2);
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('copies the log in the order the page shows it, and says so in a toast', async () => {
    backend([entry('older'), entry('newer', { timestamp_ms: NOW + 1000 })]);
    const target = await openLog();
    button(toolbar(target), 'Copy').click(); await settle();
    expect(mocks.writeText).toHaveBeenCalledTimes(1);
    const copied = mocks.writeText.mock.calls[0][0] as string;
    expect(copied.indexOf('newer')).toBeLessThan(copied.indexOf('older'));
    expect(successToast(target)).toBe('Copied');
  });

  it('says why the log could not be copied', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    backend([entry('something')]);
    mocks.writeText.mockRejectedValue(new Error('denied'));
    const target = await openLog();
    button(toolbar(target), 'Copy').click(); await settle();
    expect(errorToast(target)).toContain('Failed to copy diagnostics');
    expect(errorToast(target)).toContain('denied');
    expect(target.querySelector('.toast-success')).toBeNull();
  });

  it('clears the log, and says so in a toast', async () => {
    backend([entry('first'), entry('second')]);
    const target = await openLog();
    button(toolbar(target), 'Clear').click(); await settle();
    expect(called('clear_diagnostic_logs')).toHaveLength(1);
    expect(target.querySelector('.log-row')).toBeNull();
    expect(target.textContent).toContain('No warnings or errors in this session.');
    expect(successToast(target)).toBe('Log cleared');
    expect(button(toolbar(target), 'Copy').disabled).toBe(true);
    expect(button(toolbar(target), 'Clear').disabled).toBe(true);
  });

  it('keeps the entries, and says why, when the log cannot be cleared', async () => {
    failing('clear_diagnostic_logs', new Error('locked'), [entry('first')]);
    const target = await openLog();
    button(toolbar(target), 'Clear').click(); await settle();
    expect(errorToast(target)).toContain('Failed to clear diagnostics');
    expect(errorToast(target)).toContain('locked');
    expect(logRows(target)).toHaveLength(1);
    expect(target.querySelector('.toast-success')).toBeNull();
  });

  it('moves the focus to Refresh once the log is cleared, because Clear is off then', async () => {
    backend([entry('first')]);
    const target = await openLog();
    const clear = button(toolbar(target), 'Clear');
    clear.focus();
    await userEvent.setup().keyboard('{Enter}');
    await settle();
    expect(clear.disabled).toBe(true);
    expect(document.activeElement).toBe(button(toolbar(target), 'Refresh'));
  });

  it('leaves the focus on Clear when the log cannot be cleared', async () => {
    failing('clear_diagnostic_logs', new Error('locked'), [entry('first')]);
    const target = await openLog();
    const clear = button(toolbar(target), 'Clear');
    clear.focus();
    await userEvent.setup().keyboard('{Enter}');
    await settle();
    expect(errorToast(target)).toContain('Failed to clear diagnostics');
    expect(document.activeElement).toBe(clear);
  });

  it('lets the confirmation go away by itself after two seconds', async () => {
    vi.useFakeTimers();
    backend([entry('something')]);
    const target = await openLog();
    button(toolbar(target), 'Copy').click(); await settle();
    expect(successToast(target)).toBe('Copied');
    vi.advanceTimersByTime(1900); await settle();
    expect(successToast(target)).toBe('Copied');
    vi.advanceTimersByTime(200); await settle();
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('leaves no toast behind once it is closed', async () => {
    backend([entry('something')]);
    const target = await openLog();
    button(toolbar(target), 'Copy').click(); await settle();
    expect(successToast(target)).toBe('Copied');
    backButton(target).click(); await settle();
    expect(target.querySelector('.toast')).toBeNull();
  });
});
