<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import { ChevronRight, Cloud, Laptop, Server, Terminal } from 'lucide-svelte';
  import { useLifecycle } from '../../lib/lifecycle';
  import { createStatus } from '../../lib/status.svelte';
  import { safeInvoke as invoke } from '../../lib/tauri';
  import { VOICE_COMMANDS, type PromptName } from '../../lib/voiceCommands';
  import Group from '../ui/Group.svelte';
  import Pane from '../ui/Pane.svelte';
  import Row from '../ui/Row.svelte';
  import ApiKeySheet from './ApiKeySheet.svelte';
  import CustomEndpointSheet, { type EndpointDraft } from './CustomEndpointSheet.svelte';
  import {
    llmProcessorAction,
    llmProcessorDetail,
    llmProcessorDisabled,
    llmProcessorHint,
    orderedLlmProcessors,
    type LlmProcessorAction,
    type LlmProcessorInfo,
  } from './llmProcessors';
  import { hasDraft } from './promptDrafts.svelte';
  import PromptsEditor from './PromptsEditor.svelte';

  /**
   * `home` counts how many times the sidebar item of this pane has been pressed
   * again while it was on show. A change after the pane is mounted, and not the
   * value it is mounted with, takes an open editor back to the list.
   */
  let { home = 0 }: { home?: number } = $props();

  const lifecycle = useLifecycle();
  const status = createStatus(lifecycle);

  const ACTION_LABELS: Record<Exclude<LlmProcessorAction, null>, string> = {
    'add-key': 'Add API Key…',
    'set-up': 'Set Up…',
  };

  /** The part of what `get_prompts` lists that this pane uses; the editor reads the rest itself. */
  interface PromptState {
    name: PromptName;
    is_override: boolean;
  }

  let processors = $state<LlmProcessorInfo[]>([]);
  let currentProcessor = $state('');
  let prompts = $state<PromptState[]>([]);

  // The model the backend has (empty for the default), and what the field says.
  // The field is saved when it is left with a different value than `savedModel`.
  let savedModel = $state('');
  let modelInput = $state('');

  // The prompt whose editor is open. The editor takes the place of the whole
  // pane, so there is one heading on screen and the toolbar's back arrow leads here.
  let editing = $state<PromptName | null>(null);
  // The Voice Commands list, for putting the focus back on a row.
  let voiceCommandList = $state<HTMLElement>();

  // API key sheet: which service it is for and whether it replaces a key. The key
  // itself lives in the sheet, so it is gone when the sheet is.
  let showApiKeySheet = $state(false);
  let keyProcessor = $state<LlmProcessorInfo | null>(null);
  let editingExistingKey = $state(false);

  // Custom endpoint: what is saved, which the rows show. The sheet keeps its
  // own copy of what is typed, so a sheet that is cancelled leaves no
  // half-typed address on the page.
  let showCustomSheet = $state(false);
  let customBaseUrl = $state('');
  let customDisplayName = $state('');

  let orderedProcessors = $derived(orderedLlmProcessors(processors));
  let activeProcessor = $derived(processors.find((processor) => processor.id === currentProcessor));
  // Apple Intelligence ignores the model setting. Nothing is known of the
  // processor in use until the settings have loaded, so nothing is shown before.
  let showModel = $derived(currentProcessor !== '' && currentProcessor !== 'apple_llm');
  let modelLabel = $derived(activeProcessor ? `Model for ${activeProcessor.name}` : 'Model');
  let modelPlaceholder = $derived(
    activeProcessor?.default_model ? `Default: ${activeProcessor.default_model}` : 'Default',
  );

  onMount(() => {
    // Each reports its own failure, and the voice commands do not wait for the
    // processors, whose list costs a probe of each command-line tool.
    void Promise.all([loadProcessors(), loadConfig(), loadPrompts()]);
  });

  let seenHome = untrack(() => home);

  $effect(() => {
    if (home === seenHome) return;
    seenHome = home;
    untrack(goHome);
  });

  async function loadProcessors() {
    try {
      processors = (await invoke<LlmProcessorInfo[]>('get_llm_processors')) ?? [];
    } catch (err) {
      status.fail(`Failed to load LLM processors: ${err}`);
    }
  }

  async function loadConfig() {
    try {
      const config = await invoke<{
        llm_processor: string;
        llm_model: string | null;
        http_llm_config: {
          custom_base_url: string | null;
          custom_display_name: string | null;
        } | null;
      }>('get_config');
      currentProcessor = config.llm_processor.toLowerCase();
      savedModel = config.llm_model || '';
      modelInput = savedModel;
      customBaseUrl = config.http_llm_config?.custom_base_url || '';
      customDisplayName = config.http_llm_config?.custom_display_name || '';
    } catch (err) {
      status.fail(`Failed to load config: ${err}`);
    }
  }

  async function loadPrompts() {
    try {
      prompts = (await invoke<PromptState[]>('get_prompts')) ?? [];
    } catch (err) {
      status.fail(`Failed to load prompts: ${err}`);
    }
  }

  /**
   * Switch the pipeline over, or ask for what the processor still needs first.
   * The check that moves to the new row is the confirmation, so there is no toast.
   */
  async function selectProcessor(processor: LlmProcessorInfo) {
    // The row is disabled for these; this only guards a click that gets past it.
    if (llmProcessorDisabled(processor)) return;

    const action = llmProcessorAction(processor);
    if (action) {
      runAction(processor, action);
      return;
    }

    await status.run('Failed to switch processor', async () => {
      await invoke('set_llm_processor', { processor: processor.id });
      currentProcessor = processor.id;
    });
  }

  function runAction(processor: LlmProcessorInfo, action: Exclude<LlmProcessorAction, null>) {
    if (action === 'add-key') {
      openApiKeySheet(processor, false);
    } else {
      openCustomSheet();
    }
  }

  function openApiKeySheet(processor: LlmProcessorInfo, existing: boolean) {
    keyProcessor = processor;
    editingExistingKey = existing;
    showApiKeySheet = true;
  }

  /** The sheet refuses an empty key before it gets here, and hands the key over as typed. */
  async function saveApiKey(apiKey: string) {
    if (status.busy) return;
    const processor = keyProcessor;
    if (!processor?.api_key_name) return;

    await status.run('Failed to save API key', async () => {
      await invoke('save_api_key', { provider: processor.api_key_name, apiKey });
      await invoke('set_llm_processor', { processor: processor.id });
      currentProcessor = processor.id;
      dismissApiKeySheet();
      await loadProcessors();
      status.confirm('API key saved');
    });
  }

  /** Close the sheet; the key typed into it goes with it. */
  function dismissApiKeySheet() {
    showApiKeySheet = false;
    editingExistingKey = false;
  }

  /** Cancel: the sheet goes, and so does a failure it may have caused. */
  function closeApiKeySheet() {
    dismissApiKeySheet();
    status.reset();
  }

  function openCustomSheet() {
    showCustomSheet = true;
  }

  /** Close the sheet; what was typed in it, the key included, goes with it. */
  function dismissCustomSheet() {
    showCustomSheet = false;
  }

  /** Cancel: the sheet goes, and so does a failure it may have caused. */
  function closeCustomSheet() {
    dismissCustomSheet();
    status.reset();
  }

  async function saveCustomEndpoint(draft: EndpointDraft) {
    if (status.busy) return;
    const baseUrl = draft.baseUrl.trim();
    const displayName = draft.displayName.trim();
    const apiKey = draft.apiKey.trim();

    // The sheet's button is off without a Base URL; this only guards a submit that gets past it.
    if (!baseUrl) {
      status.fail('Base URL is required for custom endpoint');
      return;
    }

    await status.run('Failed to save custom endpoint', async () => {
      await invoke('set_custom_llm_endpoint', { baseUrl, displayName: displayName || null });
      if (apiKey) {
        await invoke('save_api_key', { provider: 'custom_llm', apiKey });
      }
      await invoke('set_llm_processor', { processor: 'custom_api' });
      currentProcessor = 'custom_api';
      customBaseUrl = baseUrl;
      customDisplayName = displayName;
      dismissCustomSheet();
      await loadProcessors();
      status.confirm('Custom endpoint saved');
    });
  }

  async function saveModel() {
    if (status.busy) return;
    const model = modelInput.trim();
    await status.run('Failed to set model', async () => {
      await invoke('set_llm_model', { model });
      savedModel = model;
      status.confirm(model ? 'Model saved' : 'Using the default model');
    });
  }

  function onModelKeydown(event: KeyboardEvent) {
    if (event.key !== 'Enter') return;
    // An input method confirms its candidate with Enter. That press is the input
    // method's own, and saving on it would send half a composition. keyCode 229
    // marks a key it handled, which WebKit can report after the composition has ended.
    if (event.isComposing || event.keyCode === 229) return;
    void saveModel();
  }

  /** Leaving the field saves a changed value. Enter has already saved its own, and an untouched field has nothing to save. */
  function onModelBlur() {
    if (modelInput.trim() !== savedModel) void saveModel();
  }

  /** Back from a prompt: the list is shown again, and the focus goes to the row that opened it. */
  function closeEditor() {
    void focusRow(leaveEditor());
  }

  /**
   * The sidebar item of this pane was pressed again. With an editor open, the
   * list is shown again as on Back, but the focus stays on the item that was
   * pressed. With none open, there is nothing to leave and nothing is read.
   */
  function goHome() {
    if (editing) leaveEditor();
  }

  /** Close the editor, and say which prompt it was. It may have been saved or restored there, so its mark is read again. */
  function leaveEditor(): PromptName | null {
    const opened = editing;
    editing = null;
    void loadPrompts();
    return opened;
  }

  /**
   * The pane was rebuilt, and the row that opened the editor went with the old
   * one, which leaves the focus on the page. Its new button takes the focus, so
   * the keyboard carries on from where it left.
   */
  async function focusRow(prompt: PromptName | null) {
    await tick();
    const index = VOICE_COMMANDS.findIndex((command) => command.prompt === prompt);
    voiceCommandList?.querySelectorAll<HTMLElement>('.row-main')[index]?.focus();
  }

  function processorIcon(processor: LlmProcessorInfo) {
    switch (processor.provider_type) {
      case 'local': return Laptop;
      case 'cli': return Terminal;
      case 'custom': return Server;
      default: return Cloud;
    }
  }

  function actionName(processor: LlmProcessorInfo, action: Exclude<LlmProcessorAction, null>): string {
    return action === 'add-key' ? `Add API Key for ${processor.name}` : `Set up ${processor.name}`;
  }

  /** The mark at the end of a voice command's row: unsaved work shows over a prompt that was edited. */
  function promptMark(prompt: PromptName): string {
    if (hasDraft(prompt)) return 'Unsaved';
    return prompts.some((candidate) => candidate.name === prompt && candidate.is_override) ? 'Edited' : '';
  }
