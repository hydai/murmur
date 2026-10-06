import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, tick, unmount, type Component } from 'svelte';
import userEvent from '@testing-library/user-event';
import AboutSection from '../components/settings/AboutSection.svelte';
import ProviderConfig from '../components/settings/ProviderConfig.svelte';
import HistoryPanel from '../components/history/HistoryPanel.svelte';
import DictionaryEditor from '../components/settings/DictionaryEditor.svelte';
import GeneralConfig from '../components/settings/GeneralConfig.svelte';
import LlmConfig from '../components/settings/LlmConfig.svelte';
import PromptsEditor from '../components/settings/PromptsEditor.svelte';
import { resetDrafts } from '../components/settings/promptDrafts.svelte';
import DiagnosticsPanel from '../components/settings/DiagnosticsPanel.svelte';

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(), listen: vi.fn(), check: vi.fn(), writeText: vi.fn(),
}));
vi.mock('../lib/tauri', () => ({ safeInvoke: mocks.invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }));
vi.mock('@tauri-apps/api/app', () => ({ getVersion: async () => '1.0.0' }));
vi.mock('@tauri-apps/plugin-updater', () => ({ check: mocks.check }));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: vi.fn() }));
vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({ writeText: mocks.writeText }));

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
  mocks.writeText.mockReset().mockResolvedValue(undefined);
});

