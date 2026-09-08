import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, tick, unmount, type Component } from 'svelte';
import userEvent from '@testing-library/user-event';
import WaveformIndicator from '../components/overlay/WaveformIndicator.svelte';
import FloatingOverlay from '../components/overlay/FloatingOverlay.svelte';
import AboutSection from '../components/settings/AboutSection.svelte';
import ProviderConfig from '../components/settings/ProviderConfig.svelte';
import HistoryPanel from '../components/history/HistoryPanel.svelte';
import DictionaryEditor from '../components/settings/DictionaryEditor.svelte';
import OutputConfig from '../components/settings/OutputConfig.svelte';
import StatusRowHarness from './StatusRowHarness.svelte';

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(), listen: vi.fn(), check: vi.fn(), startDragging: vi.fn(),
}));
vi.mock('../lib/tauri', () => ({ safeInvoke: mocks.invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }));
vi.mock('@tauri-apps/api/app', () => ({ getVersion: async () => '1.0.0' }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ startDragging: mocks.startDragging }) }));
vi.mock('@tauri-apps/plugin-updater', () => ({ check: mocks.check }));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: vi.fn() }));
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText: vi.fn().mockResolvedValue(undefined) }));

let mounted: ReturnType<typeof mount>[] = [];
let listeners: Map<string, (event: { payload: unknown }) => void>;

function render<P extends Record<string, unknown>>(component: Component<P>, props: P) {
  const target = document.createElement('div');
  document.body.append(target);
  const instance = mount(component, { target, props });
  mounted.push(instance);
  flushSync();
  return { target, instance };
}

async function settle() {
  // Resolve the sequential IPC listener registration and Svelte DOM flushes.
  for (let i = 0; i < 20; i++) await tick();
}

function emit(name: string, payload: unknown) {
  const listener = listeners.get(name);
  expect(listener, `${name} listener`).toBeDefined();
  listener!({ payload });
  flushSync();
}

function button(target: Element, text: string) {
  const result = [...target.querySelectorAll('button')].find(b => b.textContent?.includes(text));
  expect(result, `button ${text}`).toBeDefined();
  return result!;
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

beforeEach(() => {
  listeners = new Map();
  mocks.invoke.mockReset();
  mocks.check.mockReset();
  mocks.listen.mockReset().mockImplementation(async (name, callback) => {
    listeners.set(name, callback);
    return () => listeners.delete(name);
  });
  mocks.startDragging.mockResolvedValue(undefined);
});

afterEach(async () => {
  for (const instance of mounted) await unmount(instance);
  mounted = [];
  document.body.replaceChildren();
  vi.useRealTimers();
});

describe('recording overlay', () => {
  it('renders bounded waveform bars without mutating state during derivation', () => {
    const { target } = render(WaveformIndicator, { rms: 0.5, voiceActive: true });
    const bars = [...target.querySelectorAll<HTMLElement>('.bar')];
    expect(bars).toHaveLength(24);
    expect(bars.every(bar => Number.parseFloat(bar.style.height) >= 8 && Number.parseFloat(bar.style.height) <= 100)).toBe(true);
    expect(new Set(bars.map(bar => bar.style.height)).size).toBeGreaterThan(1);
  });

  it('resets previous transcript and errors when a hotkey starts another recording', async () => {
    const { target } = render(FloatingOverlay, { status: 'Ready' });
    await settle();
    emit('pipeline-result', { text: 'Previous recording', processing_time_ms: 10 });
    expect(target.textContent).toContain('Text ready');
    expect(target.textContent).not.toContain('Copied to clipboard');
    emit('pipeline-state', { state: 'done' });
    emit('pipeline-error', { message: 'Previous error', recoverable: true });
    emit('command-detected', { command_name: 'shorten' });
    emit('pipeline-state', { state: 'recording' });
    emit('recording-state', { is_recording: true });
    emit('transcription-committed', { text: 'New recording' });
    await settle();
    expect(target.textContent).toContain('New recording');
    expect(target.textContent).not.toContain('Previous recording');
    expect(target.textContent).not.toContain('Previous error');
    expect(target.querySelector('.result-indicator')).toBeNull();
    expect(target.querySelectorAll('.bar')).toHaveLength(24);
    emit('pipeline-state', { state: 'processing' });
    emit('recording-state', { is_recording: false });
    expect(target.textContent).toContain('Processing...');
    expect(target.textContent).not.toContain('Shortening...');
  });

  it('offers to cancel while processing and routes the button through the backend toggle', async () => {
    mocks.invoke.mockResolvedValue(undefined);
    const { target } = render(FloatingOverlay, { status: 'Ready' });
    await settle();
    emit('pipeline-state', { state: 'recording' });
    emit('recording-state', { is_recording: true });
    emit('pipeline-state', { state: 'processing' });
    emit('recording-state', { is_recording: false });
    button(target, 'Cancel').click();
    await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('toggle_recording');
    emit('pipeline-state', { state: 'idle' });
    await settle();
    expect(target.textContent).toContain('Cancelled');
    expect(target.textContent).not.toContain('Processing...');
  });

  it('uses native dragging for the window surface and excludes its controls', async () => {
    const { target } = render(FloatingOverlay, { status: 'Ready' });
    target.querySelector('.app-title')!.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
    expect(mocks.startDragging).toHaveBeenCalledTimes(1);
    target.querySelector('.record-button')!.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
    target.querySelector('.app-title')!.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 2 }));
    expect(mocks.startDragging).toHaveBeenCalledTimes(1);
  });
});

