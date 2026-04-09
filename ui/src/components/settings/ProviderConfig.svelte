<script lang="ts">
  import { safeInvoke as invoke } from '../../lib/tauri';
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import PageHeader from './ui/PageHeader.svelte';
  import SectionHeader from './ui/SectionHeader.svelte';
  import StatusRow from './ui/StatusRow.svelte';
  import ActionRow from './ui/ActionRow.svelte';

  interface Provider {
    id: string;
    name: string;
    configured: boolean;
    provider_type: string;
    requires_api_key: boolean;
    model_status: string | null;
  }

  let providers = $state<Provider[]>([]);
  let currentProvider = $state('');
  let showApiKeyModal = $state(false);
  let selectedProvider = $state<Provider | null>(null);
  let apiKeyInput = $state('');
  let showApiKey = $state(false);
  let loading = $state(false);
  let error = $state('');
  let success = $state('');
  let editingExistingKey = $state(false);

  // Apple STT locale state
  let appleSttLocales = $state<string[]>([]);
  let appleSttLocale = $state('auto');

  // ElevenLabs language state
  let elevenlabsLanguages = $state<[string, string][]>([]);
  let elevenlabsLanguage = $state('auto');

  // Custom STT endpoint state
  let showCustomSttSection = $state(false);
  let customSttBaseUrl = $state('');
  let customSttDisplayName = $state('');
  let customSttApiKey = $state('');
  let customSttModel = $state('');
  let customSttLanguage = $state('');
  let modelDownloadProgress = $state(0);
  let modelDownloading = $state(false);
  let downloadStatus = $state<'' | 'checking' | 'downloading' | 'success' | 'already_installed' | 'error'>('');
  let downloadError = $state('');
  let downloadStartTime = $state(0);

  let unlistenProgress: UnlistenFn | null = null;

  // Derived: group providers by type
  let localProviders = $derived(providers.filter(p => p.provider_type === 'local'));
  let cloudProviders = $derived(providers.filter(p => p.provider_type !== 'local' && p.id !== 'custom_stt'));
  let customProvider = $derived(providers.find(p => p.id === 'custom_stt'));

  onMount(async () => {
    await loadProviders();
    await loadConfig();

    if (currentProvider === 'apple_stt') {
      await loadAppleSttLocales();
    }
    if (currentProvider === 'elevenlabs') {
      await loadElevenLabsLanguages();
    }

    unlistenProgress = await listen<{ locale: string; progress: number; finished: boolean; error: string | null }>(
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

          setTimeout(() => {
            downloadStatus = '';
            downloadError = '';
            modelDownloadProgress = 0;
          }, 4000);

          loadProviders();
        }
      }
    );
  });

  onDestroy(() => {
    if (unlistenProgress) {
      unlistenProgress();
    }
  });

  async function loadProviders() {
    try {
      const result = await invoke<Provider[]>('get_stt_providers');
      providers = result;
    } catch (err) {
      error = `Failed to load providers: ${err}`;
      console.error(error);
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
      if (currentProvider === 'custom_stt') {
        showCustomSttSection = true;
      }
    } catch (err) {
      error = `Failed to load config: ${err}`;
      console.error(error);
    }
  }

  async function selectProvider(providerId: string) {
    const provider = providers.find(p => p.id === providerId);
    if (!provider) return;

    if (providerId === 'custom_stt') {
      showCustomSttSection = !showCustomSttSection;
      return;
    }

    if (!provider.requires_api_key) {
      if (provider.model_status === 'not_installed') {
        error = 'Speech model not installed. Click "Download Model" first.';
        return;
      }
      if (provider.model_status === 'unavailable') {
        error = 'This provider requires macOS 26 or later.';
        return;
      }

      try {
        loading = true;
        error = '';
        success = '';

        await invoke('set_stt_provider', { provider: providerId });
        currentProvider = providerId;

        if (providerId === 'apple_stt') {
          await loadAppleSttLocales();
        }
        if (providerId === 'elevenlabs') {
          await loadElevenLabsLanguages();
        }

        success = `Switched to ${provider.name}`;
        setTimeout(() => { success = ''; }, 3000);
      } catch (err) {
        error = `Failed to switch provider: ${err}`;
        console.error(error);
      } finally {
        loading = false;
      }
      return;
    }

    if (!provider.configured) {
      selectedProvider = provider;
      showApiKeyModal = true;
      editingExistingKey = false;
      apiKeyInput = '';
      return;
    }

    try {
      loading = true;
      error = '';
      success = '';

      await invoke('set_stt_provider', { provider: providerId });
      currentProvider = providerId;

      if (providerId === 'elevenlabs') {
        await loadElevenLabsLanguages();
      }

      success = `Switched to ${provider.name}`;
      setTimeout(() => { success = ''; }, 3000);
    } catch (err) {
      error = `Failed to switch provider: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  function editApiKey(provider: Provider) {
    selectedProvider = provider;
    showApiKeyModal = true;
    editingExistingKey = true;
    apiKeyInput = '';
  }

  async function saveApiKey() {
    if (!apiKeyInput.trim()) {
      error = 'API key cannot be empty';
      return;
    }

    if (!selectedProvider) return;

    try {
      loading = true;
      error = '';
      success = '';

      await invoke('save_api_key', {
        provider: selectedProvider.id,
        apiKey: apiKeyInput
      });

      await invoke('set_stt_provider', {
        provider: selectedProvider.id
      });

      currentProvider = selectedProvider.id;
      success = editingExistingKey
        ? `Updated API key for ${selectedProvider.name}`
        : `Configured and activated ${selectedProvider.name}`;

      showApiKeyModal = false;
      await loadProviders();

      setTimeout(() => { success = ''; }, 3000);
    } catch (err) {
      error = `Failed to save API key: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  function closeModal() {
    showApiKeyModal = false;
    apiKeyInput = '';
    showApiKey = false;
    editingExistingKey = false;
    error = '';
  }

  function toggleApiKeyVisibility() {
    showApiKey = !showApiKey;
  }

  function getProviderStatus(provider: Provider): 'green' | 'yellow' | 'red' | 'none' {
    if (provider.model_status === 'unavailable') return 'red';
    if (currentProvider === provider.id) return 'green';
    if (provider.configured || (!provider.requires_api_key && provider.model_status === 'installed')) return 'yellow';
    if (provider.requires_api_key && !provider.configured) return 'red';
    return 'none';
  }

  function getProviderStatusText(provider: Provider): string {
    if (provider.model_status === 'unavailable') return 'macOS 26+';
    if (currentProvider === provider.id) return 'Active';
    if (provider.configured) return 'Configured';
    if (!provider.requires_api_key && provider.model_status === 'installed') return 'Ready';
    if (!provider.requires_api_key && provider.model_status === 'not_installed') return 'Not Installed';
    if (provider.requires_api_key && !provider.configured) return 'Not Configured';
    return 'Available';
  }

  function getProviderValue(provider: Provider): string {
    switch (provider.provider_type) {
      case 'local': return 'on-device';
      case 'streaming': return 'streaming';
      case 'batch': return 'batch';
      default: return provider.provider_type;
    }
  }

  async function downloadModel(provider: Provider) {
    if (!provider.model_status || provider.model_status !== 'not_installed') return;

    try {
      downloadStatus = 'checking';
      downloadError = '';
      modelDownloading = true;
      modelDownloadProgress = 0;
      downloadStartTime = Date.now();
      error = '';

      await invoke('download_apple_stt_model', { locale: appleSttLocale });
    } catch (err) {
      downloadStatus = 'error';
      downloadError = `${err}`;
      modelDownloading = false;
      error = `Failed to start model download: ${err}`;
      console.error(error);
    }
  }

  async function loadElevenLabsLanguages() {
    try {
      elevenlabsLanguages = await invoke<[string, string][]>('get_elevenlabs_languages');
    } catch (err) {
      console.error('Failed to load ElevenLabs languages:', err);
    }
  }

  async function changeElevenLabsLanguage(event: Event) {
    const target = event.target as HTMLSelectElement;
    const language = target.value;
    elevenlabsLanguage = language;

    try {
      await invoke('set_elevenlabs_language', { language });
      const displayName = elevenlabsLanguages.find(([code]) => code === language)?.[1] ?? language;
      success = `Language set to ${displayName}`;
      setTimeout(() => { success = ''; }, 3000);
    } catch (err) {
      error = `Failed to set language: ${err}`;
      console.error(error);
    }
  }

  async function saveCustomSttEndpoint() {
    if (!customSttBaseUrl.trim()) {
      error = 'Base URL is required for custom STT endpoint';
      return;
    }

    try {
      loading = true;
      error = '';
      success = '';

      await invoke('set_custom_stt_endpoint', {
        baseUrl: customSttBaseUrl,
        displayName: customSttDisplayName || null,
        model: customSttModel || null,
        language: customSttLanguage || null,
      });

      if (customSttApiKey.trim()) {
        await invoke('save_api_key', {
          provider: 'custom_stt',
          apiKey: customSttApiKey
        });
      }

      await invoke('set_stt_provider', { provider: 'custom_stt' });
      currentProvider = 'custom_stt';

      await loadProviders();
      success = `Custom STT endpoint activated: ${customSttDisplayName || customSttBaseUrl}`;
      customSttApiKey = '';
      setTimeout(() => { success = ''; }, 3000);
    } catch (err) {
      error = `Failed to save custom STT endpoint: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  async function loadAppleSttLocales() {
    try {
      appleSttLocales = await invoke<string[]>('get_apple_stt_locales');
    } catch (err) {
      console.error('Failed to load Apple STT locales:', err);
    }
  }

  async function changeAppleSttLocale(event: Event) {
    const target = event.target as HTMLSelectElement;
    const locale = target.value;
    appleSttLocale = locale;

    try {
      await invoke('set_apple_stt_locale', { locale });
      await loadProviders();
      success = `Language set to ${locale === 'auto' ? 'Auto-detect' : locale}`;
      setTimeout(() => { success = ''; }, 3000);
    } catch (err) {
      error = `Failed to set locale: ${err}`;
      console.error(error);
    }
  }
</script>

<div class="provider-page">
  <PageHeader title="STT Providers" description="Configure speech-to-text engines for voice input" />

  {#if error}
    <div class="alert alert-error">{error}</div>
  {/if}
  {#if success}
    <div class="alert alert-success">{success}</div>
  {/if}

  <!-- LOCAL ON-DEVICE -->
  {#if localProviders.length > 0}
    <div class="section">
      <SectionHeader label="LOCAL ON-DEVICE" />
      <div class="section-rows">
        {#each localProviders as provider}
          <StatusRow
            label={provider.name}
            value={getProviderValue(provider)}
            status={getProviderStatus(provider)}
            statusText={getProviderStatusText(provider)}
            onclick={() => selectProvider(provider.id)}
          >
            {#snippet children()}
              {#if provider.model_status === 'not_installed'}
                <button class="inline-btn" onclick={(e) => { e.stopPropagation(); downloadModel(provider); }} disabled={modelDownloading}>
                  {modelDownloading ? 'Downloading...' : 'Download'}
                </button>
              {/if}
              {#if provider.requires_api_key && provider.configured}
                <button class="inline-btn" onclick={(e) => { e.stopPropagation(); editApiKey(provider); }}>
                  Edit Key
                </button>
              {/if}
            {/snippet}
          </StatusRow>

          <!-- Download progress inline -->
          {#if provider.id === 'apple_stt' && (modelDownloading || downloadStatus)}
            <div class="download-inline">
              {#if downloadStatus === 'checking'}
                <span class="download-msg">Checking model availability...</span>
              {:else if downloadStatus === 'downloading'}
                <span class="download-msg">Downloading — {Math.round(modelDownloadProgress * 100)}%</span>
              {/if}
              {#if modelDownloading}
                <div class="progress-bar-bg">
                  <div class="progress-bar-fill" style="width: {modelDownloadProgress * 100}%"></div>
                </div>
              {/if}
              {#if downloadStatus === 'success'}
                <span class="download-ok">Model downloaded successfully</span>
              {:else if downloadStatus === 'already_installed'}
                <span class="download-ok">Model already installed</span>
              {:else if downloadStatus === 'error'}
                <span class="download-err">{downloadError || 'Download failed'}</span>
              {/if}
            </div>
          {/if}

          <!-- Locale selector for active Apple STT -->
          {#if currentProvider === provider.id && provider.id === 'apple_stt' && appleSttLocales.length > 0}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="locale-row" onclick={(e) => e.stopPropagation()}>
              <label for="apple-stt-locale">Language</label>
              <select id="apple-stt-locale" value={appleSttLocale} onchange={changeAppleSttLocale}>
                <option value="auto">Auto-detect</option>
                {#each appleSttLocales as locale}
                  <option value={locale}>{locale}</option>
                {/each}
              </select>
            </div>
          {/if}
        {/each}
      </div>
    </div>
  {/if}

  <!-- CLOUD API -->
  {#if cloudProviders.length > 0}
    <div class="section">
      <SectionHeader label="CLOUD API" />
      <div class="section-rows">
        {#each cloudProviders as provider}
          <StatusRow
            label={provider.name}
            value={getProviderValue(provider)}
            status={getProviderStatus(provider)}
            statusText={getProviderStatusText(provider)}
            onclick={() => selectProvider(provider.id)}
          >
            {#snippet children()}
              {#if provider.requires_api_key && provider.configured}
                <button class="inline-btn" onclick={(e) => { e.stopPropagation(); editApiKey(provider); }}>
                  Edit Key
                </button>
              {/if}
            {/snippet}
          </StatusRow>

          <!-- Language selector for active ElevenLabs -->
          {#if currentProvider === provider.id && provider.id === 'elevenlabs' && elevenlabsLanguages.length > 0}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="locale-row" onclick={(e) => e.stopPropagation()}>
              <label for="elevenlabs-language">Language</label>
              <select id="elevenlabs-language" value={elevenlabsLanguage} onchange={changeElevenLabsLanguage}>
                {#each elevenlabsLanguages as [code, name]}
                  <option value={code}>{name}</option>
                {/each}
              </select>
            </div>
          {/if}
        {/each}
      </div>
    </div>
  {/if}

  <!-- CUSTOM ENDPOINT -->
  <div class="section">
    <SectionHeader label="CUSTOM ENDPOINT" />
    <ActionRow
      label="Add custom Whisper-compatible endpoint"
      onclick={() => { showCustomSttSection = !showCustomSttSection; }}
    />

    {#if showCustomSttSection}
      <div class="custom-form">
        <p class="form-desc">Connect to any OpenAI-compatible Whisper endpoint (whisper.cpp, faster-whisper, LocalAI, etc.)</p>
        <div class="form-group">
          <label for="custom-stt-base-url">Base URL</label>
          <input id="custom-stt-base-url" type="text" bind:value={customSttBaseUrl} placeholder="http://localhost:8080/v1" />
        </div>
        <div class="form-group">
          <label for="custom-stt-api-key">API Key <span class="optional">(optional for local)</span></label>
          <input id="custom-stt-api-key" type="password" bind:value={customSttApiKey} placeholder="API key (if required)" />
        </div>
        <div class="form-group">
          <label for="custom-stt-model">Model <span class="optional">(default: whisper-1)</span></label>
          <input id="custom-stt-model" type="text" bind:value={customSttModel} placeholder="whisper-1" />
        </div>
        <div class="form-group">
          <label for="custom-stt-language">Language <span class="optional">(optional, ISO-639-1)</span></label>
          <input id="custom-stt-language" type="text" bind:value={customSttLanguage} placeholder="auto-detect" />
        </div>
        <div class="form-group">
          <label for="custom-stt-display-name">Display Name <span class="optional">(optional)</span></label>
          <input id="custom-stt-display-name" type="text" bind:value={customSttDisplayName} placeholder="e.g., Local Whisper" />
        </div>
        <button class="primary-btn" onclick={saveCustomSttEndpoint} disabled={loading || !customSttBaseUrl.trim()}>
          {loading ? 'Saving...' : 'Save & Activate'}
        </button>
      </div>
    {/if}
  </div>
</div>

<!-- API Key Modal -->
{#if showApiKeyModal}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-overlay" onclick={closeModal} onkeydown={(e) => e.key === 'Escape' && closeModal()} role="presentation">
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" tabindex="-1">
      <h3>{editingExistingKey ? 'Update' : 'Configure'} {selectedProvider?.name}</h3>
      <p>{editingExistingKey ? 'Enter a new API key:' : 'Enter your API key to enable this provider:'}</p>

      <div class="api-key-wrapper">
        <input
          type={showApiKey ? 'text' : 'password'}
          bind:value={apiKeyInput}
          placeholder="API Key"
          class="api-key-input"
          onkeydown={(e) => e.key === 'Enter' && saveApiKey()}
        />
        <button class="visibility-toggle" onclick={toggleApiKeyVisibility} type="button">
          {showApiKey ? '👁️' : '👁️‍🗨️'}
        </button>
      </div>

      <div class="modal-actions">
        <button class="btn-secondary" onclick={closeModal}>Cancel</button>
        <button class="btn-primary" onclick={saveApiKey} disabled={loading}>
          {loading ? 'Saving...' : editingExistingKey ? 'Update Key' : 'Save & Activate'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .provider-page {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  /* Alerts */
  .alert {
    padding: 10px 14px;
    border-radius: 8px;
    font-size: 12px;
  }

  .alert-error {
    background: rgba(239, 68, 68, 0.15);
    border: 1px solid rgba(239, 68, 68, 0.4);
    color: #fca5a5;
  }

  .alert-success {
    background: rgba(34, 197, 94, 0.15);
    border: 1px solid rgba(34, 197, 94, 0.4);
    color: #86efac;
  }

  /* Sections */
  .section {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 100%;
  }

  .section-rows {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  /* Inline buttons (inside StatusRow) */
  .inline-btn {
    padding: 3px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-secondary);
    font-size: 11px;
    cursor: pointer;
    transition: all 0.15s ease;
    white-space: nowrap;
  }

  .inline-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-primary);
  }

  .inline-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Download progress */
  .download-inline {
    padding: 6px 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .download-msg {
    font-size: 11px;
    color: var(--text-muted);
  }

  .download-ok {
    font-size: 11px;
    color: var(--status-green);
  }

  .download-err {
    font-size: 11px;
    color: var(--status-red);
  }

  .progress-bar-bg {
    height: 4px;
    background: var(--border);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-bar-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 2px;
    transition: width 0.3s ease;
  }

  /* Locale selector row */
  .locale-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 12px;
  }

  .locale-row label {
    font-size: 12px;
    color: var(--text-muted);
  }

  .locale-row select {
    padding: 4px 8px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg-card);
    color: var(--text-primary);
    font-size: 12px;
    cursor: pointer;
    outline: none;
  }

  .locale-row select:focus {
    border-color: rgba(168, 85, 247, 0.6);
  }

  /* Custom endpoint form */
  .custom-form {
    padding: 14px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .form-desc {
    margin: 0;
    color: var(--text-muted);
    font-size: 12px;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .form-group label {
    font-size: 12px;
    color: var(--text-secondary);
  }

  .optional {
    color: var(--text-muted);
    font-size: 11px;
  }

  .form-group input {
    width: 100%;
    padding: 8px 12px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--bg-card);
    color: var(--text-primary);
    font-size: 12px;
    outline: none;
    transition: border-color 0.15s ease;
  }

  .form-group input:focus {
    border-color: rgba(168, 85, 247, 0.6);
  }

  .form-group input::placeholder {
    color: var(--text-placeholder);
  }

  .primary-btn {
    width: 100%;
    padding: 8px;
    border-radius: 8px;
    border: none;
    background: var(--accent);
    color: var(--text-primary);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .primary-btn:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .primary-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Modal */
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal {
    background: var(--bg-card);
    padding: 24px;
    border-radius: 16px;
    max-width: 400px;
    width: 90%;
    border: 1px solid var(--border);
  }

  .modal h3 {
    margin: 0 0 8px;
    font-size: 16px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .modal p {
    margin: 0 0 16px;
    color: var(--text-muted);
    font-size: 13px;
  }

  .api-key-wrapper {
    position: relative;
    margin-bottom: 20px;
  }

  .api-key-input {
    width: 100%;
    padding: 10px 12px;
    padding-right: 44px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--bg-primary);
    color: var(--text-primary);
    font-size: 13px;
    outline: none;
  }

  .api-key-input:focus {
    border-color: rgba(168, 85, 247, 0.6);
  }

  .visibility-toggle {
    position: absolute;
    right: 8px;
    top: 50%;
    transform: translateY(-50%);
    background: none;
    border: none;
    cursor: pointer;
    padding: 6px;
    font-size: 14px;
    opacity: 0.6;
  }

  .visibility-toggle:hover {
    opacity: 1;
  }

  .modal-actions {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
  }

  .btn-primary, .btn-secondary {
    padding: 8px 16px;
    border-radius: 8px;
    border: none;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-primary {
    background: var(--accent);
    color: var(--text-primary);
  }

  .btn-primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-secondary {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-primary);
  }

  .btn-secondary:hover {
    background: rgba(255, 255, 255, 0.15);
  }
</style>
