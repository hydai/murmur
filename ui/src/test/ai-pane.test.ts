import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync } from 'svelte';
import userEvent from '@testing-library/user-event';
import LlmConfig from '../components/settings/LlmConfig.svelte';
import {
  clearDraft, getDraft, hasDraft, resetDrafts, setDraft,
} from '../components/settings/promptDrafts.svelte';
import { VOICE_COMMANDS } from '../lib/voiceCommands';
import { button, render, settle, unmountAll } from './helpers';

const mocks = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('../lib/tauri', () => ({ safeInvoke: mocks.invoke }));

/** What the backend lists, one entry per processor. */
const P = {
  apple: (available = true) => ({
    id: 'apple_llm', name: 'Apple Intelligence', available, default_model: '(system default)',
    provider_type: 'local', requires_api_key: false, configured: true, api_key_name: null,
  }),
  gemini: (available = true) => ({
    id: 'gemini', name: 'Gemini CLI', available, default_model: 'gemini-3-flash-preview',
    provider_type: 'cli', requires_api_key: false, configured: true, api_key_name: null,
  }),
  copilot: (available = true) => ({
    id: 'copilot', name: 'Copilot CLI', available, default_model: 'gpt-5-mini',
    provider_type: 'cli', requires_api_key: false, configured: true, api_key_name: null,
  }),
  openai: (configured = true) => ({
    id: 'openai_api', name: 'OpenAI API', available: configured, default_model: 'gpt-4o-mini',
    provider_type: 'http', requires_api_key: true, configured, api_key_name: 'openai',
  }),
  claude: (configured = false) => ({
    id: 'claude_api', name: 'Claude API', available: configured, default_model: 'claude-sonnet-4-20250514',
    provider_type: 'http', requires_api_key: true, configured, api_key_name: 'anthropic',
  }),
  geminiApi: (configured = false) => ({
    id: 'gemini_api', name: 'Gemini API', available: configured, default_model: 'gemini-2.0-flash',
    provider_type: 'http', requires_api_key: true, configured, api_key_name: 'google_ai',
  }),
  custom: (configured = false, name = 'Custom Endpoint', default_model = 'gpt-4o-mini') => ({
    id: 'custom_api', name, available: configured, default_model,
    provider_type: 'custom', requires_api_key: false, configured, api_key_name: 'custom_llm',
  }),
};

type Listed = ReturnType<(typeof P)[keyof typeof P]>;

/** A prompt as `get_prompts` lists it; each test overrides what it is about. */
const prompt = (name: string, overrides: Record<string, unknown> = {}) => ({
  name,
  title: name,
  description: `About ${name}.`,
  required_placeholders: [] as string[],
  task_variant: name,
  content: `content of ${name}`,
  is_override: false,
  default_content: `default of ${name}`,
  ...overrides,
});

type PromptEntry = ReturnType<typeof prompt>;

const CONFIG = {
  llm_processor: 'gemini',
  llm_model: null as string | null,
  http_llm_config: null as null | Record<string, string | null>,
};

/**
 * A backend that remembers what it is told, the way the real one does: a saved
 * key marks its processor configured, a switch moves the processor in use, a
 * saved endpoint becomes the custom processor, a saved prompt becomes an
 * override. Every command succeeds.
 */
