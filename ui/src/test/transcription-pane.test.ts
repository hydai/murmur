import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync } from 'svelte';
import userEvent from '@testing-library/user-event';
import ProviderConfig from '../components/settings/ProviderConfig.svelte';
import { button, render, settle, unmountAll } from './helpers';

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock('../lib/tauri', () => ({ safeInvoke: mocks.invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }));

/** What the backend lists, one entry per provider. */
const P = {
  apple: (model_status = 'installed') => ({
    id: 'apple_stt', name: 'Apple Speech', configured: model_status === 'installed',
    provider_type: 'local', requires_api_key: false, model_status,
  }),
  elevenlabs: (configured: boolean) => ({
    id: 'elevenlabs', name: 'ElevenLabs Scribe', configured,
    provider_type: 'streaming', requires_api_key: true, model_status: null,
  }),
  openai: (configured = true) => ({
    id: 'openai', name: 'OpenAI Whisper', configured,
    provider_type: 'batch', requires_api_key: true, model_status: null,
  }),
  groq: (configured = false) => ({
    id: 'groq', name: 'Groq Whisper Turbo', configured,
    provider_type: 'batch', requires_api_key: true, model_status: null,
  }),
  custom: (configured = false, name = 'Custom Endpoint') => ({
    id: 'custom_stt', name, configured,
    provider_type: 'batch', requires_api_key: false, model_status: null,
  }),
};

type Listed = ReturnType<(typeof P)[keyof typeof P]>;

const CONFIG = {
  stt_provider: 'openai',
  apple_stt_locale: 'auto',
  elevenlabs_language: 'auto',
  http_stt_config: null as null | Record<string, string | null>,
};

const LANGUAGES = [['auto', 'Auto-detect'], ['eng', 'English'], ['jpn', 'Japanese']];

let listeners: Map<string, (event: { payload: unknown }) => void>;

/**
 * A backend that remembers what it is told, the way the real one does: a saved
 * key marks its provider configured, a switch moves the provider in use, and a
 * saved endpoint becomes the custom provider. Every command succeeds.
 */
function backend(init: { providers?: Listed[]; config?: Partial<typeof CONFIG> } = {}) {
  const state = {
    providers: init.providers ?? [P.apple(), P.openai()],
    config: { ...CONFIG, ...init.config },
  };
  mocks.invoke.mockImplementation(async (command: string, args?: any) => {
    switch (command) {
      case 'get_stt_providers': return state.providers;
      case 'get_config': return state.config;
      case 'get_apple_stt_locales': return ['en_US', 'ja_JP'];
      case 'get_elevenlabs_languages': return LANGUAGES;
      case 'save_api_key':
        state.providers = state.providers.map(p => (p.id === args.provider ? { ...p, configured: true } : p));
        return undefined;
      case 'set_stt_provider':
        state.config = { ...state.config, stt_provider: args.provider };
        return undefined;
      case 'set_custom_stt_endpoint':
        state.config = {
          ...state.config,
          http_stt_config: {
            custom_base_url: args.baseUrl,
            custom_display_name: args.displayName,
            custom_model: args.model,
            language: args.language,
          },
        };
        state.providers = state.providers.map(p => (p.id === 'custom_stt'
          ? { ...p, configured: true, name: args.displayName ?? 'Custom Endpoint' }
          : p));
        return undefined;
      default: return undefined;
    }
  });
  return state;
}

/** Like `backend`, except `command` rejects with `error`. The page logs that, so the log is silenced. */
function failing(command: string, error: Error, init: Parameters<typeof backend>[0] = {}) {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  backend(init);
  const working = mocks.invoke.getMockImplementation()!;
  mocks.invoke.mockImplementation(async (name: string, args?: unknown) => {
    if (name === command) throw error;
    return working(name, args);
  });
}

beforeEach(() => {
  listeners = new Map();
  mocks.listen.mockReset().mockImplementation(async (name: string, callback: (event: { payload: unknown }) => void) => {
    listeners.set(name, callback);
    return () => listeners.delete(name);
  });
  backend();
});