</script>

{#if editing}
  <PromptsEditor name={editing} onback={closeEditor} />
{:else}
  <Pane title="AI Processing" {status}>
    {#if orderedProcessors.length > 0}
      <Group title="Service">
        {#each orderedProcessors as processor (processor.id)}
          {@const action = llmProcessorAction(processor)}
          {@const hint = llmProcessorHint(processor)}
          {#snippet rowAction()}
            {#if action}
              <button
                type="button"
                class="btn btn-small"
                aria-label={actionName(processor, action)}
                onclick={() => runAction(processor, action)}
              >
                {ACTION_LABELS[action]}
              </button>
            {/if}
          {/snippet}
          <!-- Under the row, not in its button: the hint wraps instead of being cut off, and it can be selected. -->
          {#snippet rowHint()}
            <p class="install-hint">{hint}</p>
          {/snippet}
          <!-- A Row reserves room for a snippet it is given, so each goes in only when it has something to show. -->
          <Row
            label={processor.name}
            detail={llmProcessorDetail(processor, customBaseUrl)}
            icon={processorIcon(processor)}
            current={currentProcessor === processor.id}
            disabled={llmProcessorDisabled(processor)}
            onclick={() => selectProcessor(processor)}
            trailing={action ? rowAction : undefined}
            children={hint ? rowHint : undefined}
          />
        {/each}
      </Group>
    {/if}

    <!-- What the processor in use has: a key or an endpoint, under its own name. -->
    {#if activeProcessor?.configured}
      {@const processor = activeProcessor}
      {#if processor.provider_type === 'http'}
        <Group title={processor.name}>
          <Row label="API key" detail="Saved">
            {#snippet trailing()}
              <button
                type="button"
                class="btn btn-small"
                aria-label={`Change API key for ${processor.name}`}
                onclick={() => openApiKeySheet(processor, true)}
              >
                Change…
              </button>
            {/snippet}
          </Row>
        </Group>
      {:else if processor.provider_type === 'custom'}
        <Group title={processor.name}>
          <Row label="Endpoint" detail={customBaseUrl}>
            {#snippet trailing()}
              <button
                type="button"
                class="btn btn-small"
                aria-label={`Edit endpoint of ${processor.name}`}
                onclick={openCustomSheet}
              >
                Edit…
              </button>
            {/snippet}
          </Row>
        </Group>
      {/if}
    {/if}

    {#if showModel}
      <Group title="Model">
        <Row label={modelLabel}>
          {#snippet trailing()}
            <input
              type="text"
              class="model-field"
              aria-label="Model"
              placeholder={modelPlaceholder}
              bind:value={modelInput}
              onkeydown={onModelKeydown}
              onblur={onModelBlur}
              autocomplete="off"
              autocapitalize="off"
              spellcheck="false"
            />
          {/snippet}
        </Row>
      </Group>
    {/if}

    <!-- Wrapped so that a row can be found in it when the pane is rebuilt on coming back from an editor. -->
    <div bind:this={voiceCommandList}>
      <Group title="Voice Commands">
        {#each VOICE_COMMANDS as command (command.prompt)}
          {@const mark = promptMark(command.prompt)}
          <Row label={command.title} detail={command.detail} onclick={() => (editing = command.prompt)}>
            <!-- Not controls, so inside the button: the whole width of the row opens the prompt. -->
            {#snippet accessory()}
              {#if mark}<span class="mark">{mark}</span>{/if}
              <span class="chevron"><ChevronRight size={16} aria-hidden="true" /></span>
            {/snippet}
          </Row>
        {/each}
      </Group>
    </div>

    {#if showApiKeySheet && keyProcessor}
      <ApiKeySheet
        providerName={keyProcessor.name}
        mode={editingExistingKey ? 'change' : 'add'}
        busy={status.busy}
        onsave={saveApiKey}
        onclose={closeApiKeySheet}
      />
    {/if}

    {#if showCustomSheet}
      <CustomEndpointSheet
        idPrefix="custom-llm"
        saved={{ baseUrl: customBaseUrl, displayName: customDisplayName }}
        baseUrlPlaceholder="http://localhost:11434/v1"
        displayNamePlaceholder="Local Ollama"
        busy={status.busy}
        onsave={saveCustomEndpoint}
        onclose={closeCustomSheet}
      />
    {/if}
  </Pane>
{/if}

<style>
  /* The model field sits where a pop-up button would, at the end of its row. */
  .model-field {
    width: 220px;
    height: 22px;
    padding: 0 8px;
    border: 1px solid var(--control-border);
    border-radius: 6px;
    background: var(--field-bg);
    color: var(--text-primary);
    font: inherit;
    font-size: 12px;
    transition: border-color 0.15s ease;
  }

  .model-field::placeholder {
    color: var(--text-secondary);
  }

  /* A soft ring in place of the page-wide outline, like the sheet's fields. */
  .model-field:focus-visible {
    border-color: var(--accent);
    outline: 3px solid color-mix(in srgb, var(--accent) 30%, transparent);
    outline-offset: 0;
  }

  /* The hint is an install command whose package name has no place to break, so it may break anywhere. */
  .install-hint {
    font-size: 11.5px;
    line-height: 1.4;
    color: var(--text-secondary);
    overflow-wrap: anywhere;
    -webkit-user-select: text;
    user-select: text;
  }

  .mark {
    font-size: 12px;
    color: var(--text-secondary);
  }

  .chevron {
    display: inline-flex;
    color: var(--text-secondary);
  }

  @media (prefers-reduced-motion: reduce) {
    .model-field {
      transition: none;
    }
  }
</style>