afterEach(async () => {
  for (const instance of mounted) await unmount(instance);
  mounted = [];
  document.body.replaceChildren();
  vi.useRealTimers();
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
    button(target, 'Download and Install').click();
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
      if (command === 'get_config') return { save_history: true };
      if (command === 'get_history') return records.slice(args.offset, args.offset + args.limit);
      if (command === 'delete_history_entry') records = records.filter(entry => entry.id !== args.id);
    });
    const { target } = render(HistoryPanel, {});
    await settle();
    target.querySelector<HTMLButtonElement>('button[title="Delete"]')!.click();
    await settle();
    button(target, 'Load More').click();
    await settle();
    expect(target.querySelectorAll('.entry-card')).toHaveLength(99);
    expect([...target.querySelectorAll('.entry-text')].map(element => element.textContent)).toEqual(records.map(entry => entry.final_text));
  });

  it('retries the same snapshot size after a failed load and avoids insertion duplicates', async () => {
    let records = Array.from({ length: 120 }, (_, i) => ({ id: `${i}`, final_text: `Entry ${i}`, timestamp_ms: 0, processing_time_ms: 0 }));
    let failNext = false;
    mocks.invoke.mockImplementation(async (command, args) => {
      if (command === 'get_config') return { save_history: true };
      if (failNext) { failNext = false; throw new Error('Temporary read failure'); }
      return records.slice(args.offset, args.offset + args.limit);
    });
    const { target } = render(HistoryPanel, {});
    await settle();
    failNext = true;
    button(target, 'Load More').click();
    await settle();
    records = [{ id: 'new', final_text: 'Newest entry', timestamp_ms: 0, processing_time_ms: 0 }, ...records];
    button(target, 'Load More').click();
    await settle();
    const requests = mocks.invoke.mock.calls.filter(([command]) => command === 'get_history').map(([, args]) => args);
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
      if (command === 'get_config') return { save_history: true };
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

describe('modal dialogs', () => {
  it('moves focus into the clear-history dialog, keeps Tab inside it, and restores the opener on Escape', async () => {
    mocks.invoke.mockResolvedValue([{ id: '1', final_text: 'Entry', timestamp_ms: 0, processing_time_ms: 0 }]);
    const { target } = render(HistoryPanel, {});
    await settle();
    const user = userEvent.setup();
    const opener = button(target, 'Clear');
    await user.click(opener);
    await settle();
    const dialog = target.querySelector<HTMLElement>('[role="dialog"]')!;
    expect(dialog).not.toBeNull();
    expect(document.activeElement).toBe(button(dialog, 'Cancel'));
    await user.tab();
    expect(document.activeElement).toBe(dialog.querySelector('.btn-destructive'));
    await user.tab();
    expect(document.activeElement).toBe(button(dialog, 'Cancel'));
    await user.keyboard('{Escape}');
    await settle();
    expect(target.querySelector('[role="dialog"]')).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it('names the add-word sheet after its heading', async () => {
    mocks.invoke.mockResolvedValue({ entries: [] });
    const { target } = render(DictionaryEditor, {});
    await settle();
    target.querySelector<HTMLButtonElement>('button[aria-label="Add Word"]')!.click();
    flushSync();
    const dialog = target.querySelector<HTMLElement>('[role="dialog"]')!;
    const labelledBy = dialog.getAttribute('aria-labelledby');
    expect(labelledBy).toBeTruthy();
    expect(target.querySelector(`#${labelledBy}`)?.textContent).toBe('Add Word');
  });

  it('focuses the first field of the add-word sheet and wraps Shift+Tab to its last control', async () => {
    mocks.invoke.mockResolvedValue({ entries: [] });
    const { target } = render(DictionaryEditor, {});
    await settle();
    const user = userEvent.setup();
    const opener = target.querySelector<HTMLButtonElement>('button[aria-label="Add Word"]')!;
    await user.click(opener);
    await settle();
    const dialog = target.querySelector<HTMLElement>('[role="dialog"]')!;
    expect(document.activeElement).toBe(dialog.querySelector('#term'));
    await user.tab({ shift: true });
    expect(document.activeElement).toBe(button(dialog, 'Add Word'));
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
      ? { hotkey: 'Ctrl+`', output_mode: 'clipboard', save_history: true }
      : undefined);
    const { target } = render(GeneralConfig, {});
    await settle();
    const toggle = () => target.querySelector<HTMLButtonElement>('[role="switch"][aria-label="Save transcription history"]')!;
    expect(toggle().getAttribute('aria-checked')).toBe('true');
    toggle().click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('set_save_history', { enabled: false });
    expect(toggle().getAttribute('aria-checked')).toBe('false');
  });
});

describe('chinese conversion setting', () => {
  it('shows the active conversion and switches it through the backend', async () => {
    mocks.invoke.mockImplementation(async command => command === 'get_config'
      ? { hotkey: 'Ctrl+`', output_mode: 'clipboard', save_history: true, chinese_conversion: 'traditional' }
      : undefined);
    const { target } = render(GeneralConfig, {});
    await settle();
    const select = target.querySelector<HTMLSelectElement>('select[aria-label="Chinese characters"]')!;
    expect(select.value).toBe('traditional');
    select.value = 'none'; select.dispatchEvent(new Event('change', { bubbles: true })); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('set_chinese_conversion', { mode: 'none' });
    expect(select.value).toBe('none');
  });
});

describe('prompt editor', () => {
  const prompt = (name: string, content: string) => ({
    name, title: name, description: '', required_placeholders: [],
    task_variant: 'post_process', content, is_override: false, default_content: content,
  });

  function mockPrompts(...entries: ReturnType<typeof prompt>[]) {
    mocks.invoke.mockImplementation(async command =>
      command === 'get_prompts' ? entries : undefined);
  }

  // Drafts live as long as the window, so one test's edit would otherwise open the next one.
  beforeEach(() => {
    resetDrafts();
  });

  it('keeps the success toast after the save reloads the prompts', async () => {
    mockPrompts(prompt('post_process', 'original'));
    const { target } = render(PromptsEditor, { name: 'post_process', onback: vi.fn() });
    await settle();

    const editor = target.querySelector('textarea')!;
    editor.value = 'edited';
    editor.dispatchEvent(new Event('input', { bubbles: true }));
    await settle();

    button(target, 'Save').click();
    await settle();

    expect(mocks.invoke).toHaveBeenCalledWith('set_prompt', {
      params: { name: 'post_process', content: 'edited' },
    });
    // The prompts are read again after the save; that must not wipe the toast.
    expect(target.querySelector('.toast-success')?.textContent).toContain('Saved');
  });

  it('keeps unsaved edits after leaving the editor and coming back', async () => {
    mockPrompts(prompt('post_process', 'a'), prompt('shorten', 'b'));
    const { target, instance } = render(PromptsEditor, { name: 'post_process', onback: vi.fn() });
    await settle();

    const editor = target.querySelector('textarea')!;
    editor.value = 'work in progress';
    editor.dispatchEvent(new Event('input', { bubbles: true }));
    await settle();

    // This file keeps its own list of mounted components, so it unmounts through that list.
    await unmount(instance);
    mounted = [];

    const again = render(PromptsEditor, { name: 'post_process', onback: vi.fn() });
    await settle();
    expect(again.target.querySelector('textarea')!.value).toBe('work in progress');
  });

  it('drops the draft once the prompt is restored to its default', async () => {
    // Restore Default is only offered for a prompt that currently has an override.
    mocks.invoke.mockImplementation(async command => command === 'get_prompts'
      ? [{ ...prompt('post_process', 'a'), is_override: true, default_content: 'a' }]
      : undefined);
    const { target } = render(PromptsEditor, { name: 'post_process', onback: vi.fn() });
    await settle();

    const editor = () => target.querySelector('textarea')!;
    editor().value = 'edited';
    editor().dispatchEvent(new Event('input', { bubbles: true }));
    await settle();

    button(target, 'Restore Default').click();
    await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('reset_prompt', {
      params: { name: 'post_process' },
    });
    // A surviving draft would shadow the content that just came back from disk.
    expect(editor().value).toBe('a');
  });

  it('starts each opened prompt without the previous toast', async () => {
    const prompts = ['post_process', 'shorten', 'change_tone', 'generate_reply', 'translate']
      .map(name => prompt(name, `content of ${name}`));
    mocks.invoke.mockImplementation(async command => {
      switch (command) {
        case 'get_llm_processors': return [];
        case 'get_config': return { llm_processor: 'gemini', llm_model: null, http_llm_config: null };
        case 'get_prompts': return prompts;
        default: return undefined;
      }
    });
    const { target } = render(LlmConfig, {});
    await settle();

    button(target, 'Shorten').click();
    await settle();
    const editor = target.querySelector('textarea')!;
    editor.value = 'edited';
    editor.dispatchEvent(new Event('input', { bubbles: true }));
    await settle();
    button(target, 'Save').click();
    await settle();
    expect(target.querySelector('.toast-success')).not.toBeNull();

    target.querySelector<HTMLElement>('[aria-label="Back to AI Processing"]')!.click();
    await settle();
    button(target, 'Reply').click();
    await settle();
    expect(target.querySelector('.toast-success')).toBeNull();
  });
});

describe('elevenlabs onboarding', () => {
  it('loads the language list when the key is entered from the modal', async () => {
    const providers = (configured: boolean) => [
      { id: 'elevenlabs', name: 'ElevenLabs Scribe', configured, provider_type: 'streaming', requires_api_key: true, model_status: null },
      { id: 'openai', name: 'OpenAI Whisper', configured: true, provider_type: 'batch', requires_api_key: true, model_status: null },
    ];
    let keySaved = false;
    mocks.invoke.mockImplementation(async (command: string) => {
      switch (command) {
        case 'get_stt_providers': return providers(keySaved);
        // Onboarding starts on another provider, so initialize() does not
        // preload the ElevenLabs languages.
        case 'get_config': return { stt_provider: 'openai', elevenlabs_language: 'auto' };
        case 'get_elevenlabs_languages': return [['auto', 'Auto'], ['eng', 'English']];
        case 'save_api_key': keySaved = true; return undefined;
        default: return undefined;
      }
    });

    const { target } = render(ProviderConfig, {});
    await settle();
    expect(mocks.invoke).not.toHaveBeenCalledWith('get_elevenlabs_languages');

    button(target, 'ElevenLabs Scribe').click();
    await settle();
    const input = target.querySelector('input[type="password"]') as HTMLInputElement;
    input.value = 'key';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await settle();
    button(target, 'Save').click();
    await settle();

    expect(mocks.invoke).toHaveBeenCalledWith('get_elevenlabs_languages');
    expect(target.querySelector('.locale-selector, select')).not.toBeNull();
  });
});

describe('diagnostics export', () => {
  it('copies the log in the order the panel displays it', async () => {
    mocks.invoke.mockImplementation(async (command: string) => command === 'get_diagnostic_logs'
      ? [
          { timestamp_ms: 1_000, level: 'warn', target: 'lt_stt', message: 'oldest' },
          { timestamp_ms: 2_000, level: 'error', target: 'lt_pipeline', message: 'newest' },
        ]
      : undefined);

    const { target } = render(DiagnosticsPanel, { onback: vi.fn() });
    await settle();

    const rows = [...target.querySelectorAll('.log-row')].map(row => row.textContent ?? '');
    expect(rows[0]).toContain('newest');

    button(target, 'Copy').click();
    await settle();

    const copied = mocks.writeText.mock.calls[0][0] as string;
    expect(copied.indexOf('newest')).toBeLessThan(copied.indexOf('oldest'));
  });
});