afterEach(async () => {
  await unmountAll();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

async function open() {
  const { target } = render(ProviderConfig, {});
  await settle();
  return target;
}

function emit(name: string, payload: unknown) {
  const listener = listeners.get(name);
  expect(listener, `${name} listener`).toBeDefined();
  listener!({ payload });
  flushSync();
}

const inUse = (target: Element) => [...target.querySelectorAll('[aria-current="true"]')];
const errorToast = (target: Element) => target.querySelector('.toast-error')?.textContent;
const successToast = (target: Element) => target.querySelector('.toast-success')?.textContent;
const dialog = (target: Element) => target.querySelector<HTMLElement>('[role="dialog"]');
const dialogTitle = (target: Element) => dialog(target)?.querySelector('h2')?.textContent;
const field = (target: Element, id: string) => dialog(target)!.querySelector<HTMLInputElement>(`#${id}`)!;
/** The one field of the API key sheet, which ApiKeySheet gives an id of its own. */
const keyField = (target: Element) => dialog(target)!.querySelector<HTMLInputElement>('input')!;
const select = (target: Element, label: string) =>
  target.querySelector<HTMLSelectElement>(`select[aria-label="${label}"]`)!;
const called = (command: string) => mocks.invoke.mock.calls.filter(([name]) => name === command);

function group(target: Element, title: string): HTMLElement {
  const found = [...target.querySelectorAll<HTMLElement>('.group')]
    .find(candidate => candidate.querySelector('.group-title')?.textContent === title);
  expect(found, `group ${title}`).toBeDefined();
  return found!;
}

/** A row of the Service group, found by its name. */
function service(target: Element, name: string): HTMLElement {
  const found = [...group(target, 'Service').querySelectorAll<HTMLElement>('.row')]
    .find(row => row.querySelector('.row-label')?.textContent === name);
  expect(found, `service ${name}`).toBeDefined();
  return found!;
}

/** Every row of a group as [label, detail, trailing buttons]. */
const rowsOf = (container: Element) => [...container.querySelectorAll('.row')].map(row => [
  row.querySelector('.row-label')?.textContent,
  row.querySelector('.row-detail')?.textContent ?? null,
  [...row.querySelectorAll('.row-trailing button')].map(b => b.textContent?.trim()),
]);

function fill(input: HTMLInputElement, value: string) {
  input.value = value;
  input.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
}

async function choose(element: HTMLSelectElement, value: string) {
  element.value = value;
  element.dispatchEvent(new Event('change', { bubbles: true }));
  await settle();
}

/** `command` answers only when the returned `release` is called; everything else is as `backend` made it. */
function holding(command: string) {
  let release!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  const working = mocks.invoke.getMockImplementation()!;
  mocks.invoke.mockImplementation(async (name: string, args?: unknown) => {
    if (name === command) return held;
    return working(name, args);
  });
  return release;
}

describe('Transcription pane: services', () => {
  it('lists the services local first, then cloud, then custom, with the copy the spec gives', async () => {
    backend({
      providers: [P.elevenlabs(false), P.openai(true), P.groq(false), P.apple('not_installed'), P.custom(false)],
      config: { stt_provider: 'openai' },
    });
    const target = await open();
    expect(target.querySelector('h1')?.textContent).toBe('Transcription');
    expect([...target.querySelectorAll('.group-title')].map(title => title.textContent))
      .toEqual(['Service', 'OpenAI Whisper']);
    expect(rowsOf(group(target, 'Service'))).toEqual([
      ['Apple Speech', 'Speech model not downloaded', ['Download']],
      ['ElevenLabs Scribe', 'Cloud · live', ['Add API Key…']],
      ['OpenAI Whisper', 'Cloud · key saved', []],
      ['Groq Whisper Turbo', 'Cloud', ['Add API Key…']],
      ['Custom Endpoint', 'Any Whisper-compatible server', ['Set Up…']],
    ]);
  });

  it('shows the key and the live capability together for a provider that has both', async () => {
    backend({ providers: [P.elevenlabs(true), P.openai(false)], config: { stt_provider: 'elevenlabs' } });
    const target = await open();
    expect(service(target, 'ElevenLabs Scribe').querySelector('.row-detail')?.textContent)
      .toBe('Cloud · live · key saved');
  });

  it('gives each service the icon of its kind', async () => {
    backend({ providers: [P.apple(), P.elevenlabs(true), P.openai(), P.custom(true, 'Local Whisper')] });
    const target = await open();
    const icon = (name: string) => service(target, name).querySelector('.row-icon svg')?.getAttribute('class');
    expect(icon('Apple Speech')).toContain('lucide-laptop');
    expect(icon('ElevenLabs Scribe')).toContain('lucide-cloud');
    expect(icon('OpenAI Whisper')).toContain('lucide-cloud');
    expect(icon('Local Whisper')).toContain('lucide-server');
  });

  it('treats a provider listed without its type fields as cloud, and settings without the optional parts as defaults', async () => {
    mocks.invoke.mockImplementation(async (command: string) => command === 'get_stt_providers'
      ? [{ id: 'openai', name: 'OpenAI Whisper', description: 'Cloud transcription', configured: true }]
      : { stt_provider: 'openai' });
    const target = await open();
    expect(rowsOf(group(target, 'Service'))).toEqual([['OpenAI Whisper', 'Cloud · key saved', []]]);
    expect(rowsOf(group(target, 'OpenAI Whisper'))).toEqual([['API key', 'Saved', ['Change…']]]);
    expect(inUse(target)).toHaveLength(1);
    // A load that fell over would say so, so its silence is part of the point.
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('shows a configured custom endpoint by its address', async () => {
    backend({
      providers: [P.openai(), P.custom(true, 'Local Whisper')],
      config: {
        http_stt_config: { custom_base_url: 'http://localhost:8080/v1', custom_display_name: 'Local Whisper', custom_model: null, language: null },
      },
    });
    const target = await open();
    expect(rowsOf(group(target, 'Service'))[1]).toEqual(['Local Whisper', 'http://localhost:8080/v1', []]);
  });

  it('marks the provider in use and switches on click without a success toast', async () => {
    backend({ providers: [P.apple(), P.openai()], config: { stt_provider: 'apple_stt' } });
    const target = await open();
    expect(inUse(target)).toHaveLength(1);
    expect(inUse(target)[0].textContent).toContain('Apple Speech');
    button(target, 'OpenAI Whisper').click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('set_stt_provider', { provider: 'openai' });
    expect(target.querySelector('[aria-current="true"]')?.textContent).toContain('OpenAI Whisper');
    expect(inUse(target)).toHaveLength(1);
    expect(target.querySelector('.toast-success')).toBeNull();
  });

  it('keeps the provider in use when switching fails', async () => {
    failing('set_stt_provider', new Error('boom'), {
      providers: [P.apple(), P.openai()], config: { stt_provider: 'apple_stt' },
    });
    const target = await open();
    button(target, 'OpenAI Whisper').click(); await settle();
    expect(target.querySelector('[aria-current="true"]')?.textContent).toContain('Apple Speech');
    expect(inUse(target)).toHaveLength(1);
    expect(errorToast(target)).toContain('Failed to switch provider');
  });

  it('loads what a provider needs when it is switched to', async () => {
    backend({ providers: [P.apple(), P.elevenlabs(true), P.openai()], config: { stt_provider: 'openai' } });
    const target = await open();
    expect(called('get_apple_stt_locales')).toHaveLength(0);
    expect(called('get_elevenlabs_languages')).toHaveLength(0);

    button(target, 'Apple Speech').click(); await settle();
    expect(called('get_apple_stt_locales')).toHaveLength(1);
    expect(select(target, 'Language')).not.toBeNull();

    button(target, 'ElevenLabs Scribe').click(); await settle();
    expect(called('get_elevenlabs_languages')).toHaveLength(1);
    expect([...select(target, 'Language').options].map(o => o.value)).toEqual(['auto', 'eng', 'jpn']);
  });

  it('disables a provider that is unavailable on this Mac', async () => {
    backend({ providers: [P.apple('unavailable'), P.openai()], config: { stt_provider: 'openai' } });
    const target = await open();
    const row = service(target, 'Apple Speech');
    expect(row.querySelector<HTMLButtonElement>('.row-main')?.disabled).toBe(true);
    expect(row.querySelector('.row-detail')?.textContent).toBe('Requires macOS 26');
    row.querySelector<HTMLButtonElement>('.row-main')!.click(); await settle();
    expect(called('set_stt_provider')).toHaveLength(0);
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('asks for the speech model first when it is not installed', async () => {
    backend({ providers: [P.apple('not_installed'), P.openai()], config: { stt_provider: 'openai' } });
    const target = await open();
    button(service(target, 'Apple Speech'), 'Apple Speech').click(); await settle();
    expect(errorToast(target)).toContain('Download the speech model first.');
    expect(called('set_stt_provider')).toHaveLength(0);
    expect(inUse(target)[0].textContent).toContain('OpenAI Whisper');
  });

  it('says why Apple Speech cannot be used when it is chosen without macOS 26', async () => {
    // The row is disabled, so this only guards the handler against a click that gets through.
    backend({ providers: [P.apple('unavailable'), P.openai()], config: { stt_provider: 'openai' } });
    const target = await open();
    const main = service(target, 'Apple Speech').querySelector<HTMLButtonElement>('.row-main')!;
    main.disabled = false;
    main.click(); await settle();
    expect(errorToast(target)).toContain('Apple Speech requires macOS 26 or later.');
    expect(called('set_stt_provider')).toHaveLength(0);
  });

  it('says so when the providers or the settings cannot be loaded', async () => {
    failing('get_stt_providers', new Error('unavailable'));
    let target = await open();
    expect(errorToast(target)).toContain('Failed to load providers');
    expect(target.querySelector('.group')).toBeNull();
    await unmountAll();

    failing('get_config', new Error('unavailable'));
    target = await open();
    expect(errorToast(target)).toContain('Failed to load config');
  });
});

describe('Transcription pane: the provider in use', () => {
  it('shows the language and the speech model for Apple Speech', async () => {
    backend({ providers: [P.apple(), P.openai()], config: { stt_provider: 'apple_stt', apple_stt_locale: 'ja_JP' } });
    const target = await open();
    const settings = group(target, 'Apple Speech');
    expect(rowsOf(settings).map(([label]) => label)).toEqual(['Language', 'Speech model']);
    expect([...select(target, 'Language').options].map(o => [o.value, o.textContent]))
      .toEqual([['auto', 'Automatic'], ['en_US', 'en_US'], ['ja_JP', 'ja_JP']]);
    expect(select(target, 'Language').value).toBe('ja_JP');
    expect(settings.textContent).toContain('Installed');
    expect(settings.querySelector('.row-trailing button')).toBeNull();
  });

  it('offers the download from the speech model row when the model is missing', async () => {
    backend({ providers: [P.apple('not_installed'), P.openai()], config: { stt_provider: 'apple_stt' } });
    const target = await open();
    const model = [...group(target, 'Apple Speech').querySelectorAll<HTMLElement>('.row')]
      .find(row => row.querySelector('.row-label')?.textContent === 'Speech model')!;
    expect(model.textContent).not.toContain('Installed');
    button(model, 'Download').click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('download_apple_stt_model', { locale: 'auto' });
  });

  it('shows the saved key and the language for ElevenLabs', async () => {
    backend({ providers: [P.elevenlabs(true), P.openai()], config: { stt_provider: 'elevenlabs', elevenlabs_language: 'jpn' } });
    const target = await open();
    expect(rowsOf(group(target, 'ElevenLabs Scribe'))).toEqual([
      ['API key', 'Saved', ['Change…']],
      ['Language', null, []],
    ]);
    expect(select(target, 'Language').value).toBe('jpn');
  });

  it('calls the ElevenLabs automatic choice Automatic, like the Apple Speech menu, and keeps the backend names for the rest', async () => {
    backend({ providers: [P.elevenlabs(true), P.openai()], config: { stt_provider: 'elevenlabs' } });
    const target = await open();
    // The backend names it "Auto-detect"; the other language menu on this page says "Automatic".
    expect([...select(target, 'Language').options].map(o => [o.value, o.textContent]))
      .toEqual([['auto', 'Automatic'], ['eng', 'English'], ['jpn', 'Japanese']]);
  });

  it.each([['OpenAI Whisper', P.openai(true)], ['Groq Whisper Turbo', P.groq(true)]])(
    'shows the saved key for %s',
    async (name, listed) => {
      backend({ providers: [P.apple(), listed], config: { stt_provider: listed.id } });
      const target = await open();
      expect(rowsOf(group(target, name))).toEqual([['API key', 'Saved', ['Change…']]]);
      expect(target.querySelector('select')).toBeNull();
    },
  );

  it('shows the address of the custom endpoint in use', async () => {
    backend({
      providers: [P.openai(), P.custom(true, 'Local Whisper')],
      config: {
        stt_provider: 'custom_stt',
        http_stt_config: { custom_base_url: 'http://localhost:8080/v1', custom_display_name: 'Local Whisper', custom_model: null, language: null },
      },
    });
    const target = await open();
    expect(rowsOf(group(target, 'Local Whisper'))).toEqual([['Endpoint', 'http://localhost:8080/v1', ['Edit…']]]);
  });

  it('sets the Apple Speech language and picks up the model status of the new language', async () => {
    backend({ providers: [P.apple(), P.openai()], config: { stt_provider: 'apple_stt' } });
    const target = await open();
    const before = called('get_stt_providers').length;
    await choose(select(target, 'Language'), 'ja_JP');
    expect(mocks.invoke).toHaveBeenCalledWith('set_apple_stt_locale', { locale: 'ja_JP' });
    expect(called('get_stt_providers').length).toBe(before + 1);
    expect(select(target, 'Language').value).toBe('ja_JP');
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('keeps the Apple Speech language when the backend rejects the change', async () => {
    failing('set_apple_stt_locale', new Error('denied'), {
      providers: [P.apple(), P.openai()], config: { stt_provider: 'apple_stt' },
    });
    const target = await open();
    const language = select(target, 'Language');
    await choose(language, 'ja_JP');
    expect(language.value).toBe('auto');
    expect(errorToast(target)).toContain('Failed to set locale');
  });

  it('sets the ElevenLabs language without a success toast', async () => {
    backend({ providers: [P.elevenlabs(true), P.openai()], config: { stt_provider: 'elevenlabs' } });
    const target = await open();
    await choose(select(target, 'Language'), 'jpn');
    expect(mocks.invoke).toHaveBeenCalledWith('set_elevenlabs_language', { language: 'jpn' });
    expect(select(target, 'Language').value).toBe('jpn');
    expect(target.querySelector('.toast')).toBeNull();
  });

  it.each([
    ['Apple Speech', 'apple_stt', 'set_apple_stt_locale', 'ja_JP'],
    ['ElevenLabs Scribe', 'elevenlabs', 'set_elevenlabs_language', 'jpn'],
  ])('keeps the %s language on screen while the backend is still deciding', async (_name, id, command, pick) => {
    backend({ providers: [P.apple(), P.elevenlabs(true), P.openai()], config: { stt_provider: id } });
    const release = holding(command);
    const target = await open();
    const language = select(target, 'Language');
    await choose(language, pick);
    // A handler that does not hand its promise back would have the pick snap back to the old language here.
    expect(language.value).toBe(pick);
    release();
    await settle();
    expect(language.value).toBe(pick);
  });

  it('keeps the ElevenLabs language when the backend rejects the change', async () => {
    failing('set_elevenlabs_language', new Error('denied'), {
      providers: [P.elevenlabs(true), P.openai()], config: { stt_provider: 'elevenlabs' },
    });
    const target = await open();
    const language = select(target, 'Language');
    await choose(language, 'jpn');
    expect(language.value).toBe('auto');
    expect(errorToast(target)).toContain('Failed to set language');
  });
});

describe('Transcription pane: API key sheet', () => {
  it('opens titled for a provider without a key, from its button or its row', async () => {
    backend({ providers: [P.elevenlabs(false), P.openai(true)], config: { stt_provider: 'openai' } });
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    expect(dialogTitle(target)).toBe('Add API Key for ElevenLabs Scribe');
    expect(called('set_stt_provider')).toHaveLength(0);
    await userEvent.setup().keyboard('{Escape}');
    expect(dialog(target)).toBeNull();

    button(target, 'ElevenLabs Scribe').click(); await settle();
    expect(dialogTitle(target)).toBe('Add API Key for ElevenLabs Scribe');
  });

  it('renders the sheet inside the pane', async () => {
    backend({ providers: [P.elevenlabs(false), P.openai(true)] });
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    expect(target.querySelector('.pane [role="dialog"]')).not.toBeNull();
  });

  it('opens titled as a change for a key that is already saved', async () => {
    backend({ providers: [P.apple(), P.openai(true)], config: { stt_provider: 'openai' } });
    const target = await open();
    button(group(target, 'OpenAI Whisper'), 'Change…').click(); await settle();
    expect(dialogTitle(target)).toBe('Change API Key for OpenAI Whisper');
    expect(button(dialog(target)!, 'Save').textContent?.trim()).toBe('Save');
  });

  it('saves a new key, switches to the provider, and confirms with a toast', async () => {
    backend({ providers: [P.elevenlabs(false), P.openai(true)], config: { stt_provider: 'openai' } });
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    expect(button(dialog(target)!, 'Save & Use')).toBeDefined();
    fill(keyField(target), 'sk-test');
    button(dialog(target)!, 'Save & Use').click(); await settle();

    expect(mocks.invoke).toHaveBeenCalledWith('save_api_key', { provider: 'elevenlabs', apiKey: 'sk-test' });
    expect(mocks.invoke).toHaveBeenCalledWith('set_stt_provider', { provider: 'elevenlabs' });
    const order = mocks.invoke.mock.calls.map(([name]) => name);
    expect(order.indexOf('save_api_key')).toBeLessThan(order.indexOf('set_stt_provider'));
    expect(dialog(target)).toBeNull();
    expect(successToast(target)).toContain('API key saved');
    expect(inUse(target)[0].textContent).toContain('ElevenLabs Scribe');
    // The language list of a newly chosen ElevenLabs is there without reopening the page.
    expect(select(target, 'Language')).not.toBeNull();
    expect(rowsOf(group(target, 'ElevenLabs Scribe'))[0]).toEqual(['API key', 'Saved', ['Change…']]);
  });

  it('saves a changed key as a key change, with a toast', async () => {
    backend({ providers: [P.apple(), P.openai(true)], config: { stt_provider: 'openai' } });
    const target = await open();
    button(group(target, 'OpenAI Whisper'), 'Change…').click(); await settle();
    fill(keyField(target), 'sk-new');
    button(dialog(target)!, 'Save').click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('save_api_key', { provider: 'openai', apiKey: 'sk-new' });
    expect(dialog(target)).toBeNull();
    expect(successToast(target)).toContain('API key saved');
  });

  it('says Saving… on the button while the key is being saved, then closes', async () => {
    backend({ providers: [P.elevenlabs(false), P.openai(true)], config: { stt_provider: 'openai' } });
    const release = holding('save_api_key');
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    fill(keyField(target), 'sk-test');
    button(dialog(target)!, 'Save & Use').click(); await settle();
    const save = dialog(target)!.querySelector<HTMLButtonElement>('button[type="submit"]')!;
    expect(save.textContent?.trim()).toBe('Saving…');
    expect(save.disabled).toBe(true);
    release(); await settle();
    expect(dialog(target)).toBeNull();
  });

  it('lets the confirmation go away by itself after two seconds', async () => {
    vi.useFakeTimers();
    backend({ providers: [P.elevenlabs(false), P.openai(true)], config: { stt_provider: 'openai' } });
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    fill(keyField(target), 'sk-test');
    button(dialog(target)!, 'Save & Use').click(); await settle();
    expect(successToast(target)).toContain('API key saved');
    vi.advanceTimersByTime(1900); await settle();
    expect(successToast(target)).toContain('API key saved');
    vi.advanceTimersByTime(200); await settle();
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('submits with Enter in the key field', async () => {
    backend({ providers: [P.elevenlabs(false), P.openai(true)] });
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    // The sheet focuses its first field when it opens.
    expect(document.activeElement).toBe(keyField(target));
    const user = userEvent.setup();
    await user.keyboard('sk-typed{Enter}');
    await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('save_api_key', { provider: 'elevenlabs', apiKey: 'sk-typed' });
  });

  it('explains an empty key inside the sheet and saves nothing', async () => {
    backend({ providers: [P.elevenlabs(false), P.openai(true)] });
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    button(dialog(target)!, 'Save & Use').click(); await settle();
    expect(dialog(target)?.querySelector('.sheet-error')?.textContent).toBe('API key cannot be empty');
    expect(called('save_api_key')).toHaveLength(0);
    expect(target.querySelector('.toast')).toBeNull();
    // It is about what was typed, so typing something clears it.
    fill(keyField(target), 'k');
    expect(dialog(target)?.querySelector('.sheet-error')).toBeNull();
  });

  it('keeps the sheet open and says why when the key cannot be saved', async () => {
    failing('save_api_key', new Error('keychain locked'), {
      providers: [P.elevenlabs(false), P.openai(true)], config: { stt_provider: 'openai' },
    });
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    fill(keyField(target), 'sk-test');
    button(dialog(target)!, 'Save & Use').click(); await settle();
    expect(errorToast(target)).toContain('Failed to save API key');
    expect(errorToast(target)).toContain('keychain locked');
    expect(dialog(target)).not.toBeNull();
    expect(called('set_stt_provider')).toHaveLength(0);
    expect(inUse(target)[0].textContent).toContain('OpenAI Whisper');
  });

  it('takes the failure with it when the sheet it came from is cancelled', async () => {
    failing('save_api_key', new Error('keychain locked'), {
      providers: [P.elevenlabs(false), P.openai(true)], config: { stt_provider: 'openai' },
    });
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    fill(keyField(target), 'sk-test');
    button(dialog(target)!, 'Save & Use').click(); await settle();
    expect(errorToast(target)).toContain('keychain locked');
    button(dialog(target)!, 'Cancel').click(); await settle();
    expect(dialog(target)).toBeNull();
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('closes on Cancel and Escape without saving, and forgets what was typed', async () => {
    backend({ providers: [P.elevenlabs(false), P.openai(true)] });
    const target = await open();
    const user = userEvent.setup();
    button(target, 'Add API Key…').click(); await settle();
    fill(keyField(target), 'sk-secret');
    button(dialog(target)!, 'Cancel').click(); await settle();
    expect(dialog(target)).toBeNull();

    button(target, 'Add API Key…').click(); await settle();
    expect(keyField(target).value).toBe('');
    fill(keyField(target), 'sk-secret');
    await user.keyboard('{Escape}'); await settle();
    expect(dialog(target)).toBeNull();

    button(target, 'Add API Key…').click(); await settle();
    expect(keyField(target).value).toBe('');
    expect(called('save_api_key')).toHaveLength(0);
  });

  it('shows the key as hidden again when the sheet is reopened', async () => {
    backend({ providers: [P.elevenlabs(false), P.openai(true)] });
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    dialog(target)!.querySelector<HTMLButtonElement>('button[aria-label="Show API key"]')!.click(); await settle();
    expect(keyField(target).type).toBe('text');
    button(dialog(target)!, 'Cancel').click(); await settle();
    button(target, 'Add API Key…').click(); await settle();
    expect(keyField(target).type).toBe('password');
  });
});

describe('Transcription pane: custom endpoint sheet', () => {
  it('opens from the row or the Set Up button of an endpoint that is not set up', async () => {
    backend({ providers: [P.openai(), P.custom(false)], config: { stt_provider: 'openai' } });
    const target = await open();
    button(target, 'Set Up…').click(); await settle();
    expect(dialogTitle(target)).toBe('Custom Endpoint');
    button(dialog(target)!, 'Cancel').click(); await settle();
    expect(dialog(target)).toBeNull();

    button(target, 'Custom Endpoint').click(); await settle();
    expect(dialogTitle(target)).toBe('Custom Endpoint');
    expect(called('set_stt_provider')).toHaveLength(0);
  });

  it('lays out its fields with the labels, placeholders and hint the spec gives', async () => {
    backend({ providers: [P.openai(), P.custom(false)] });
    const target = await open();
    button(target, 'Set Up…').click(); await settle();
    const sheet = dialog(target)!;
    expect([...sheet.querySelectorAll('label')].map(label => label.textContent))
      .toEqual(['Base URL', 'API Key', 'Model', 'Language', 'Display Name']);
    expect([...sheet.querySelectorAll('input')].map(input => [input.id, input.type, input.placeholder])).toEqual([
      ['custom-stt-base-url', 'text', 'http://localhost:8080/v1'],
      ['custom-stt-api-key', 'password', 'Only if the server needs one'],
      ['custom-stt-model', 'text', 'whisper-1'],
      ['custom-stt-language', 'text', 'Automatic'],
      ['custom-stt-display-name', 'text', 'Local Whisper'],
    ]);
    // Every label points at its own field.
    for (const label of sheet.querySelectorAll('label')) {
      expect(sheet.querySelector(`#${label.htmlFor}`)).not.toBeNull();
    }
    const hint = sheet.querySelector('#custom-stt-language-hint');
    expect(hint?.textContent).toBe('ISO-639-1 code, e.g. en');
    expect(field(target, 'custom-stt-language').getAttribute('aria-describedby')).toBe('custom-stt-language-hint');
    expect(document.activeElement).toBe(field(target, 'custom-stt-base-url'));
  });

  it('cannot be submitted without a Base URL', async () => {
    backend({ providers: [P.openai(), P.custom(false)] });
    const target = await open();
    button(target, 'Set Up…').click(); await settle();
    const submit = button(dialog(target)!, 'Save & Use');
    expect(submit.type).toBe('submit');
    expect(submit.disabled).toBe(true);
    fill(field(target, 'custom-stt-base-url'), '   ');
    expect(submit.disabled).toBe(true);
    fill(field(target, 'custom-stt-base-url'), 'http://localhost:8080/v1');
    expect(submit.disabled).toBe(false);
  });

  it('saves the endpoint, uses it, and confirms with a toast', async () => {
    backend({ providers: [P.openai(), P.custom(false)], config: { stt_provider: 'openai' } });
    const target = await open();
    button(target, 'Set Up…').click(); await settle();
    fill(field(target, 'custom-stt-base-url'), ' http://localhost:8080/v1 ');
    fill(field(target, 'custom-stt-api-key'), 'secret');
    fill(field(target, 'custom-stt-language'), 'en');
    fill(field(target, 'custom-stt-display-name'), 'Local Whisper');
    button(dialog(target)!, 'Save & Use').click(); await settle();

    expect(mocks.invoke).toHaveBeenCalledWith('set_custom_stt_endpoint', {
      baseUrl: 'http://localhost:8080/v1', displayName: 'Local Whisper', model: null, language: 'en',
    });
    expect(mocks.invoke).toHaveBeenCalledWith('save_api_key', { provider: 'custom_stt', apiKey: 'secret' });
    expect(mocks.invoke).toHaveBeenCalledWith('set_stt_provider', { provider: 'custom_stt' });
    const order = mocks.invoke.mock.calls.map(([name]) => name);
    expect(order.indexOf('set_custom_stt_endpoint')).toBeLessThan(order.indexOf('save_api_key'));
    expect(order.indexOf('save_api_key')).toBeLessThan(order.indexOf('set_stt_provider'));
    expect(dialog(target)).toBeNull();
    expect(successToast(target)).toContain('Custom endpoint saved');
    expect(inUse(target)[0].textContent).toContain('Local Whisper');
    expect(rowsOf(group(target, 'Local Whisper'))).toEqual([['Endpoint', 'http://localhost:8080/v1', ['Edit…']]]);
  });

  it('leaves the stored key alone when none is typed', async () => {
    backend({ providers: [P.openai(), P.custom(false)] });
    const target = await open();
    button(target, 'Set Up…').click(); await settle();
    fill(field(target, 'custom-stt-base-url'), 'http://localhost:8080/v1');
    button(dialog(target)!, 'Save & Use').click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('set_custom_stt_endpoint', {
      baseUrl: 'http://localhost:8080/v1', displayName: null, model: null, language: null,
    });
    expect(called('save_api_key')).toHaveLength(0);
  });

  it('keeps the sheet open and says why when the endpoint cannot be saved', async () => {
    failing('set_custom_stt_endpoint', new Error('disk full'), {
      providers: [P.openai(), P.custom(false)], config: { stt_provider: 'openai' },
    });
    const target = await open();
    button(target, 'Set Up…').click(); await settle();
    fill(field(target, 'custom-stt-base-url'), 'http://localhost:8080/v1');
    button(dialog(target)!, 'Save & Use').click(); await settle();
    expect(errorToast(target)).toContain('Failed to save custom STT endpoint');
    expect(dialog(target)).not.toBeNull();
    expect(called('set_stt_provider')).toHaveLength(0);
    expect(inUse(target)[0].textContent).toContain('OpenAI Whisper');
  });

  it('edits a saved endpoint from the in-use group, starting from what is saved', async () => {
    backend({
      providers: [P.openai(), P.custom(true, 'Local Whisper')],
      config: {
        stt_provider: 'custom_stt',
        http_stt_config: { custom_base_url: 'http://a.test/v1', custom_display_name: 'Local Whisper', custom_model: 'large-v3', language: 'en' },
      },
    });
    const target = await open();
    button(group(target, 'Local Whisper'), 'Edit…').click(); await settle();
    expect(dialogTitle(target)).toBe('Custom Endpoint');
    expect(field(target, 'custom-stt-base-url').value).toBe('http://a.test/v1');
    expect(field(target, 'custom-stt-model').value).toBe('large-v3');
    expect(field(target, 'custom-stt-language').value).toBe('en');
    expect(field(target, 'custom-stt-display-name').value).toBe('Local Whisper');
    // The stored key is never read back.
    expect(field(target, 'custom-stt-api-key').value).toBe('');
  });

  it('shows nothing of a half-typed address on the page, and forgets it on Cancel', async () => {
    backend({
      providers: [P.openai(), P.custom(true, 'Local Whisper')],
      config: {
        stt_provider: 'custom_stt',
        http_stt_config: { custom_base_url: 'http://a.test/v1', custom_display_name: 'Local Whisper', custom_model: null, language: null },
      },
    });
    const target = await open();
    button(group(target, 'Local Whisper'), 'Edit…').click(); await settle();
    fill(field(target, 'custom-stt-base-url'), 'http://typing.test');
    // The rows behind the sheet still show the saved address.
    expect(service(target, 'Local Whisper').querySelector('.row-detail')?.textContent).toBe('http://a.test/v1');
    expect(rowsOf(group(target, 'Local Whisper'))[0][1]).toBe('http://a.test/v1');
    button(dialog(target)!, 'Cancel').click(); await settle();
    expect(called('set_custom_stt_endpoint')).toHaveLength(0);

    button(group(target, 'Local Whisper'), 'Edit…').click(); await settle();
    expect(field(target, 'custom-stt-base-url').value).toBe('http://a.test/v1');
  });

  it('shows a new address once it is saved', async () => {
    backend({
      providers: [P.openai(), P.custom(true, 'Local Whisper')],
      config: {
        stt_provider: 'custom_stt',
        http_stt_config: { custom_base_url: 'http://a.test/v1', custom_display_name: 'Local Whisper', custom_model: null, language: null },
      },
    });
    const target = await open();
    button(group(target, 'Local Whisper'), 'Edit…').click(); await settle();
    fill(field(target, 'custom-stt-base-url'), 'http://b.test/v1');
    button(dialog(target)!, 'Save & Use').click(); await settle();
    expect(service(target, 'Local Whisper').querySelector('.row-detail')?.textContent).toBe('http://b.test/v1');
    expect(rowsOf(group(target, 'Local Whisper'))[0][1]).toBe('http://b.test/v1');
  });

  it('does not open by itself when the custom endpoint is the provider in use', async () => {
    backend({
      providers: [P.openai(), P.custom(true, 'Local Whisper')],
      config: {
        stt_provider: 'custom_stt',
        http_stt_config: { custom_base_url: 'http://a.test/v1', custom_display_name: null, custom_model: null, language: null },
      },
    });
    const target = await open();
    expect(dialog(target)).toBeNull();
  });
});

describe('Transcription pane: Apple speech model download', () => {
  it('shows the download in the Apple Speech row, from checking to done, and clears it after four seconds', async () => {
    vi.useFakeTimers();
    const state = backend({ providers: [P.apple('not_installed'), P.openai()], config: { stt_provider: 'openai' } });
    const target = await open();
    const row = () => service(target, 'Apple Speech');
    expect(row().querySelector('.row-extra')).toBeNull();

    button(row(), 'Download').click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('download_apple_stt_model', { locale: 'auto' });
    expect(row().querySelector('.row-extra')?.textContent).toContain('Checking model availability…');
    expect(row().querySelector('[role="progressbar"]')).not.toBeNull();
    // Pressing it again would start a second download.
    expect(button(row(), 'Download').disabled).toBe(true);

    emit('apple-stt-model-progress', { locale: 'en_US', progress: 0.42, finished: false, error: null });
    await settle();
    expect(row().querySelector('.row-extra')?.textContent).toContain('Downloading… 42%');
    const bar = row().querySelector('[role="progressbar"]')!;
    expect(bar.getAttribute('aria-valuenow')).toBe('42');
    expect(bar.querySelector<HTMLElement>('div')?.style.width).toBe('42%');

    // The model is on disk once the download ends, which the list reflects.
    state.providers = [P.apple('installed'), P.openai()];
    vi.advanceTimersByTime(600);
    emit('apple-stt-model-progress', { locale: 'en_US', progress: 1, finished: true, error: null });
    await settle();
    expect(row().querySelector('.row-extra')?.textContent).toContain('Model downloaded');
    expect(row().querySelector('[role="progressbar"]')).toBeNull();
    expect(row().querySelector('.row-detail')?.textContent).toBe('On this Mac · no API key needed');
    expect(row().querySelector('.row-trailing')).toBeNull();

    vi.advanceTimersByTime(3900); await settle();
    expect(row().querySelector('.row-extra')?.textContent).toContain('Model downloaded');
    vi.advanceTimersByTime(200); await settle();
    expect(row().querySelector('.row-extra')).toBeNull();
  });

  it('says so when the model turns out to be installed already', async () => {
    vi.useFakeTimers();
    backend({ providers: [P.apple('not_installed'), P.openai()], config: { stt_provider: 'openai' } });
    const target = await open();
    button(service(target, 'Apple Speech'), 'Download').click(); await settle();
    // Finished at once: there was nothing to download.
    emit('apple-stt-model-progress', { locale: 'en_US', progress: 1, finished: true, error: null });
    await settle();
    expect(service(target, 'Apple Speech').querySelector('.row-extra')?.textContent).toContain('Model already installed');
  });

  it('shows why a download failed, as an alert in the row', async () => {
    vi.useFakeTimers();
    backend({ providers: [P.apple('not_installed'), P.openai()], config: { stt_provider: 'openai' } });
    const target = await open();
    button(service(target, 'Apple Speech'), 'Download').click(); await settle();
    emit('apple-stt-model-progress', {
      locale: 'en_US', progress: 0, finished: true, error: 'Download failed or model unavailable for this locale',
    });
    await settle();
    const row = service(target, 'Apple Speech');
    expect(row.querySelector('[role="alert"]')?.textContent).toBe('Download failed or model unavailable for this locale');
    // The download button is back for another try.
    expect(button(row, 'Download').disabled).toBe(false);
    vi.advanceTimersByTime(4100); await settle();
    expect(row.querySelector('.row-extra')).toBeNull();
  });

  it('reports a download that cannot be started', async () => {
    failing('download_apple_stt_model', new Error('no network'), {
      providers: [P.apple('not_installed'), P.openai()], config: { stt_provider: 'openai' },
    });
    const target = await open();
    button(service(target, 'Apple Speech'), 'Download').click(); await settle();
    expect(errorToast(target)).toContain('Failed to start model download');
    expect(service(target, 'Apple Speech').querySelector('[role="alert"]')?.textContent).toContain('no network');
    expect(button(service(target, 'Apple Speech'), 'Download').disabled).toBe(false);
  });
});