describe('updater and lifecycle', () => {
  it('turns a background notification into a downloadable update and waits for installation', async () => {
    const installation = deferred<void>();
    const update = {
      version: '2.0.0', body: 'New release', close: vi.fn().mockResolvedValue(undefined),
      downloadAndInstall: vi.fn().mockImplementation(callback => {
        callback({ event: 'Finished' });
        return installation.promise;
      }),
    };
    mocks.check.mockResolvedValue(update);
    const { target } = render(AboutSection, {});
    await settle();
    emit('update-available', { version: '2.0.0' });
    await settle();
    expect(mocks.check).toHaveBeenCalledTimes(1);
    button(target, 'Download & Install').click();
    await settle();
    expect(update.downloadAndInstall).toHaveBeenCalledTimes(1);
    expect(target.textContent).not.toContain('Restart Now');
    installation.resolve();
    await settle();
    expect(target.textContent).toContain('Restart Now');
  });

  it('unsubscribes even when listener registration resolves after unmount', async () => {
    const registration = deferred<() => void>();
    const unlisten = vi.fn();
    mocks.listen.mockReturnValue(registration.promise);
    const { instance } = render(AboutSection, {});
    await unmount(instance);
    mounted = [];
    registration.resolve(unlisten);
    await settle();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('closes update resources received after the settings tab was destroyed', async () => {
    const checking = deferred<unknown>();
    const close = vi.fn().mockResolvedValue(undefined);
    mocks.check.mockReturnValue(checking.promise);
    const { target, instance } = render(AboutSection, {});
    button(target, 'Check for Updates').click();
    await unmount(instance);
    mounted = [];
    checking.resolve({ version: '2.0.0', close });
    await settle();
    expect(close).toHaveBeenCalledTimes(1);
  });

  it('cleans up a provider subscription after switching tabs during initialization', async () => {
    const registration = deferred<() => void>();
    const unlisten = vi.fn();
    mocks.listen.mockReturnValue(registration.promise);
    mocks.invoke.mockImplementation(async command => command === 'get_stt_providers' ? [] : { stt_provider: 'openai' });
    const { instance } = render(ProviderConfig, {});
    await unmount(instance);
    mounted = [];
    registration.resolve(unlisten);
    await settle();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});

describe('history snapshots', () => {
  it('keeps all remaining records after deleting from a partially loaded list', async () => {
    let records = Array.from({ length: 100 }, (_, i) => ({ id: `${i}`, final_text: `Entry ${i}`, timestamp_ms: 0, processing_time_ms: 0 }));
    mocks.invoke.mockImplementation(async (command, args) => {
      if (command === 'get_history') return records.slice(args.offset, args.offset + args.limit);
      if (command === 'delete_history_entry') records = records.filter(entry => entry.id !== args.id);
    });
    const { target } = render(HistoryPanel, {});
    await settle();
    target.querySelector<HTMLButtonElement>('button[title="Delete"]')!.click();
    await settle();
    button(target, 'Load more').click();
    await settle();
    expect(target.querySelectorAll('.entry-card')).toHaveLength(99);
    expect([...target.querySelectorAll('.entry-text')].map(element => element.textContent)).toEqual(records.map(entry => entry.final_text));
  });

  it('retries the same snapshot size after a failed load and avoids insertion duplicates', async () => {
    let records = Array.from({ length: 120 }, (_, i) => ({ id: `${i}`, final_text: `Entry ${i}`, timestamp_ms: 0, processing_time_ms: 0 }));
    let failNext = false;
    mocks.invoke.mockImplementation(async (_command, args) => {
      if (failNext) { failNext = false; throw new Error('Temporary read failure'); }
      return records.slice(args.offset, args.offset + args.limit);
    });
    const { target } = render(HistoryPanel, {});
    await settle();
    failNext = true;
    button(target, 'Load more').click();
    await settle();
    records = [{ id: 'new', final_text: 'Newest entry', timestamp_ms: 0, processing_time_ms: 0 }, ...records];
    button(target, 'Load more').click();
    await settle();
    const requests = mocks.invoke.mock.calls.map(([, args]) => args);
    expect(requests).toEqual([{ offset: 0, limit: 50 }, { offset: 0, limit: 100 }, { offset: 0, limit: 100 }]);
    expect(target.querySelectorAll('.entry-card')).toHaveLength(100);
    expect(target.querySelector('.entry-text')?.textContent).toBe('Newest entry');
    expect(new Set([...target.querySelectorAll('.entry-text')].map(element => element.textContent)).size).toBe(100);
  });

  it('ignores a stale search response and cancels debounced searches on unmount', async () => {
    vi.useFakeTimers();
    const first = deferred<unknown[]>();
    const second = deferred<unknown[]>();
    mocks.invoke.mockImplementation(async (command, args) => {
      if (command === 'get_history') return [];
      return args.query === 'first' ? first.promise : second.promise;
    });
    const { target, instance } = render(HistoryPanel, {});
    await settle();
    const input = target.querySelector('input')!;
    input.value = 'first'; input.dispatchEvent(new Event('input', { bubbles: true }));
    await vi.advanceTimersByTimeAsync(300);
    input.value = 'second'; input.dispatchEvent(new Event('input', { bubbles: true }));
    await vi.advanceTimersByTimeAsync(300);
    second.resolve([{ id: 'second', final_text: 'Newest search result', timestamp_ms: 0, processing_time_ms: 0 }]);
    await settle();
    first.resolve([{ id: 'first', final_text: 'Stale result', timestamp_ms: 0, processing_time_ms: 0 }]);
    await settle();
    expect(target.textContent).toContain('Newest search result');
    expect(target.textContent).not.toContain('Stale result');
    input.value = 'third'; input.dispatchEvent(new Event('input', { bubbles: true }));
    const callsBeforeUnmount = mocks.invoke.mock.calls.length;
    await unmount(instance); mounted = [];
    await vi.advanceTimersByTimeAsync(300);
    expect(mocks.invoke).toHaveBeenCalledTimes(callsBeforeUnmount);
  });
});

it('makes provider selection and edit actions independently keyboard accessible', async () => {
  const select = vi.fn();
  const edit = vi.fn();
  const { target } = render(StatusRowHarness, { select, edit });
  const user = userEvent.setup();
  expect(target.querySelector('button button')).toBeNull();
  await user.tab();
  expect(document.activeElement?.textContent).toContain('Cloud provider');
  await user.keyboard('{Enter}');
  expect(select).toHaveBeenCalledTimes(1);
  await user.tab();
  expect(document.activeElement?.textContent).toBe('Edit key');
  await user.keyboard(' ');
  expect(edit).toHaveBeenCalledTimes(1);
  expect(select).toHaveBeenCalledTimes(1);
});

describe('modal dialogs', () => {
  it('moves focus into the clear-history dialog, keeps Tab inside it, and restores the opener on Escape', async () => {
    mocks.invoke.mockResolvedValue([{ id: '1', final_text: 'Entry', timestamp_ms: 0, processing_time_ms: 0 }]);
    const { target } = render(HistoryPanel, {});
    await settle();
    const user = userEvent.setup();
    const opener = button(target, 'Clear All');
    await user.click(opener);
    await settle();
    const dialog = target.querySelector<HTMLElement>('[role="dialog"]')!;
    expect(dialog).not.toBeNull();
    expect(document.activeElement).toBe(button(dialog, 'Cancel'));
    await user.tab();
    expect(document.activeElement).toBe(dialog.querySelector('.btn-danger'));
    await user.tab();
    expect(document.activeElement).toBe(button(dialog, 'Cancel'));
    await user.keyboard('{Escape}');
    await settle();
    expect(target.querySelector('[role="dialog"]')).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it('names the add-entry dialog after its heading', async () => {
    mocks.invoke.mockResolvedValue({ entries: [] });
    const { target } = render(DictionaryEditor, {});
    await settle();
    button(target, 'Add new word or correction').click();
    flushSync();
    const dialog = target.querySelector<HTMLElement>('[role="dialog"]')!;
    const labelledBy = dialog.getAttribute('aria-labelledby');
    expect(labelledBy).toBeTruthy();
    expect(target.querySelector(`#${labelledBy}`)?.textContent).toBe('Add Dictionary Entry');
  });

  it('focuses the first field of the add-entry dialog and wraps Shift+Tab to its last control', async () => {
    mocks.invoke.mockResolvedValue({ entries: [] });
    const { target } = render(DictionaryEditor, {});
    await settle();
    const user = userEvent.setup();
    const opener = button(target, 'Add new word or correction');
    await user.click(opener);
    await settle();
    const dialog = target.querySelector<HTMLElement>('[role="dialog"]')!;
    expect(document.activeElement).toBe(dialog.querySelector('#term'));
    await user.tab({ shift: true });
    expect(document.activeElement).toBe(button(dialog, 'Add Entry'));
    await user.keyboard('{Escape}');
    await settle();
    expect(target.querySelector('[role="dialog"]')).toBeNull();
    expect(document.activeElement).toBe(opener);
  });
});

describe('provider page initialization', () => {
  it('still loads providers when the optional progress listener cannot be registered', async () => {
    mocks.listen.mockImplementation(async (name, callback) => {
      if (name === 'apple-stt-model-progress') throw new Error('event plugin unavailable');
      listeners.set(name, callback);
      return () => listeners.delete(name);
    });
    mocks.invoke.mockImplementation(async command => command === 'get_stt_providers'
      ? [{ id: 'openai', name: 'OpenAI Whisper', description: 'Cloud transcription', configured: true }]
      : { stt_provider: 'openai', apple_stt_locale: 'auto', elevenlabs_language: 'auto', http_stt_config: null });
    const { target } = render(ProviderConfig, {});
    await settle();
    expect(target.textContent).toContain('OpenAI Whisper');
  });
});

describe('history opt-out', () => {
  it('shows the saved-history state and toggles it through the backend', async () => {
    mocks.invoke.mockImplementation(async command => command === 'get_config'
      ? { output_mode: 'clipboard', save_history: true }
      : undefined);
    const { target } = render(OutputConfig, {});
    await settle();
    const row = button(target, 'Save transcription history');
    expect(row.textContent).toContain('On');
    row.click();
    await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('set_save_history', { enabled: false });
    expect(button(target, 'Save transcription history').textContent).toContain('Off');
  });
});
