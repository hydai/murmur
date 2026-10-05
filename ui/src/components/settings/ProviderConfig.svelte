<script lang="ts">
  import { onMount } from 'svelte';
  import { Check, Cloud, Laptop, Server } from 'lucide-svelte';
  import { useLifecycle } from '../../lib/lifecycle';
  import { createStatus } from '../../lib/status.svelte';
  import { safeInvoke as invoke } from '../../lib/tauri';
  import Group from '../ui/Group.svelte';
  import Pane from '../ui/Pane.svelte';
  import Row from '../ui/Row.svelte';
  import Select from '../ui/Select.svelte';
  import Sheet from '../ui/Sheet.svelte';
  import ApiKeySheet from './ApiKeySheet.svelte';
  import {
    orderedSttProviders,
    sttProviderAction,
    sttProviderDetail,
    sttProviderDisabled,
    type Provider,
    type ProviderAction,
  } from './providerGroups';

  const lifecycle = useLifecycle();
  const status = createStatus(lifecycle);

  /** The providers whose row in the settings group is an API key. */
  const KEYED_PROVIDERS = ['elevenlabs', 'openai', 'groq'];

  const ACTION_LABELS: Record<Exclude<ProviderAction, null>, string> = {
    download: 'Download',
    'add-key': 'Add API Key…',
    'set-up': 'Set Up…',
  };

  let providers = $state<Provider[]>([]);
  let currentProvider = $state('');

  // API key sheet: which service it is for and whether it replaces a key. The key
  // itself lives in the sheet, so it is gone when the sheet is.
  let showApiKeySheet = $state(false);
  let selectedProvider = $state<Provider | null>(null);
  let editingExistingKey = $state(false);

  // Apple STT locale state
  let appleSttLocales = $state<string[]>([]);
  let appleSttLocale = $state('auto');

  // ElevenLabs language state
  let elevenlabsLanguages = $state<[string, string][]>([]);
  let elevenlabsLanguage = $state('auto');

  // Custom STT endpoint: what is saved, and the sheet's own copy of it. The
  // rows show the saved values, so typing in the sheet changes nothing behind
  // it, and a sheet that is cancelled leaves no half-typed address on the page.
  let showCustomSttSheet = $state(false);
  let customSttBaseUrl = $state('');
  let customSttDisplayName = $state('');
  let customSttModel = $state('');
  let customSttLanguage = $state('');
  let draft = $state({ baseUrl: '', apiKey: '', model: '', language: '', displayName: '' });

  let modelDownloadProgress = $state(0);
  let modelDownloading = $state(false);
  let downloadStatus = $state<'' | 'checking' | 'downloading' | 'success' | 'already_installed' | 'error'>('');
  let downloadError = $state('');
  let downloadStartTime = $state(0);

  let orderedProviders = $derived(orderedSttProviders(providers));
  let activeProvider = $derived(providers.find((provider) => provider.id === currentProvider));
  let downloadPercent = $derived(Math.round(modelDownloadProgress * 100));
  let appleSttLocaleOptions = $derived([
    { value: 'auto', label: 'Automatic' },
    ...appleSttLocales.map((locale) => ({ value: locale, label: locale })),
  ]);
  // The backend calls the automatic choice "Auto-detect"; the Apple Speech menu says "Automatic", so this one does too.
  let elevenlabsLanguageOptions = $derived(
    elevenlabsLanguages.map(([value, label]) => ({ value, label: value === 'auto' ? 'Automatic' : label })),
  );

  onMount(() => {
    void initialize().catch((err) => { status.fail(`Failed to initialize providers: ${err}`); });
  });

  async function initialize() {
    try {
      await lifecycle.listen(
        'apple-stt-model-progress',
        (event) => {
          const { progress, finished, error: errorMsg } = event.payload;
          modelDownloadProgress = progress;

          if (!finished && progress > 0) {
            downloadStatus = 'downloading';
          }

          if (finished) {
            modelDownloading = false;
            const elapsed = Date.now() - downloadStartTime;

            if (errorMsg) {
              downloadStatus = 'error';
              downloadError = errorMsg;
            } else if (elapsed < 500 && progress >= 1.0) {
              downloadStatus = 'already_installed';
            } else {
              downloadStatus = 'success';
            }

            lifecycle.timeout(() => {
              downloadStatus = '';
              downloadError = '';
              modelDownloadProgress = 0;
            }, 4000, 'download-status');

            loadProviders();
          }
        }
      );
    } catch (err) {
      // Progress reporting is optional; the page must still load its data.
      console.warn(`Model download progress unavailable: ${err}`);
    }
    if (lifecycle.disposed) return;
    await loadProviders();
    await loadConfig();

    if (currentProvider === 'apple_stt') {
      await loadAppleSttLocales();
    }
    if (currentProvider === 'elevenlabs') {
      await loadElevenLabsLanguages();
    }
  }

  async function loadProviders() {
    try {
      const result = await invoke<Provider[]>('get_stt_providers');
      providers = result;
    } catch (err) {
      status.fail(`Failed to load providers: ${err}`);
    }
  }

  async function loadConfig() {
    try {
      const config = await invoke<{
        stt_provider: string;
        apple_stt_locale: string;
        elevenlabs_language: string;
        http_stt_config: {
          custom_base_url: string | null;
          custom_display_name: string | null;
          custom_model: string | null;
          language: string | null;
        };
      }>('get_config');
      currentProvider = config.stt_provider.toLowerCase();
      appleSttLocale = config.apple_stt_locale || 'auto';
      elevenlabsLanguage = config.elevenlabs_language || 'auto';
      customSttBaseUrl = config.http_stt_config?.custom_base_url || '';
      customSttDisplayName = config.http_stt_config?.custom_display_name || '';
      customSttModel = config.http_stt_config?.custom_model || '';
      customSttLanguage = config.http_stt_config?.language || '';
    } catch (err) {
      status.fail(`Failed to load config: ${err}`);
    }
  }

  /**
   * Switch the pipeline over and load whatever extras the provider needs.
   * The check that moves to the new row is the confirmation, so there is no toast.
   */
  async function activate(provider: Provider) {
    await status.run('Failed to switch provider', async () => {
      await invoke('set_stt_provider', { provider: provider.id });
      currentProvider = provider.id;
      if (provider.id === 'apple_stt') {
        await loadAppleSttLocales();
      }
      if (provider.id === 'elevenlabs') {
        await loadElevenLabsLanguages();
      }
    });
  }

  async function selectProvider(providerId: string) {
    const provider = providers.find(p => p.id === providerId);
    if (!provider) return;

    // An unconfigured provider needs its form before it can be activated.
    if (providerId === 'custom_stt' && !provider.configured) {
      openCustomSttSheet();
      return;
    }
    if (provider.model_status === 'not_installed') {
      status.fail('Download the speech model first.');
      return;
    }
    if (provider.model_status === 'unavailable') {
      status.fail('Apple Speech requires macOS 26 or later.');
      return;
    }
    if (provider.requires_api_key && !provider.configured) {
      openApiKeySheet(provider, false);
      return;
    }

    await activate(provider);
  }

  function runAction(provider: Provider, action: Exclude<ProviderAction, null>) {
    if (action === 'download') {
      void downloadModel(provider);
    } else if (action === 'add-key') {
      openApiKeySheet(provider, false);
    } else {
      openCustomSttSheet();
    }
  }

  function openApiKeySheet(provider: Provider, existing: boolean) {
    selectedProvider = provider;
    editingExistingKey = existing;
    showApiKeySheet = true;
  }

  /** The sheet refuses an empty key before it gets here, and hands the key over as typed. */
  async function saveApiKey(apiKey: string) {
    if (status.busy) return;
    if (!selectedProvider) return;

    const provider = selectedProvider;
    await status.run('Failed to save API key', async () => {
      await invoke('save_api_key', { provider: provider.id, apiKey });
      // Activating shares activate()'s per-provider loads, or the language
      // selector stays hidden until Settings is reopened.
      await invoke('set_stt_provider', { provider: provider.id });
      currentProvider = provider.id;
      if (provider.id === 'elevenlabs') {
        await loadElevenLabsLanguages();
      }
      dismissApiKeySheet();
      await loadProviders();
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

  async function downloadModel(provider: Provider) {
    if (!provider.model_status || provider.model_status !== 'not_installed') return;

    try {
      downloadStatus = 'checking';
      downloadError = '';
      modelDownloading = true;
      modelDownloadProgress = 0;
      downloadStartTime = Date.now();
      status.reset();

      await invoke('download_apple_stt_model', { locale: appleSttLocale });
    } catch (err) {
      downloadStatus = 'error';
      downloadError = `${err}`;
      modelDownloading = false;
      status.fail(`Failed to start model download: ${err}`);
    }
  }

  async function loadElevenLabsLanguages() {
    try {
      elevenlabsLanguages = await invoke<[string, string][]>('get_elevenlabs_languages');
    } catch (err) {
      console.error('Failed to load ElevenLabs languages:', err);
    }
  }

  // The Select waits for the promise before it lines the element up with the
  // page's value, so a language the backend refuses goes back to what was saved.
  async function changeElevenLabsLanguage(language: string): Promise<void> {
    await status.run('Failed to set language', async () => {
      await invoke('set_elevenlabs_language', { language });
      elevenlabsLanguage = language;
    });
  }

  function openCustomSttSheet() {
    draft = {
      baseUrl: customSttBaseUrl,
      apiKey: '',
      model: customSttModel,
      language: customSttLanguage,
      displayName: customSttDisplayName,
    };
    showCustomSttSheet = true;
  }

  /** Close the sheet and forget what was typed in it, the key included. */
  function dismissCustomSttSheet() {
    showCustomSttSheet = false;
    draft = { baseUrl: '', apiKey: '', model: '', language: '', displayName: '' };
  }

  /** Cancel: the sheet goes, and so does a failure it may have caused. */
  function closeCustomSttSheet() {
    dismissCustomSttSheet();
    status.reset();
  }

  async function saveCustomSttEndpoint() {
    if (status.busy) return;
    const baseUrl = draft.baseUrl.trim();
    const displayName = draft.displayName.trim();
    const model = draft.model.trim();
    const language = draft.language.trim();
    const apiKey = draft.apiKey.trim();

    // The sheet's button is off without a Base URL; this only guards a submit that gets past it.
    if (!baseUrl) {
      status.fail('Base URL is required for custom STT endpoint');
      return;
    }

    await status.run('Failed to save custom STT endpoint', async () => {
      await invoke('set_custom_stt_endpoint', {
        baseUrl,
        displayName: displayName || null,
        model: model || null,
        language: language || null,
      });

      if (apiKey) {
        await invoke('save_api_key', {
          provider: 'custom_stt',
          apiKey
        });
      }

      await invoke('set_stt_provider', { provider: 'custom_stt' });
      currentProvider = 'custom_stt';
      customSttBaseUrl = baseUrl;
      customSttDisplayName = displayName;
      customSttModel = model;
      customSttLanguage = language;

      dismissCustomSttSheet();
      await loadProviders();
      status.confirm('Custom endpoint saved');
    });
  }

  async function loadAppleSttLocales() {
    try {
      appleSttLocales = await invoke<string[]>('get_apple_stt_locales');
    } catch (err) {
      console.error('Failed to load Apple STT locales:', err);
    }
  }

  async function changeAppleSttLocale(locale: string): Promise<void> {
    await status.run('Failed to set locale', async () => {
      await invoke('set_apple_stt_locale', { locale });
      appleSttLocale = locale;
      await loadProviders();
    });
  }

  function providerIcon(provider: Provider) {
    if (provider.id === 'custom_stt') return Server;
    return provider.provider_type === 'local' ? Laptop : Cloud;
  }

  function actionName(provider: Provider, action: Exclude<ProviderAction, null>): string {
    switch (action) {
      case 'download': return `Download speech model for ${provider.name}`;
      case 'add-key': return `Add API Key for ${provider.name}`;
      default: return `Set up ${provider.name}`;
    }
  }
</script>

<Pane title="Transcription" {status}>
  {#if orderedProviders.length > 0}
    <Group title="Service">
      {#each orderedProviders as provider (provider.id)}
        {@const action = sttProviderAction(provider)}
        {@const downloadShown = provider.id === 'apple_stt' && (modelDownloading || downloadStatus !== '')}
        {#snippet rowAction()}
          {#if action}
            <button
              type="button"
              class="btn btn-small"
              aria-label={actionName(provider, action)}
              disabled={action === 'download' && modelDownloading}
              onclick={() => runAction(provider, action)}
            >
              {ACTION_LABELS[action]}
            </button>
          {/if}
        {/snippet}
        {#snippet rowDownload()}
          <div class="download">
            {#if downloadStatus === 'checking'}
              <p class="download-text">Checking model availability…</p>
            {:else if downloadStatus === 'downloading'}
              <p class="download-text">Downloading… {downloadPercent}%</p>
            {:else if downloadStatus === 'success' || downloadStatus === 'already_installed'}
              <p class="download-text" role="status">
                <span class="download-done"><Check size={13} aria-hidden="true" /></span>
                {downloadStatus === 'success' ? 'Model downloaded' : 'Model already installed'}
              </p>
            {:else if downloadStatus === 'error'}
              <p class="download-text download-error" role="alert">{downloadError || 'Download failed'}</p>
            {/if}
            {#if modelDownloading}
              <div
                class="progress"
                role="progressbar"
                aria-label="Speech model download"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={downloadPercent}
              >
                <div class="progress-fill" style:width="{downloadPercent}%"></div>
              </div>
            {/if}
          </div>
        {/snippet}
        <!-- A Row reserves room for each snippet it is given, so they go in only when they have something to show. -->
        <Row
          label={provider.name}
          detail={sttProviderDetail(provider, customSttBaseUrl)}
          icon={providerIcon(provider)}
          current={currentProvider === provider.id}
          disabled={sttProviderDisabled(provider)}
          onclick={() => selectProvider(provider.id)}
          trailing={action ? rowAction : undefined}
          children={downloadShown ? rowDownload : undefined}
        />
      {/each}
    </Group>
  {/if}

  <!-- What the service in use needs, under its own name. -->
  {#if activeProvider}
    {@const provider = activeProvider}
    {#if provider.id === 'apple_stt'}
      <Group title={provider.name}>
        {#if appleSttLocales.length > 0}
          <Row label="Language">
            {#snippet trailing()}
              <Select
                value={appleSttLocale}
                options={appleSttLocaleOptions}
                label="Language"
                onchange={changeAppleSttLocale}
              />
            {/snippet}
          </Row>
        {/if}
        <Row label="Speech model">
          {#snippet trailing()}
            {#if provider.model_status === 'installed'}
              <span class="installed">
                <span class="installed-icon"><Check size={15} aria-hidden="true" /></span>
                Installed
              </span>
            {:else}
              <button
                type="button"
                class="btn btn-small"
                aria-label={`Download speech model for ${provider.name}`}
                disabled={modelDownloading || provider.model_status !== 'not_installed'}
                onclick={() => downloadModel(provider)}
              >
                Download
              </button>
            {/if}
          {/snippet}
        </Row>
      </Group>
    {:else if KEYED_PROVIDERS.includes(provider.id)}
      <Group title={provider.name}>
        <Row label="API key" detail={provider.configured ? 'Saved' : undefined}>
          {#snippet trailing()}
            <button
              type="button"
              class="btn btn-small"
              aria-label={provider.configured ? `Change API key for ${provider.name}` : `Add API Key for ${provider.name}`}
              onclick={() => openApiKeySheet(provider, provider.configured)}
            >
              {provider.configured ? 'Change…' : 'Add API Key…'}
            </button>
          {/snippet}
        </Row>
        {#if provider.id === 'elevenlabs' && elevenlabsLanguages.length > 0}
          <Row label="Language">
            {#snippet trailing()}
              <Select
                value={elevenlabsLanguage}
                options={elevenlabsLanguageOptions}
                label="Language"
                onchange={changeElevenLabsLanguage}
              />
            {/snippet}
          </Row>
        {/if}
      </Group>
    {:else if provider.id === 'custom_stt'}
      <Group title={provider.name}>
        <Row label="Endpoint" detail={provider.configured ? customSttBaseUrl : undefined}>
          {#snippet trailing()}
            <button
              type="button"
              class="btn btn-small"
              aria-label={provider.configured ? `Edit endpoint of ${provider.name}` : `Set up ${provider.name}`}
              onclick={openCustomSttSheet}
            >
              {provider.configured ? 'Edit…' : 'Set Up…'}
            </button>
          {/snippet}
        </Row>
      </Group>
    {/if}
  {/if}

  <!-- Inside the pane, where the tokens are. -->
  {#if showApiKeySheet && selectedProvider}
    <ApiKeySheet
      providerName={selectedProvider.name}
      mode={editingExistingKey ? 'change' : 'add'}
      busy={status.busy}
      onsave={saveApiKey}
      onclose={closeApiKeySheet}
    />
  {/if}

  {#if showCustomSttSheet}
    <Sheet title="Custom Endpoint" onclose={closeCustomSttSheet} onsubmit={saveCustomSttEndpoint}>
      <label for="custom-stt-base-url">Base URL</label>
      <input
        id="custom-stt-base-url"
        type="text"
        bind:value={draft.baseUrl}
        placeholder="http://localhost:8080/v1"
        aria-required="true"
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
      />
      <label for="custom-stt-api-key">API Key</label>
      <input
        id="custom-stt-api-key"
        type="password"
        bind:value={draft.apiKey}
        placeholder="Only if the server needs one"
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
      />
      <label for="custom-stt-model">Model</label>
      <input
        id="custom-stt-model"
        type="text"
        bind:value={draft.model}
        placeholder="whisper-1"
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
      />
      <label for="custom-stt-language">Language</label>
      <input
        id="custom-stt-language"
        type="text"
        bind:value={draft.language}
        placeholder="Automatic"
        aria-describedby="custom-stt-language-hint"
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
      />
      <p id="custom-stt-language-hint" class="hint">ISO-639-1 code, e.g. en</p>
      <label for="custom-stt-display-name">Display Name</label>
      <input
        id="custom-stt-display-name"
        type="text"
        bind:value={draft.displayName}
        placeholder="Local Whisper"
        autocomplete="off"
      />
      {#snippet actions()}
        <button type="button" class="btn" onclick={closeCustomSttSheet}>Cancel</button>
        <button type="submit" class="btn btn-primary" disabled={status.busy || !draft.baseUrl.trim()}>
          {status.busy ? 'Saving…' : 'Save & Use'}
        </button>
      {/snippet}
    </Sheet>
  {/if}
</Pane>

<style>
  /* The Apple Speech row's download, under its text. */
  .download {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .download-text {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 11.5px;
    color: var(--text-secondary);
  }

  /* --success is for the check mark only, never for text. */
  .download-done {
    display: inline-flex;
    flex: none;
    color: var(--success);
  }

  .download-error {
    color: var(--danger);
  }

  .progress {
    height: 4px;
    overflow: hidden;
    border-radius: 2px;
    background: var(--fill-selected);
  }

  .progress-fill {
    height: 100%;
    border-radius: 2px;
    background: var(--accent);
    transition: width 0.3s ease;
  }

  .installed {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .installed-icon {
    display: inline-flex;
    flex: none;
    color: var(--success);
  }

  .hint {
    font-size: 11.5px;
    color: var(--text-secondary);
  }

  @media (prefers-reduced-motion: reduce) {
    .progress-fill {
      transition: none;
    }
  }
</style>