function backend(init: { processors?: Listed[]; config?: Partial<typeof CONFIG>; prompts?: PromptEntry[] } = {}) {
  const state = {
    processors: init.processors ?? [P.apple(), P.gemini(), P.claude(true)],
    config: { ...CONFIG, ...init.config },
    prompts: init.prompts ?? VOICE_COMMANDS.map((command) => prompt(command.prompt)),
  };
  mocks.invoke.mockImplementation(async (command: string, args?: any) => {
    switch (command) {
      case 'get_llm_processors': return state.processors;
      case 'get_config': return state.config;
      case 'get_prompts': return state.prompts;
      case 'save_api_key':
        state.processors = state.processors.map(p => (p.api_key_name === args.provider
          ? { ...p, configured: true, available: true }
          : p));
        return undefined;
      case 'set_llm_processor':
        state.config = { ...state.config, llm_processor: args.processor };
        return undefined;
      case 'set_llm_model':
        state.config = { ...state.config, llm_model: args.model || null };
        return undefined;
      case 'set_custom_llm_endpoint':
        state.config = {
          ...state.config,
          http_llm_config: { custom_base_url: args.baseUrl, custom_display_name: args.displayName },
        };
        state.processors = state.processors.map(p => (p.id === 'custom_api'
          ? { ...p, configured: true, available: true, name: args.displayName ?? 'Custom Endpoint' }
          : p));
        return undefined;
      case 'set_prompt':
        state.prompts = state.prompts.map(p => (p.name === args.params.name
          ? { ...p, content: args.params.content, is_override: true }
          : p));
        return undefined;
      case 'reset_prompt':
        state.prompts = state.prompts.map(p => (p.name === args.params.name
          ? { ...p, content: p.default_content, is_override: false }
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

/** `command` is answered, as `backend` would, only once the returned `release` is called; everything else is as `backend` made it. */
function holding(command: string) {
  let release!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  const working = mocks.invoke.getMockImplementation()!;
  mocks.invoke.mockImplementation(async (name: string, args?: unknown) => {
    if (name === command) {
      await held;
    }
    return working(name, args);
  });
  return release;
}

beforeEach(() => {
  resetDrafts();
  backend();
});

afterEach(async () => {
  await unmountAll();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

async function open() {
  const { target } = render(LlmConfig, {});
  await settle();
  return target;
}

const inUse = (target: Element) => [...target.querySelectorAll('[aria-current="true"]')];
const errorToast = (target: Element) => target.querySelector('.toast-error')?.textContent;
const successToast = (target: Element) => target.querySelector('.toast-success')?.textContent?.trim();
const dialog = (target: Element) => target.querySelector<HTMLElement>('[role="dialog"]');
const dialogTitle = (target: Element) => dialog(target)?.querySelector('h2')?.textContent;
const field = (target: Element, id: string) => dialog(target)!.querySelector<HTMLInputElement>(`#${id}`)!;
/** The one field of the API key sheet, which ApiKeySheet gives an id of its own. */
const keyField = (target: Element) => dialog(target)!.querySelector<HTMLInputElement>('input')!;
const modelField = (target: Element) => target.querySelector<HTMLInputElement>('input[aria-label="Model"]');
const editor = (target: Element) => target.querySelector<HTMLTextAreaElement>('textarea')!;
const called = (command: string) => mocks.invoke.mock.calls.filter(([name]) => name === command);
const backButton = (target: Element) =>
  target.querySelector<HTMLButtonElement>('[aria-label="Back to AI Processing"]')!;

function group(target: Element, title: string): HTMLElement {
  const found = [...target.querySelectorAll<HTMLElement>('.group')]
    .find(candidate => candidate.querySelector('.group-title')?.textContent === title);
  expect(found, `group ${title}`).toBeDefined();
  return found!;
}

const groupTitles = (target: Element) => [...target.querySelectorAll('.group-title')].map(title => title.textContent);

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

/** Every voice command as [name, the mark at the end of its row]. */
const marksOf = (target: Element) => [...group(target, 'Voice Commands').querySelectorAll('.row')].map(row => [
  row.querySelector('.row-label')?.textContent,
  row.querySelector('.row-accessory')?.textContent?.trim(),
]);

/** A row of the Voice Commands group, found by its name. */
function voiceRow(target: Element, name: string): HTMLElement {
  const found = [...group(target, 'Voice Commands').querySelectorAll<HTMLElement>('.row')]
    .find(row => row.querySelector('.row-label')?.textContent === name);
  expect(found, `voice command ${name}`).toBeDefined();
  return found!;
}

function fill(input: HTMLInputElement | HTMLTextAreaElement, value: string) {
  input.value = value;
  input.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
}

/** A key press in `element`, with the properties an input method sets when it has handled the key. */
function press(element: Element, key: string, { isComposing = false, keyCode = 13 } = {}) {
  const event = new KeyboardEvent('keydown', { key, isComposing, bubbles: true });
  // jsdom leaves keyCode out of the init dictionary, so it has to be defined on the event itself.
  Object.defineProperty(event, 'keyCode', { get: () => keyCode });
  element.dispatchEvent(event);
  flushSync();
}

async function openEditor(target: Element, title: string) {
  button(target, title).click();
  await settle();
}

describe('AI Processing pane: services', () => {
  it('lists the services local first, then command-line tools, cloud, and custom, with the copy the spec gives', async () => {
    backend({
      processors: [P.gemini(), P.copilot(false), P.apple(), P.openai(true), P.claude(false), P.geminiApi(false), P.custom(false)],
      config: { llm_processor: 'gemini' },
    });
    const target = await open();
    expect(target.querySelector('h1')?.textContent).toBe('AI Processing');
    expect(rowsOf(group(target, 'Service'))).toEqual([
      ['Apple Intelligence', 'On this Mac', []],
      ['Gemini CLI', 'Command-line tool', []],
      ['Copilot CLI', 'Not installed', []],
      ['OpenAI API', 'Cloud · key saved', []],
      ['Claude API', 'Cloud', ['Add API Key…']],
      ['Gemini API', 'Cloud', ['Add API Key…']],
      ['Custom Endpoint', 'Any OpenAI-compatible server', ['Set Up…']],
    ]);
  });

  it('names each button after the service it is for, since the visible text is the same on several rows', async () => {
    backend({
      processors: [P.gemini(), P.openai(true), P.claude(false), P.geminiApi(false), P.custom(false)],
      config: { llm_processor: 'openai_api' },
    });
    const target = await open();
    const names = (container: Element) =>
      [...container.querySelectorAll('.row-trailing button')].map(b => b.getAttribute('aria-label'));
    expect(names(group(target, 'Service'))).toEqual([
      'Add API Key for Claude API', 'Add API Key for Gemini API', 'Set up Custom Endpoint',
    ]);
    expect(names(group(target, 'OpenAI API'))).toEqual(['Change API key for OpenAI API']);
  });

  it('names the button that edits the endpoint in use after it', async () => {
    backend({
      processors: [P.gemini(), P.custom(true, 'Local Ollama')],
      config: {
        llm_processor: 'custom_api',
        http_llm_config: { custom_base_url: 'http://localhost:11434/v1', custom_display_name: 'Local Ollama' },
      },
    });
    const target = await open();
    expect(button(group(target, 'Local Ollama'), 'Edit…').getAttribute('aria-label'))
      .toBe('Edit endpoint of Local Ollama');
  });

  it('keeps the install hint of a tool that is not installed in its row, outside the row button', async () => {
    backend({ processors: [P.gemini(false), P.copilot(false), P.apple()], config: { llm_processor: 'apple_llm' } });
    const target = await open();
    const hints = {
      'Gemini CLI': 'Install from: https://github.com/google/generative-ai-cli',
      'Copilot CLI': 'Install: npm install -g @githubnext/github-copilot-cli',
    };
    for (const [name, hint] of Object.entries(hints)) {
      const row = service(target, name);
      // The button's second line stays short: a button cuts a long line off and its text cannot be selected.
      expect(row.querySelector('.row-detail')?.textContent).toBe('Not installed');
      const holder = [...row.querySelectorAll('*')]
        .find(element => element.children.length === 0 && element.textContent?.trim() === hint);
      expect(holder, `hint of ${name}`).toBeDefined();
      expect(holder!.closest('button')).toBeNull();
    }
  });

  it('gives a row room for a hint only when it has one', async () => {
    backend({
      processors: [P.gemini(true), P.copilot(false), P.apple(false), P.openai(true), P.claude(false), P.custom(false)],
      config: { llm_processor: 'gemini' },
    });
    const target = await open();
    // An installed tool, the Mac's own model, cloud services and the custom endpoint have nothing to add.
    expect([...group(target, 'Service').querySelectorAll('.row')]
      .filter(row => row.querySelector('.row-extra'))
      .map(row => row.querySelector('.row-label')?.textContent)).toEqual(['Copilot CLI']);
  });

  it('gives a tool with no known install hint nothing under its row', async () => {
    backend({
      processors: [{ ...P.gemini(false), id: 'other_cli', name: 'Other CLI' }, P.apple()],
      config: { llm_processor: 'apple_llm' },
    });
    const target = await open();
    expect(service(target, 'Other CLI').querySelector('.row-detail')?.textContent).toBe('Not installed');
    expect(service(target, 'Other CLI').querySelector('.row-extra')).toBeNull();
  });

  it('says Apple Intelligence is not available when this Mac cannot run it', async () => {
    backend({ processors: [P.apple(false), P.gemini()], config: { llm_processor: 'gemini' } });
    const target = await open();
    expect(service(target, 'Apple Intelligence').querySelector('.row-detail')?.textContent)
      .toBe('Not available on this Mac');
  });

  it('gives each service the icon of its kind', async () => {
    backend({
      processors: [P.apple(), P.gemini(), P.openai(true), P.custom(true, 'Local Ollama')],
      config: { llm_processor: 'gemini' },
    });
    const target = await open();
    const icon = (name: string) => service(target, name).querySelector('.row-icon svg')?.getAttribute('class');
    expect(icon('Apple Intelligence')).toContain('lucide-laptop');
    expect(icon('Gemini CLI')).toContain('lucide-terminal');
    expect(icon('OpenAI API')).toContain('lucide-cloud');
    expect(icon('Local Ollama')).toContain('lucide-server');
  });

  it('lists a configured custom endpoint and marks it in use', async () => {
    backend({
      processors: [
        P.gemini(),
        {
          id: 'custom_api', name: 'Local Ollama', provider_type: 'custom', configured: true, available: true,
          requires_api_key: false, default_model: '', api_key_name: 'custom_llm',
        },
      ],
      config: {
        llm_processor: 'custom_api',
        http_llm_config: { custom_base_url: 'http://localhost:11434/v1', custom_display_name: 'Local Ollama' },
      },
    });
    const target = await open();
    const row = target.querySelector('[aria-current="true"]')!;
    expect(row.textContent).toContain('Local Ollama');
    expect(row.textContent).toContain('http://localhost:11434/v1');
    expect(inUse(target)).toHaveLength(1);
  });

  it('marks the processor in use and switches on click without a success toast', async () => {
    backend({ processors: [P.apple(), P.gemini(), P.claude(true)], config: { llm_processor: 'gemini' } });
    const target = await open();
    expect(inUse(target)).toHaveLength(1);
    expect(inUse(target)[0].textContent).toContain('Gemini CLI');
    button(target, 'Claude API').click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('set_llm_processor', { processor: 'claude_api' });
    expect(target.querySelector('[aria-current="true"]')?.textContent).toContain('Claude API');
    expect(inUse(target)).toHaveLength(1);
    expect(target.querySelector('.toast-success')).toBeNull();
  });

  it('keeps the processor in use when switching fails', async () => {
    failing('set_llm_processor', new Error('boom'), {
      processors: [P.apple(), P.gemini(), P.claude(true)], config: { llm_processor: 'gemini' },
    });
    const target = await open();
    button(target, 'Claude API').click(); await settle();
    expect(target.querySelector('[aria-current="true"]')?.textContent).toContain('Gemini CLI');
    expect(inUse(target)).toHaveLength(1);
    expect(target.querySelector('.toast-error')?.textContent).toContain('Failed to switch processor');
  });

  it('disables a command-line tool that is not installed, and an Apple Intelligence this Mac cannot run', async () => {
    backend({ processors: [P.apple(false), P.gemini(), P.copilot(false)], config: { llm_processor: 'gemini' } });
    const target = await open();
    for (const name of ['Apple Intelligence', 'Copilot CLI']) {
      const main = service(target, name).querySelector<HTMLButtonElement>('.row-main')!;
      expect(main.disabled, name).toBe(true);
      main.click();
    }
    await settle();
    expect(service(target, 'Gemini CLI').querySelector<HTMLButtonElement>('.row-main')?.disabled).toBe(false);
    expect(called('set_llm_processor')).toHaveLength(0);
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('does not switch to a processor that cannot run even if a click gets past the disabled row', async () => {
    // The row is disabled, so this only guards the handler against a click that gets through.
    backend({ processors: [P.gemini(), P.copilot(false)], config: { llm_processor: 'gemini' } });
    const target = await open();
    const main = service(target, 'Copilot CLI').querySelector<HTMLButtonElement>('.row-main')!;
    main.disabled = false;
    main.click(); await settle();
    expect(called('set_llm_processor')).toHaveLength(0);
    expect(inUse(target)[0].textContent).toContain('Gemini CLI');
  });

  it('says so when the processors or the settings cannot be loaded', async () => {
    failing('get_llm_processors', new Error('unavailable'));
    let target = await open();
    expect(errorToast(target)).toContain('Failed to load LLM processors');
    expect(groupTitles(target)).not.toContain('Service');
    await unmountAll();

    failing('get_config', new Error('unavailable'));
    target = await open();
    expect(errorToast(target)).toContain('Failed to load config');
  });

  it('treats an empty list and settings without the optional parts as defaults', async () => {
    mocks.invoke.mockImplementation(async (command: string) => {
      switch (command) {
        case 'get_llm_processors':
        case 'get_prompts': return [];
        case 'get_config': return { llm_processor: 'gemini' };
        default: return undefined;
      }
    });
    const target = await open();
    // A load that fell over would say so, so its silence is part of the point.
    expect(target.querySelector('.toast')).toBeNull();
    expect(groupTitles(target)).toEqual(['Model', 'Voice Commands']);
    expect(modelField(target)?.value).toBe('');
    expect(modelField(target)?.placeholder).toBe('Default');
  });
});

describe('AI Processing pane: the processor in use', () => {
  it('shows the saved key of a cloud service in use', async () => {
    backend({ processors: [P.gemini(), P.openai(true)], config: { llm_processor: 'openai_api' } });
    const target = await open();
    expect(groupTitles(target)).toEqual(['Service', 'OpenAI API', 'Model', 'Voice Commands']);
    expect(rowsOf(group(target, 'OpenAI API'))).toEqual([['API key', 'Saved', ['Change…']]]);
  });

  it('shows the address of the custom endpoint in use', async () => {
    backend({
      processors: [P.gemini(), P.custom(true, 'Local Ollama')],
      config: {
        llm_processor: 'custom_api',
        http_llm_config: { custom_base_url: 'http://localhost:11434/v1', custom_display_name: 'Local Ollama' },
      },
    });
    const target = await open();
    expect(rowsOf(group(target, 'Local Ollama'))).toEqual([['Endpoint', 'http://localhost:11434/v1', ['Edit…']]]);
    expect(service(target, 'Local Ollama').querySelector('.row-detail')?.textContent).toBe('http://localhost:11434/v1');
  });

  it('has no group of its own for a tool, an on-device model, or a service that has no key yet', async () => {
    backend({ processors: [P.apple(), P.gemini(), P.claude(false)], config: { llm_processor: 'gemini' } });
    let target = await open();
    expect(groupTitles(target)).toEqual(['Service', 'Model', 'Voice Commands']);
    await unmountAll();

    backend({ processors: [P.apple(), P.gemini(), P.claude(false)], config: { llm_processor: 'apple_llm' } });
    target = await open();
    expect(groupTitles(target)).toEqual(['Service', 'Voice Commands']);
    await unmountAll();

    // Never says "Saved" for a key that is not there.
    backend({ processors: [P.gemini(), P.claude(false)], config: { llm_processor: 'claude_api' } });
    target = await open();
    expect(groupTitles(target)).toEqual(['Service', 'Model', 'Voice Commands']);
  });
});

describe('AI Processing pane: model', () => {
  it('hides the model field for Apple Intelligence', async () => {
    backend({ processors: [P.apple(), P.gemini()], config: { llm_processor: 'apple_llm' } });
    const target = await open();
    expect(modelField(target)).toBeNull();
    expect(groupTitles(target)).not.toContain('Model');
  });

  it('brings the model field back when another processor is chosen', async () => {
    backend({ processors: [P.apple(), P.gemini()], config: { llm_processor: 'apple_llm' } });
    const target = await open();
    button(target, 'Gemini CLI').click(); await settle();
    expect(modelField(target)).not.toBeNull();
  });

  it('names the processor in use, and shows its default model as the placeholder', async () => {
    backend({ processors: [P.gemini(), P.openai(true)], config: { llm_processor: 'gemini' } });
    const target = await open();
    expect(rowsOf(group(target, 'Model')).map(([label]) => label)).toEqual(['Model for Gemini CLI']);
    expect(modelField(target)?.placeholder).toBe('Default: gemini-3-flash-preview');
    button(target, 'OpenAI API').click(); await settle();
    expect(rowsOf(group(target, 'Model')).map(([label]) => label)).toEqual(['Model for OpenAI API']);
    expect(modelField(target)?.placeholder).toBe('Default: gpt-4o-mini');
  });

  it('shows the model that is saved', async () => {
    backend({ config: { llm_processor: 'gemini', llm_model: 'gemini-2.5-pro' } });
    const target = await open();
    expect(modelField(target)?.value).toBe('gemini-2.5-pro');
  });

  it('saves the model on Enter', async () => {
    const target = await open();
    const input = modelField(target)!;
    fill(input, 'gpt-4.1');
    press(input, 'Enter'); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('set_llm_model', { model: 'gpt-4.1' });
    expect(target.querySelector('.toast-success')?.textContent).toContain('Model saved');
  });

  it('saves a changed model when the field loses focus', async () => {
    const target = await open();
    const input = modelField(target)!;
    input.focus();
    fill(input, 'gpt-4.1');
    input.blur(); await settle();
    expect(called('set_llm_model')).toHaveLength(1);
    expect(mocks.invoke).toHaveBeenCalledWith('set_llm_model', { model: 'gpt-4.1' });
    expect(successToast(target)).toContain('Model saved');
  });

  it('does not save when the field loses focus unchanged', async () => {
    backend({ config: { llm_processor: 'gemini', llm_model: 'gpt-4.1' } });
    const target = await open();
    const input = modelField(target)!;
    input.focus();
    input.blur(); await settle();
    // Typing the saved value back is no change either.
    input.focus();
    fill(input, 'something else');
    fill(input, 'gpt-4.1');
    input.blur(); await settle();
    expect(called('set_llm_model')).toHaveLength(0);
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('saves once when Enter is followed by the field losing focus', async () => {
    const target = await open();
    const input = modelField(target)!;
    input.focus();
    fill(input, 'gpt-4.1');
    press(input, 'Enter'); await settle();
    input.blur(); await settle();
    expect(called('set_llm_model')).toHaveLength(1);
  });

  it('saves the next change after an earlier one', async () => {
    const target = await open();
    const input = modelField(target)!;
    input.focus();
    fill(input, 'gpt-4.1');
    input.blur(); await settle();
    input.focus();
    fill(input, 'gpt-5');
    input.blur(); await settle();
    expect(called('set_llm_model').map(([, args]) => args)).toEqual([{ model: 'gpt-4.1' }, { model: 'gpt-5' }]);
  });

  it('goes back to the default model when the field is emptied', async () => {
    backend({ config: { llm_processor: 'gemini', llm_model: 'gpt-4.1' } });
    const target = await open();
    const input = modelField(target)!;
    fill(input, '');
    press(input, 'Enter'); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('set_llm_model', { model: '' });
    expect(successToast(target)).toContain('Using the default model');
  });

  it('saves the model without the spaces around it', async () => {
    const target = await open();
    const input = modelField(target)!;
    fill(input, '  gpt-4.1 ');
    press(input, 'Enter'); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('set_llm_model', { model: 'gpt-4.1' });
  });

  it('keeps what was typed and says why when the model cannot be saved', async () => {
    failing('set_llm_model', new Error('disk full'));
    const target = await open();
    const input = modelField(target)!;
    fill(input, 'gpt-4.1');
    press(input, 'Enter'); await settle();
    expect(errorToast(target)).toContain('Failed to set model');
    expect(errorToast(target)).toContain('disk full');
    expect(target.querySelector('.toast-success')).toBeNull();
    expect(input.value).toBe('gpt-4.1');
  });

  it('does not save on the Enter that confirms an input method composition', async () => {
    const target = await open();
    const input = modelField(target)!;
    fill(input, 'gpt');
    // The key an input method handles reports isComposing, or keyCode 229 once the composition has ended.
    press(input, 'Enter', { isComposing: true });
    press(input, 'Enter', { keyCode: 229 });
    await settle();
    expect(called('set_llm_model')).toHaveLength(0);
    press(input, 'Enter'); await settle();
    expect(called('set_llm_model')).toHaveLength(1);
  });

  it('ignores the other keys', async () => {
    const target = await open();
    const input = modelField(target)!;
    fill(input, 'gpt-4.1');
    press(input, 'a', { keyCode: 65 });
    await settle();
    expect(called('set_llm_model')).toHaveLength(0);
  });
});

describe('AI Processing pane: API key sheet', () => {
  it('opens titled for a service without a key, from its button or its row', async () => {
    backend({ processors: [P.gemini(), P.claude(false)], config: { llm_processor: 'gemini' } });
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    expect(dialogTitle(target)).toBe('Add API Key for Claude API');
    expect(called('set_llm_processor')).toHaveLength(0);
    button(dialog(target)!, 'Cancel').click(); await settle();
    expect(dialog(target)).toBeNull();

    button(target, 'Claude API').click(); await settle();
    expect(dialogTitle(target)).toBe('Add API Key for Claude API');
    expect(called('set_llm_processor')).toHaveLength(0);
  });

  it('renders the sheet inside the pane', async () => {
    backend({ processors: [P.gemini(), P.claude(false)] });
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    expect(target.querySelector('.pane [role="dialog"]')).not.toBeNull();
  });

  it('opens titled as a change for a key that is already saved', async () => {
    backend({ processors: [P.gemini(), P.openai(true)], config: { llm_processor: 'openai_api' } });
    const target = await open();
    button(group(target, 'OpenAI API'), 'Change…').click(); await settle();
    expect(dialogTitle(target)).toBe('Change API Key for OpenAI API');
    expect(button(dialog(target)!, 'Save').textContent?.trim()).toBe('Save');
  });

  it('saves a new key under the key name of the service, switches to it, and confirms with a toast', async () => {
    backend({ processors: [P.gemini(), P.claude(false)], config: { llm_processor: 'gemini' } });
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    expect(button(dialog(target)!, 'Save & Use')).toBeDefined();
    fill(keyField(target), 'sk-ant-test');
    button(dialog(target)!, 'Save & Use').click(); await settle();

    // The key is stored under the name the backend gave, not the processor's id.
    expect(mocks.invoke).toHaveBeenCalledWith('save_api_key', { provider: 'anthropic', apiKey: 'sk-ant-test' });
    expect(mocks.invoke).toHaveBeenCalledWith('set_llm_processor', { processor: 'claude_api' });
    const order = mocks.invoke.mock.calls.map(([name]) => name);
    expect(order.indexOf('save_api_key')).toBeLessThan(order.indexOf('set_llm_processor'));
    expect(dialog(target)).toBeNull();
    expect(successToast(target)).toContain('API key saved');
    expect(inUse(target)[0].textContent).toContain('Claude API');
    expect(rowsOf(group(target, 'Claude API'))).toEqual([['API key', 'Saved', ['Change…']]]);
    expect(rowsOf(group(target, 'Service')).find(([label]) => label === 'Claude API'))
      .toEqual(['Claude API', 'Cloud · key saved', []]);
  });

  it('saves a changed key as a key change, with a toast', async () => {
    backend({ processors: [P.gemini(), P.openai(true)], config: { llm_processor: 'openai_api' } });
    const target = await open();
    button(group(target, 'OpenAI API'), 'Change…').click(); await settle();
    fill(keyField(target), 'sk-new');
    button(dialog(target)!, 'Save').click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('save_api_key', { provider: 'openai', apiKey: 'sk-new' });
    expect(mocks.invoke).toHaveBeenCalledWith('set_llm_processor', { processor: 'openai_api' });
    expect(dialog(target)).toBeNull();
    expect(successToast(target)).toContain('API key saved');
  });

  it('says Saving… on the button while the key is being saved, then closes', async () => {
    backend({ processors: [P.gemini(), P.claude(false)], config: { llm_processor: 'gemini' } });
    const release = holding('save_api_key');
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    fill(keyField(target), 'sk-ant-test');
    button(dialog(target)!, 'Save & Use').click(); await settle();
    const save = dialog(target)!.querySelector<HTMLButtonElement>('button[type="submit"]')!;
    expect(save.textContent?.trim()).toBe('Saving…');
    expect(save.disabled).toBe(true);
    release(); await settle();
    expect(dialog(target)).toBeNull();
  });

  it('lets the confirmation go away by itself after two seconds', async () => {
    vi.useFakeTimers();
    backend({ processors: [P.gemini(), P.claude(false)], config: { llm_processor: 'gemini' } });
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    fill(keyField(target), 'sk-ant-test');
    button(dialog(target)!, 'Save & Use').click(); await settle();
    expect(successToast(target)).toContain('API key saved');
    vi.advanceTimersByTime(1900); await settle();
    expect(successToast(target)).toContain('API key saved');
    vi.advanceTimersByTime(200); await settle();
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('explains an empty key inside the sheet and saves nothing', async () => {
    backend({ processors: [P.gemini(), P.claude(false)] });
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    button(dialog(target)!, 'Save & Use').click(); await settle();
    expect(dialog(target)?.querySelector('.sheet-error')?.textContent).toBe('API key cannot be empty');
    expect(called('save_api_key')).toHaveLength(0);
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('keeps the sheet open and says why when the key cannot be saved', async () => {
    failing('save_api_key', new Error('keychain locked'), {
      processors: [P.gemini(), P.claude(false)], config: { llm_processor: 'gemini' },
    });
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    fill(keyField(target), 'sk-ant-test');
    button(dialog(target)!, 'Save & Use').click(); await settle();
    expect(errorToast(target)).toContain('Failed to save API key');
    expect(errorToast(target)).toContain('keychain locked');
    expect(dialog(target)).not.toBeNull();
    expect(called('set_llm_processor')).toHaveLength(0);
    expect(inUse(target)[0].textContent).toContain('Gemini CLI');
  });

  it('keeps the processor in use when it is the switch that fails after the key was saved', async () => {
    failing('set_llm_processor', new Error('boom'), {
      processors: [P.gemini(), P.claude(false)], config: { llm_processor: 'gemini' },
    });
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    fill(keyField(target), 'sk-ant-test');
    button(dialog(target)!, 'Save & Use').click(); await settle();
    expect(errorToast(target)).toContain('Failed to save API key');
    expect(dialog(target)).not.toBeNull();
    expect(inUse(target)[0].textContent).toContain('Gemini CLI');
  });

  it('takes the failure with it when the sheet it came from is cancelled', async () => {
    failing('save_api_key', new Error('keychain locked'), {
      processors: [P.gemini(), P.claude(false)], config: { llm_processor: 'gemini' },
    });
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    fill(keyField(target), 'sk-ant-test');
    button(dialog(target)!, 'Save & Use').click(); await settle();
    expect(errorToast(target)).toContain('keychain locked');
    button(dialog(target)!, 'Cancel').click(); await settle();
    expect(dialog(target)).toBeNull();
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('forgets what was typed when the sheet is cancelled', async () => {
    backend({ processors: [P.gemini(), P.claude(false)] });
    const target = await open();
    button(target, 'Add API Key…').click(); await settle();
    fill(keyField(target), 'sk-secret');
    button(dialog(target)!, 'Cancel').click(); await settle();
    button(target, 'Add API Key…').click(); await settle();
    expect(keyField(target).value).toBe('');
    expect(called('save_api_key')).toHaveLength(0);
  });
});

describe('AI Processing pane: custom endpoint sheet', () => {
  it('opens from the row or the Set Up button of an endpoint that is not set up', async () => {
    backend({ processors: [P.gemini(), P.custom(false)], config: { llm_processor: 'gemini' } });
    const target = await open();
    button(target, 'Set Up…').click(); await settle();
    expect(dialogTitle(target)).toBe('Custom Endpoint');
    button(dialog(target)!, 'Cancel').click(); await settle();
    expect(dialog(target)).toBeNull();

    button(target, 'Custom Endpoint').click(); await settle();
    expect(dialogTitle(target)).toBe('Custom Endpoint');
    expect(called('set_llm_processor')).toHaveLength(0);
  });

  it('lays out its fields with the labels and placeholders the spec gives', async () => {
    backend({ processors: [P.gemini(), P.custom(false)] });
    const target = await open();
    button(target, 'Set Up…').click(); await settle();
    const sheet = dialog(target)!;
    expect([...sheet.querySelectorAll('label')].map(label => label.textContent))
      .toEqual(['Base URL', 'API Key', 'Display Name']);
    expect([...sheet.querySelectorAll('input')].map(input => [input.id, input.type, input.placeholder])).toEqual([
      ['custom-llm-base-url', 'text', 'http://localhost:11434/v1'],
      ['custom-llm-api-key', 'password', 'Only if the server needs one'],
      ['custom-llm-display-name', 'text', 'Local Ollama'],
    ]);
    // Every label points at its own field.
    for (const label of sheet.querySelectorAll('label')) {
      expect(sheet.querySelector(`#${label.htmlFor}`)).not.toBeNull();
    }
    expect(document.activeElement).toBe(field(target, 'custom-llm-base-url'));
  });

  it('cannot be submitted without a Base URL', async () => {
    backend({ processors: [P.gemini(), P.custom(false)] });
    const target = await open();
    button(target, 'Set Up…').click(); await settle();
    const submit = button(dialog(target)!, 'Save & Use');
    expect(submit.type).toBe('submit');
    expect(submit.disabled).toBe(true);
    fill(field(target, 'custom-llm-base-url'), '   ');
    expect(submit.disabled).toBe(true);
    fill(field(target, 'custom-llm-base-url'), 'http://localhost:11434/v1');
    expect(submit.disabled).toBe(false);
  });

  it('saves the endpoint, uses it, and confirms with a toast', async () => {
    backend({ processors: [P.gemini(), P.custom(false)], config: { llm_processor: 'gemini' } });
    const target = await open();
    button(target, 'Set Up…').click(); await settle();
    fill(field(target, 'custom-llm-base-url'), ' http://localhost:11434/v1 ');
    fill(field(target, 'custom-llm-api-key'), 'secret');
    fill(field(target, 'custom-llm-display-name'), 'Local Ollama');
    button(dialog(target)!, 'Save & Use').click(); await settle();

    expect(mocks.invoke).toHaveBeenCalledWith('set_custom_llm_endpoint', {
      baseUrl: 'http://localhost:11434/v1', displayName: 'Local Ollama',
    });
    expect(mocks.invoke).toHaveBeenCalledWith('save_api_key', { provider: 'custom_llm', apiKey: 'secret' });
    expect(mocks.invoke).toHaveBeenCalledWith('set_llm_processor', { processor: 'custom_api' });
    const order = mocks.invoke.mock.calls.map(([name]) => name);
    expect(order.indexOf('set_custom_llm_endpoint')).toBeLessThan(order.indexOf('save_api_key'));
    expect(order.indexOf('save_api_key')).toBeLessThan(order.indexOf('set_llm_processor'));
    expect(dialog(target)).toBeNull();
    expect(successToast(target)).toContain('Custom endpoint saved');
    expect(inUse(target)[0].textContent).toContain('Local Ollama');
    expect(rowsOf(group(target, 'Local Ollama'))).toEqual([['Endpoint', 'http://localhost:11434/v1', ['Edit…']]]);
    expect(service(target, 'Local Ollama').querySelector('.row-detail')?.textContent).toBe('http://localhost:11434/v1');
  });

  it('leaves the stored key alone and sends no display name when none is typed', async () => {
    backend({ processors: [P.gemini(), P.custom(false)] });
    const target = await open();
    button(target, 'Set Up…').click(); await settle();
    fill(field(target, 'custom-llm-base-url'), 'http://localhost:11434/v1');
    button(dialog(target)!, 'Save & Use').click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('set_custom_llm_endpoint', {
      baseUrl: 'http://localhost:11434/v1', displayName: null,
    });
    expect(called('save_api_key')).toHaveLength(0);
  });

  it('submits with Enter in a field', async () => {
    backend({ processors: [P.gemini(), P.custom(false)] });
    const target = await open();
    button(target, 'Set Up…').click(); await settle();
    fill(field(target, 'custom-llm-base-url'), 'http://localhost:11434/v1');
    dialog(target)!.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    await settle();
    expect(called('set_custom_llm_endpoint')).toHaveLength(1);
  });

  it('keeps the sheet open and says why when the endpoint cannot be saved', async () => {
    failing('set_custom_llm_endpoint', new Error('disk full'), {
      processors: [P.gemini(), P.custom(false)], config: { llm_processor: 'gemini' },
    });
    const target = await open();
    button(target, 'Set Up…').click(); await settle();
    fill(field(target, 'custom-llm-base-url'), 'http://localhost:11434/v1');
    button(dialog(target)!, 'Save & Use').click(); await settle();
    expect(errorToast(target)).toContain('Failed to save custom endpoint');
    expect(errorToast(target)).toContain('disk full');
    expect(dialog(target)).not.toBeNull();
    expect(called('set_llm_processor')).toHaveLength(0);
    expect(inUse(target)[0].textContent).toContain('Gemini CLI');
  });

  it('takes the failure with it when the sheet is cancelled', async () => {
    failing('set_custom_llm_endpoint', new Error('disk full'), {
      processors: [P.gemini(), P.custom(false)],
    });
    const target = await open();
    button(target, 'Set Up…').click(); await settle();
    fill(field(target, 'custom-llm-base-url'), 'http://localhost:11434/v1');
    button(dialog(target)!, 'Save & Use').click(); await settle();
    button(dialog(target)!, 'Cancel').click(); await settle();
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('edits a saved endpoint from the in-use group, starting from what is saved', async () => {
    backend({
      processors: [P.gemini(), P.custom(true, 'Local Ollama')],
      config: {
        llm_processor: 'custom_api',
        http_llm_config: { custom_base_url: 'http://localhost:11434/v1', custom_display_name: 'Local Ollama' },
      },
    });
    const target = await open();
    button(group(target, 'Local Ollama'), 'Edit…').click(); await settle();
    expect(dialogTitle(target)).toBe('Custom Endpoint');
    expect(field(target, 'custom-llm-base-url').value).toBe('http://localhost:11434/v1');
    expect(field(target, 'custom-llm-display-name').value).toBe('Local Ollama');
    // The stored key is never read back.
    expect(field(target, 'custom-llm-api-key').value).toBe('');
  });

  it('shows nothing of a half-typed address on the page, and forgets it on Cancel', async () => {
    backend({
      processors: [P.gemini(), P.custom(true, 'Local Ollama')],
      config: {
        llm_processor: 'custom_api',
        http_llm_config: { custom_base_url: 'http://localhost:11434/v1', custom_display_name: 'Local Ollama' },
      },
    });
    const target = await open();
    button(group(target, 'Local Ollama'), 'Edit…').click(); await settle();
    fill(field(target, 'custom-llm-base-url'), 'http://typing.test');
    // The rows behind the sheet still show the saved address.
    expect(service(target, 'Local Ollama').querySelector('.row-detail')?.textContent).toBe('http://localhost:11434/v1');
    expect(rowsOf(group(target, 'Local Ollama'))[0][1]).toBe('http://localhost:11434/v1');
    button(dialog(target)!, 'Cancel').click(); await settle();
    expect(called('set_custom_llm_endpoint')).toHaveLength(0);

    button(group(target, 'Local Ollama'), 'Edit…').click(); await settle();
    expect(field(target, 'custom-llm-base-url').value).toBe('http://localhost:11434/v1');
  });

  it('does not open by itself when the custom endpoint is the processor in use', async () => {
    backend({
      processors: [P.gemini(), P.custom(true, 'Local Ollama')],
      config: {
        llm_processor: 'custom_api',
        http_llm_config: { custom_base_url: 'http://localhost:11434/v1', custom_display_name: null },
      },
    });
    const target = await open();
    expect(dialog(target)).toBeNull();
  });
});

describe('AI Processing pane: voice commands', () => {
  it('lists the five commands with what to say, each leading to its prompt', async () => {
    const target = await open();
    expect(groupTitles(target)).toEqual(['Service', 'Model', 'Voice Commands']);
    expect(rowsOf(group(target, 'Voice Commands')).map(([label, detail]) => [label, detail])).toEqual([
      ['Clean Up', 'Every transcription without a command'],
      ['Shorten', 'Say “shorten: …”'],
      ['Change Tone', 'Say “make it formal: …” or “make it casual: …”'],
      ['Reply', 'Say “reply to: …”'],
      ['Translate', 'Say “translate to Japanese: …”'],
    ]);
    for (const row of group(target, 'Voice Commands').querySelectorAll('.row')) {
      expect(row.querySelector('.row-main')?.tagName).toBe('BUTTON');
      // The chevron is part of the button, so the whole width of the row opens the prompt.
      expect(row.querySelector('button.row-main .row-accessory svg')?.getAttribute('class'))
        .toContain('lucide-chevron-right');
      expect(row.querySelector('.row-trailing')).toBeNull();
    }
  });

  it('opens a voice command from the mark or the chevron at the end of its row', async () => {
    backend({
      prompts: VOICE_COMMANDS.map((entry) => prompt(entry.prompt, { is_override: entry.prompt === 'shorten' })),
    });
    const target = await open();
    const user = userEvent.setup();
    await user.click(voiceRow(target, 'Shorten').querySelector('.mark')!);
    await settle();
    expect(target.querySelector('h1')?.textContent).toBe('Shorten');
    backButton(target).click(); await settle();
    await user.click(voiceRow(target, 'Reply').querySelector('.chevron')!);
    await settle();
    expect(target.querySelector('h1')?.textContent).toBe('Reply');
  });

  it('reads the mark as part of the name of the row, so assistive technology hears it too', async () => {
    backend({
      prompts: VOICE_COMMANDS.map((entry) => prompt(entry.prompt, { is_override: entry.prompt === 'shorten' })),
    });
    setDraft('translate', 'work in progress');
    const target = await open();
    expect(voiceRow(target, 'Shorten').querySelector('.row-main')?.textContent).toContain('Edited');
    expect(voiceRow(target, 'Translate').querySelector('.row-main')?.textContent).toContain('Unsaved');
    expect(voiceRow(target, 'Reply').querySelector('.row-main')?.textContent).not.toMatch(/Edited|Unsaved/);
  });

  it('marks a prompt that has been edited, and a draft over that', async () => {
    backend({
      prompts: VOICE_COMMANDS.map((command) => prompt(command.prompt, {
        is_override: command.prompt === 'shorten' || command.prompt === 'change_tone',
      })),
    });
    setDraft('change_tone', 'work in progress');
    setDraft('translate', 'another draft');
    const target = await open();
    expect(marksOf(target)).toEqual([
      ['Clean Up', ''],
      ['Shorten', 'Edited'],
      ['Change Tone', 'Unsaved'],
      ['Reply', ''],
      ['Translate', 'Unsaved'],
    ]);
  });

  it('opens a voice command and keeps its draft after going back', async () => {
    const target = await open();
    button(target, 'Shorten').click(); await settle();
    expect(target.querySelector('h1')?.textContent).toBe('Shorten');
    fill(editor(target), 'work in progress'); await settle();
    backButton(target).click(); await settle();
    expect(button(target, 'Shorten').closest('.row')?.textContent).toContain('Unsaved');
    button(target, 'Shorten').click(); await settle();
    expect(editor(target).value).toBe('work in progress');
  });

  it('shows only the editor while a prompt is open, and the pane again when it is closed', async () => {
    const target = await open();
    await openEditor(target, 'Reply');
    expect(target.querySelectorAll('h1')).toHaveLength(1);
    expect(target.querySelector('h1')?.textContent).toBe('Reply');
    expect(target.querySelector('.group')).toBeNull();
    expect(backButton(target)).not.toBeNull();
    backButton(target).click(); await settle();
    expect(target.querySelectorAll('h1')).toHaveLength(1);
    expect(target.querySelector('h1')?.textContent).toBe('AI Processing');
    expect(backButton(target)).toBeNull();
  });

  it('puts the focus back on the row of the command that was opened when its editor is closed', async () => {
    const target = await open();
    await openEditor(target, 'Reply');
    backButton(target).click(); await settle();
    expect(document.activeElement).toBe(button(target, 'Reply'));
    expect(document.activeElement?.classList.contains('row-main')).toBe(true);

    // Whichever command it was, not just the first row.
    await openEditor(target, 'Translate');
    backButton(target).click(); await settle();
    expect(document.activeElement).toBe(button(target, 'Translate'));
  });

  it('puts the focus back on the row when the editor is closed from the keyboard', async () => {
    const target = await open();
    await openEditor(target, 'Shorten');
    backButton(target).focus();
    await userEvent.setup().keyboard('{Enter}');
    await settle();
    expect(document.activeElement).toBe(button(target, 'Shorten'));
  });

  it('keeps what the pane knew about the processors while a prompt is open', async () => {
    const target = await open();
    const before = called('get_llm_processors').length;
    await openEditor(target, 'Shorten');
    backButton(target).click(); await settle();
    expect(called('get_llm_processors')).toHaveLength(before);
    expect(inUse(target)[0].textContent).toContain('Gemini CLI');
  });

  it('shows the Edited mark of a prompt that was saved, once back', async () => {
    const target = await open();
    await openEditor(target, 'Reply');
    fill(editor(target), 'edited'); await settle();
    button(target, 'Save').click(); await settle();
    backButton(target).click(); await settle();
    expect(marksOf(target).find(([label]) => label === 'Reply')).toEqual(['Reply', 'Edited']);
    expect(hasDraft('generate_reply')).toBe(false);
  });

  it('takes the Edited mark away from a prompt that was restored, once back', async () => {
    backend({
      prompts: VOICE_COMMANDS.map((command) => prompt(command.prompt, { is_override: command.prompt === 'shorten' })),
    });
    const target = await open();
    expect(marksOf(target).find(([label]) => label === 'Shorten')).toEqual(['Shorten', 'Edited']);
    await openEditor(target, 'Shorten');
    button(target, 'Restore Default').click(); await settle();
    backButton(target).click(); await settle();
    expect(marksOf(target).find(([label]) => label === 'Shorten')).toEqual(['Shorten', '']);
  });

  it('keeps a draft when the pane is closed and opened again', async () => {
    const target = await open();
    await openEditor(target, 'Shorten');
    fill(editor(target), 'work in progress'); await settle();
    await unmountAll();

    const again = await open();
    expect(marksOf(again).find(([label]) => label === 'Shorten')).toEqual(['Shorten', 'Unsaved']);
    await openEditor(again, 'Shorten');
    expect(editor(again).value).toBe('work in progress');
  });

  it('still lists the five commands when the prompts cannot be loaded', async () => {
    failing('get_prompts', new Error('unavailable'));
    const target = await open();
    expect(errorToast(target)).toContain('Failed to load prompts');
    expect(marksOf(target).map(([label]) => label))
      .toEqual(['Clean Up', 'Shorten', 'Change Tone', 'Reply', 'Translate']);
    expect(marksOf(target).map(([, mark]) => mark)).toEqual(['', '', '', '', '']);
    // The drafts do not depend on the load.
    setDraft('shorten', 'work in progress');
    flushSync();
    expect(marksOf(target)[1]).toEqual(['Shorten', 'Unsaved']);
  });

  it('keeps a failure the pane is showing when it reads the prompts again on coming back', async () => {
    failing('get_config', new Error('unavailable'));
    const target = await open();
    expect(errorToast(target)).toContain('Failed to load config');
    await openEditor(target, 'Shorten');
    backButton(target).click(); await settle();
    // Reading the prompts is not the user's next action, so it does not dismiss what they have not read.
    expect(errorToast(target)).toContain('Failed to load config');
  });

  it('says so when the prompts cannot be loaded on coming back', async () => {
    const target = await open();
    await openEditor(target, 'Shorten');
    failing('get_prompts', new Error('unavailable'));
    backButton(target).click(); await settle();
    expect(errorToast(target)).toContain('Failed to load prompts');
  });
});

describe('AI Processing pane: prompt editor', () => {
  it('is titled by the command, and says what the prompt does and when it is used', async () => {
    const target = await open();
    await openEditor(target, 'Shorten');
    expect(target.querySelector('h1')?.textContent).toBe('Shorten');
    expect(target.querySelector('.editor-intro')?.textContent)
      .toBe('About shorten. Used when you start with “shorten:”.');
    await unmountAll();

    const again = await open();
    await openEditor(again, 'Clean Up');
    expect(again.querySelector('.editor-intro')?.textContent)
      .toBe('About post_process. Used for every transcription without a command.');
  });

  it('puts the stored prompt in an editor that is named and not spell-checked', async () => {
    const target = await open();
    await openEditor(target, 'Shorten');
    expect(editor(target).value).toBe('content of shorten');
    expect(editor(target).getAttribute('aria-label')).toBe('Shorten prompt');
    expect(editor(target).getAttribute('spellcheck')).toBe('false');
    expect(editor(target).closest('.pane')).not.toBeNull();
  });

  it('has no Required line for a prompt that requires nothing', async () => {
    const target = await open();
    await openEditor(target, 'Shorten');
    expect(target.querySelector('.editor-required')).toBeNull();
    expect(target.querySelector('.editor-missing')?.textContent).toBe('');
  });

  it('shows the placeholders the prompt requires', async () => {
    backend({
      prompts: VOICE_COMMANDS.map((command) => prompt(command.prompt, {
        required_placeholders: command.prompt === 'change_tone' ? ['{tone}', '{text}'] : ['{text}'],
        content: '{tone} {text}',
      })),
    });
    const target = await open();
    await openEditor(target, 'Change Tone');
    expect(target.querySelector('.editor-required span')?.textContent).toBe('Required:');
    expect([...target.querySelectorAll('.editor-required code')].map(ph => ph.textContent))
      .toEqual(['{tone}', '{text}']);
    expect(target.querySelector('.editor-missing')?.textContent).toBe('');
    expect(target.querySelector('.editor-required .is-missing')).toBeNull();
  });

  it('marks the placeholders the text lacks, lists them, and still allows saving', async () => {
    backend({
      prompts: VOICE_COMMANDS.map((command) => prompt(command.prompt, {
        required_placeholders: ['{tone}', '{text}'],
        content: '{tone} {text}',
      })),
    });
    const target = await open();
    await openEditor(target, 'Change Tone');
    fill(editor(target), 'Only {text} is left'); await settle();
    expect([...target.querySelectorAll('.editor-required .is-missing')].map(ph => ph.textContent)).toEqual(['{tone}']);
    expect(target.querySelector('.editor-missing')?.textContent)
      .toBe("Missing {tone}. The tone you ask for won't be sent to the model.");

    fill(editor(target), 'Neither'); await settle();
    expect([...target.querySelectorAll('.editor-required .is-missing')].map(ph => ph.textContent))
      .toEqual(['{tone}', '{text}']);
    expect(target.querySelector('.editor-missing')?.textContent)
      .toBe("Missing {tone}, {text}. Your transcription won't be inserted into the prompt. The tone you ask for won't be sent to the model.");
    // A prompt without them is a mistake the user may still want to keep.
    expect(button(target, 'Save').disabled).toBe(false);

    fill(editor(target), '{tone} {text}'); await settle();
    expect(target.querySelector('.editor-required .is-missing')).toBeNull();
    expect(target.querySelector('.editor-missing')?.textContent).toBe('');
  });

  /** What each prompt requires, as the backend lists it. */
  const REQUIRED: Record<string, string[]> = {
    post_process: ['{dictionary_terms}', '{raw_text}'],
    shorten: ['{text}'],
    change_tone: ['{tone}', '{text}'],
    generate_reply: ['{context}'],
    translate: ['{language}', '{text}'],
  };

  /** What the editor of the command called `title` says is missing once `text` replaces the prompt. */
  async function missingNoteAfterTyping(title: string, text: string) {
    backend({
      prompts: VOICE_COMMANDS.map((command) => prompt(command.prompt, {
        required_placeholders: REQUIRED[command.prompt],
        content: REQUIRED[command.prompt].join(' '),
      })),
    });
    const target = await open();
    await openEditor(target, title);
    fill(editor(target), text); await settle();
    return target.querySelector('.editor-missing')?.textContent;
  }

  // The backend never refuses a prompt that lacks a placeholder; it leaves that value out.
  // So each warning says what is left out, and only the transcription is "not inserted".
  it('says the transcription is not inserted when Shorten has no {text}', async () => {
    expect(await missingNoteAfterTyping('Shorten', 'Make it shorter'))
      .toBe("Missing {text}. Your transcription won't be inserted into the prompt.");
  });

  it('says the dictionary terms are not sent, not that the transcription is lost, when Clean Up has no {dictionary_terms}', async () => {
    expect(await missingNoteAfterTyping('Clean Up', 'Fix this: {raw_text}'))
      .toBe("Missing {dictionary_terms}. Your dictionary terms won't be sent to the model.");
  });

  it('says the tone is not sent, not that the transcription is lost, when Change Tone has no {tone}', async () => {
    expect(await missingNoteAfterTyping('Change Tone', 'Rewrite this: {text}'))
      .toBe("Missing {tone}. The tone you ask for won't be sent to the model.");
  });

  it('says the language is not sent, not that the transcription is lost, when Translate has no {language}', async () => {
    expect(await missingNoteAfterTyping('Translate', 'Translate this: {text}'))
      .toBe("Missing {language}. The language you ask for won't be sent to the model.");
  });

  it('names both and says what each leaves out when Change Tone has neither placeholder', async () => {
    expect(await missingNoteAfterTyping('Change Tone', 'Rewrite this'))
      .toBe("Missing {tone}, {text}. Your transcription won't be inserted into the prompt. The tone you ask for won't be sent to the model.");
  });

  it('names both and says what each leaves out when Clean Up has neither placeholder', async () => {
    expect(await missingNoteAfterTyping('Clean Up', 'Fix this'))
      .toBe("Missing {dictionary_terms}, {raw_text}. Your transcription won't be inserted into the prompt. Your dictionary terms won't be sent to the model.");
  });

  it.each([
    ['Clean Up', '{raw_text}', 'Terms: {dictionary_terms}'],
    ['Reply', '{context}', 'Write a reply'],
  ])('says the transcription is not inserted when %s has no %s', async (title, name, text) => {
    expect(await missingNoteAfterTyping(title, text))
      .toBe(`Missing ${name}. Your transcription won't be inserted into the prompt.`);
  });

  it('keeps a generic line for a placeholder it has no wording for', async () => {
    backend({
      prompts: VOICE_COMMANDS.map((command) => prompt(command.prompt, {
        required_placeholders: ['{future}'],
        content: '{future}',
      })),
    });
    const target = await open();
    await openEditor(target, 'Shorten');
    fill(editor(target), 'no placeholder'); await settle();
    expect(target.querySelector('.editor-missing')?.textContent)
      .toBe("Missing {future}. It won't be sent to the model.");
  });

  it('announces a placeholder that goes missing through a status region that is there from the start', async () => {
    backend({
      prompts: VOICE_COMMANDS.map((command) => prompt(command.prompt, {
        required_placeholders: ['{text}'],
        content: 'Shorten: {text}',
      })),
    });
    const target = await open();
    await openEditor(target, 'Shorten');
    // A region that appears together with its text is not announced, so it has to be there while empty.
    const note = target.querySelector('.editor-missing')!;
    expect(note.getAttribute('role')).toBe('status');
    expect(note.textContent).toBe('');
    fill(editor(target), 'no placeholder'); await settle();
    expect(target.querySelector('.editor-missing')).toBe(note);
    expect(note.textContent).toContain('Missing {text}.');
  });

  it('offers Save only for a change that is not blank', async () => {
    const target = await open();
    await openEditor(target, 'Shorten');
    const save = button(target, 'Save');
    expect(save.classList.contains('btn-primary')).toBe(true);
    expect(save.disabled).toBe(true);
    fill(editor(target), 'edited'); await settle();
    expect(save.disabled).toBe(false);
    fill(editor(target), '  \n '); await settle();
    expect(save.disabled).toBe(true);
    // Typing the stored text back is no change, and leaves no draft behind.
    fill(editor(target), 'edited'); await settle();
    expect(hasDraft('shorten')).toBe(true);
    fill(editor(target), 'content of shorten'); await settle();
    expect(save.disabled).toBe(true);
    expect(hasDraft('shorten')).toBe(false);
  });

  it('offers Restore Default only for a prompt that has an override', async () => {
    backend({
      prompts: VOICE_COMMANDS.map((command) => prompt(command.prompt, { is_override: command.prompt === 'translate' })),
    });
    const target = await open();
    await openEditor(target, 'Shorten');
    expect(button(target, 'Restore Default').disabled).toBe(true);
    backButton(target).click(); await settle();
    await openEditor(target, 'Translate');
    expect(button(target, 'Restore Default').disabled).toBe(false);
  });

  it('saves the edit, forgets the draft, shows what is stored, and confirms with a toast', async () => {
    const target = await open();
    await openEditor(target, 'Shorten');
    fill(editor(target), 'edited'); await settle();
    expect(hasDraft('shorten')).toBe(true);
    button(target, 'Save').click(); await settle();

    expect(mocks.invoke).toHaveBeenCalledWith('set_prompt', { params: { name: 'shorten', content: 'edited' } });
    // The editor reads the prompts back: what it shows is what is stored.
    expect(called('get_prompts').length).toBeGreaterThan(1);
    expect(hasDraft('shorten')).toBe(false);
    expect(editor(target).value).toBe('edited');
    expect(successToast(target)).toBe('Saved');
    expect(button(target, 'Save').disabled).toBe(true);
    expect(button(target, 'Restore Default').disabled).toBe(false);
  });

  it('keeps as a draft what was typed while the save was still going', async () => {
    const target = await open();
    await openEditor(target, 'Shorten');
    fill(editor(target), 'edited'); await settle();
    const release = holding('set_prompt');
    button(target, 'Save').click(); await settle();
    fill(editor(target), 'edited, and a little more'); await settle();
    release(); await settle();
    // The save went through with the text that was there when it started, so the rest is still unsaved.
    expect(hasDraft('shorten')).toBe(true);
    expect(getDraft('shorten')).toBe('edited, and a little more');
    expect(editor(target).value).toBe('edited, and a little more');
    expect(successToast(target)).toBe('Saved');
    expect(button(target, 'Save').disabled).toBe(false);
  });

  it('lets the Saved toast go away by itself after two seconds', async () => {
    vi.useFakeTimers();
    const target = await open();
    await openEditor(target, 'Shorten');
    fill(editor(target), 'edited'); await settle();
    button(target, 'Save').click(); await settle();
    expect(successToast(target)).toBe('Saved');
    vi.advanceTimersByTime(1900); await settle();
    expect(successToast(target)).toBe('Saved');
    vi.advanceTimersByTime(200); await settle();
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('restores the default prompt, forgets the draft, and confirms with a toast', async () => {
    backend({
      prompts: VOICE_COMMANDS.map((command) => prompt(command.prompt, {
        is_override: true, content: 'custom',
      })),
    });
    const target = await open();
    await openEditor(target, 'Shorten');
    fill(editor(target), 'edited on top'); await settle();
    button(target, 'Restore Default').click(); await settle();

    expect(mocks.invoke).toHaveBeenCalledWith('reset_prompt', { params: { name: 'shorten' } });
    expect(hasDraft('shorten')).toBe(false);
    expect(editor(target).value).toBe('default of shorten');
    expect(successToast(target)).toBe('Restored the default prompt');
    expect(button(target, 'Restore Default').disabled).toBe(true);
  });

  it('keeps as a draft what was typed while the default was being restored', async () => {
    backend({
      prompts: VOICE_COMMANDS.map((command) => prompt(command.prompt, {
        is_override: true, content: 'custom',
      })),
    });
    const target = await open();
    await openEditor(target, 'Shorten');
    fill(editor(target), 'edited on top'); await settle();
    const release = holding('reset_prompt');
    button(target, 'Restore Default').click(); await settle();
    fill(editor(target), 'typed meanwhile'); await settle();
    release(); await settle();
    // The restore replaced the text that was there when it started, so what came after is still unsaved.
    expect(hasDraft('shorten')).toBe(true);
    expect(getDraft('shorten')).toBe('typed meanwhile');
    expect(editor(target).value).toBe('typed meanwhile');
    expect(successToast(target)).toBe('Restored the default prompt');
    expect(button(target, 'Save').disabled).toBe(false);
  });

  it('leaves no draft behind when what was typed while the default was being restored is the default itself', async () => {
    backend({
      prompts: VOICE_COMMANDS.map((command) => prompt(command.prompt, {
        is_override: true, content: 'custom',
      })),
    });
    const target = await open();
    await openEditor(target, 'Shorten');
    fill(editor(target), 'edited on top'); await settle();
    const release = holding('reset_prompt');
    button(target, 'Restore Default').click(); await settle();
    fill(editor(target), 'default of shorten'); await settle();
    release(); await settle();
    // Nothing differs from what is stored now, so there is nothing to call "Unsaved".
    expect(hasDraft('shorten')).toBe(false);
    expect(editor(target).value).toBe('default of shorten');
    expect(button(target, 'Save').disabled).toBe(true);
  });

  it('keeps the draft and says why when the prompt cannot be saved', async () => {
    failing('set_prompt', new Error('disk full'));
    const target = await open();
    await openEditor(target, 'Shorten');
    fill(editor(target), 'edited'); await settle();
    button(target, 'Save').click(); await settle();
    expect(errorToast(target)).toContain('Failed to save');
    expect(errorToast(target)).toContain('disk full');
    expect(target.querySelector('.toast-success')).toBeNull();
    expect(hasDraft('shorten')).toBe(true);
    expect(editor(target).value).toBe('edited');
    expect(button(target, 'Save').disabled).toBe(false);
  });

  it('keeps the draft and says why when the default cannot be restored', async () => {
    failing('reset_prompt', new Error('disk full'), {
      prompts: VOICE_COMMANDS.map((command) => prompt(command.prompt, { is_override: true })),
    });
    const target = await open();
    await openEditor(target, 'Shorten');
    fill(editor(target), 'edited'); await settle();
    button(target, 'Restore Default').click(); await settle();
    expect(errorToast(target)).toContain('Failed to restore');
    expect(hasDraft('shorten')).toBe(true);
    expect(editor(target).value).toBe('edited');
  });

  it('refuses a blank prompt with an explanation if a save gets past the disabled button', async () => {
    // The button is disabled for a blank prompt, so this only guards the handler against a click that gets through.
    const target = await open();
    await openEditor(target, 'Shorten');
    fill(editor(target), '   '); await settle();
    const save = button(target, 'Save');
    expect(save.disabled).toBe(true);
    save.disabled = false;
    save.click(); await settle();
    expect(errorToast(target)).toContain("Prompt can't be empty. Type something or restore the default.");
    expect(called('set_prompt')).toHaveLength(0);
    expect(hasDraft('shorten')).toBe(true);
  });

  it('shows the toast of a save without a trace of an earlier prompt', async () => {
    const target = await open();
    await openEditor(target, 'Shorten');
    fill(editor(target), 'edited'); await settle();
    button(target, 'Save').click(); await settle();
    expect(successToast(target)).toBe('Saved');
    backButton(target).click(); await settle();
    expect(target.querySelector('.toast')).toBeNull();
    await openEditor(target, 'Reply');
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('says so, and can still be left, when the prompts cannot be loaded', async () => {
    const target = await open();
    failing('get_prompts', new Error('unavailable'));
    await openEditor(target, 'Shorten');
    expect(errorToast(target)).toContain('Failed to load prompts');
    expect(target.querySelector('h1')?.textContent).toBe('Shorten');
    expect(editor(target)).toBeNull();
    backButton(target).click(); await settle();
    expect(target.querySelector('h1')?.textContent).toBe('AI Processing');
  });

  it('shows an empty draft as an empty editor, not as the stored prompt', async () => {
    const target = await open();
    await openEditor(target, 'Shorten');
    fill(editor(target), ''); await settle();
    backButton(target).click(); await settle();
    expect(marksOf(target).find(([label]) => label === 'Shorten')).toEqual(['Shorten', 'Unsaved']);
    await openEditor(target, 'Shorten');
    expect(editor(target).value).toBe('');
  });
});

describe('prompt drafts', () => {
  it('keeps one draft per prompt until it is cleared', () => {
    expect(getDraft('shorten')).toBeUndefined();
    expect(hasDraft('shorten')).toBe(false);
    setDraft('shorten', 'a');
    setDraft('translate', 'b');
    expect(getDraft('shorten')).toBe('a');
    expect(getDraft('translate')).toBe('b');
    expect(hasDraft('shorten')).toBe(true);
    clearDraft('shorten');
    expect(getDraft('shorten')).toBeUndefined();
    expect(hasDraft('shorten')).toBe(false);
    expect(hasDraft('translate')).toBe(true);
  });

  it('counts an empty text as a draft', () => {
    setDraft('shorten', '');
    expect(hasDraft('shorten')).toBe(true);
    expect(getDraft('shorten')).toBe('');
  });

  it('replaces a draft, and forgets them all on a reset', () => {
    setDraft('shorten', 'a');
    setDraft('shorten', 'b');
    expect(getDraft('shorten')).toBe('b');
    setDraft('generate_reply', 'c');
    resetDrafts();
    expect(hasDraft('shorten')).toBe(false);
    expect(hasDraft('generate_reply')).toBe(false);
  });

  it('is not an error to clear a draft that is not there', () => {
    expect(() => clearDraft('translate')).not.toThrow();
  });
});
