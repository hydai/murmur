<script lang="ts">
  import { safeInvoke as invoke } from '../../lib/tauri';
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';

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

  onMount(async () => {
    await loadProviders();
    await loadConfig();

    // Load language lists for active provider
    if (currentProvider === 'apple_stt') {
      await loadAppleSttLocales();
    }
    if (currentProvider === 'elevenlabs') {
      await loadElevenLabsLanguages();
    }

    // Listen for model download progress events
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
            // Completed almost instantly — model was already installed
            downloadStatus = 'already_installed';
          } else {
            downloadStatus = 'success';
          }

          // Auto-clear status after 4s
          setTimeout(() => {
            downloadStatus = '';
            downloadError = '';
            modelDownloadProgress = 0;
          }, 4000);

          // Reload providers to update model status
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

    // Custom STT: expand config section instead of immediate activation
    if (providerId === 'custom_stt') {
      showCustomSttSection = !showCustomSttSection;
      return;
    }

    // If provider doesn't require API key (local provider), activate directly
    if (!provider.requires_api_key) {
      // Check if model needs downloading first
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

        // Load available locales for Apple STT
        if (providerId === 'apple_stt') {
          await loadAppleSttLocales();
        }
        // Load available languages for ElevenLabs
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

    // Provider is already configured, just switch to it
    try {
      loading = true;
      error = '';
      success = '';

      await invoke('set_stt_provider', { provider: providerId });
      currentProvider = providerId;

      // Load available languages for ElevenLabs
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

      // Save the API key
      await invoke('save_api_key', {
        provider: selectedProvider.id,
        apiKey: apiKeyInput
      });

      // Set as current provider
      await invoke('set_stt_provider', {
        provider: selectedProvider.id
      });

      currentProvider = selectedProvider.id;
      success = editingExistingKey
        ? `Updated API key for ${selectedProvider.name}`
        : `Configured and activated ${selectedProvider.name}`;

      // Close modal and reload providers
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

  function getProviderStatusClass(provider: Provider): string {
    if (provider.model_status === 'unavailable') {
      return 'unavailable';
    }
    if (currentProvider === provider.id) {
      return 'active';
    }
    if (provider.configured) {
      return 'configured';
    }
    return 'not-configured';
  }

  function getProviderTypeLabel(providerType: string): string {
    switch (providerType) {
      case 'streaming': return 'Streaming';
      case 'batch': return 'Batch';
      case 'local': return 'Local (On-Device)';
      default: return providerType;
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
      // Reload providers to get updated model status for new locale
      await loadProviders();
      success = `Language set to ${locale === 'auto' ? 'Auto-detect' : locale}`;
      setTimeout(() => { success = ''; }, 3000);
    } catch (err) {
      error = `Failed to set locale: ${err}`;
      console.error(error);
    }
  }
</script>

<div class="provider-config">
  <h2>STT Provider</h2>

  {#if error}
    <div class="alert alert-error">{error}</div>
  {/if}

  {#if success}
    <div class="alert alert-success">{success}</div>
  {/if}

  <div class="providers-list">
    {#each providers as provider}
      <div
        class="provider-card {getProviderStatusClass(provider)}"
        onclick={() => selectProvider(provider.id)}
        onkeydown={(e) => e.key === 'Enter' && selectProvider(provider.id)}
        role="button"
        tabindex="0"
      >
        <div class="provider-header">
          <h3>{provider.name}</h3>
          <div class="provider-actions">
            {#if provider.requires_api_key && provider.configured}
              <button
                class="edit-key-btn"
                onclick={(e) => { e.stopPropagation(); editApiKey(provider); }}
                title="Edit API Key"
              >
                Edit Key
              </button>
            {/if}
            {#if provider.model_status === 'not_installed'}
              <button
                class="download-btn"
                onclick={(e) => { e.stopPropagation(); downloadModel(provider); }}
                disabled={modelDownloading}
              >
                {modelDownloading ? 'Downloading...' : 'Download Model'}
              </button>
            {/if}
            {#if provider.model_status === 'unavailable'}
              <span class="badge badge-unavailable">macOS 26+</span>
            {:else if currentProvider === provider.id}
              <span class="badge badge-active">Active</span>
            {:else if provider.configured}
              <span class="badge badge-configured">Configured</span>
            {:else if !provider.requires_api_key && provider.model_status === 'installed'}
              <span class="badge badge-configured">Ready</span>
            {:else if provider.requires_api_key}
              <span class="badge badge-not-configured">Not Configured</span>
            {/if}
          </div>
        </div>
        <div class="provider-details">
          <span class="provider-type">{getProviderTypeLabel(provider.provider_type)}</span>
          {#if !provider.requires_api_key}
            <span class="provider-note">No API key required</span>
          {/if}
        </div>

        {#if provider.id === 'apple_stt' && (modelDownloading || downloadStatus)}
          {#if downloadStatus === 'checking'}
            <div class="download-status">Checking model availability...</div>
          {:else if downloadStatus === 'downloading'}
            <div class="download-status">Downloading speech model — {Math.round(modelDownloadProgress * 100)}%</div>
          {/if}
          {#if modelDownloading}
            <div class="progress-bar-container">
              <div class="progress-bar" style="width: {modelDownloadProgress * 100}%"></div>
            </div>
          {/if}
          {#if downloadStatus === 'success'}
            <div class="download-success">Model downloaded successfully</div>
          {:else if downloadStatus === 'already_installed'}
            <div class="download-success">Model already installed</div>
          {:else if downloadStatus === 'error'}
            <div class="download-error">{downloadError || 'Download failed'}</div>
          {/if}
        {/if}

        {#if currentProvider === provider.id && provider.id === 'apple_stt' && appleSttLocales.length > 0}
          <div class="locale-selector" onclick={(e) => e.stopPropagation()}>
            <label for="apple-stt-locale">Language:</label>
            <select
              id="apple-stt-locale"
              value={appleSttLocale}
              onchange={changeAppleSttLocale}
            >
              <option value="auto">Auto-detect</option>
              {#each appleSttLocales as locale}
                <option value={locale}>{locale}</option>
              {/each}
            </select>
          </div>
        {/if}

        {#if currentProvider === provider.id && provider.id === 'elevenlabs' && elevenlabsLanguages.length > 0}
          <div class="locale-selector" onclick={(e) => e.stopPropagation()}>
            <label for="elevenlabs-language">Language:</label>
            <select
              id="elevenlabs-language"
              value={elevenlabsLanguage}
              onchange={changeElevenLabsLanguage}
            >
              {#each elevenlabsLanguages as [code, name]}
                <option value={code}>{name}</option>
              {/each}
            </select>
          </div>
        {/if}
      </div>
    {/each}
  </div>

  {#if showCustomSttSection}
    <div class="custom-endpoint-form">
      <p class="form-description">
        Connect to any OpenAI-compatible Whisper endpoint (whisper.cpp, faster-whisper, LocalAI, etc.)
      </p>
      <div class="form-group">
        <label for="custom-stt-base-url">Base URL</label>
        <input
          id="custom-stt-base-url"
          type="text"
          class="form-input"
          bind:value={customSttBaseUrl}
          placeholder="http://localhost:8080/v1"
        />
      </div>
      <div class="form-group">
        <label for="custom-stt-api-key">API Key <span class="optional">(optional for local)</span></label>
        <input
          id="custom-stt-api-key"
          type="password"
          class="form-input"
          bind:value={customSttApiKey}
          placeholder="API key (if required)"
        />
      </div>
      <div class="form-group">
        <label for="custom-stt-model">Model <span class="optional">(default: whisper-1)</span></label>
        <input
          id="custom-stt-model"
          type="text"
          class="form-input"
          bind:value={customSttModel}
          placeholder="whisper-1"
        />
      </div>
      <div class="form-group">
        <label for="custom-stt-language">Language <span class="optional">(optional, ISO-639-1 e.g. en, zh, ja)</span></label>
        <input
          id="custom-stt-language"
          type="text"
          class="form-input"
          bind:value={customSttLanguage}
          placeholder="auto-detect"
        />
      </div>
      <div class="form-group">
        <label for="custom-stt-display-name">Display Name <span class="optional">(optional)</span></label>
        <input
          id="custom-stt-display-name"
          type="text"
          class="form-input"
          bind:value={customSttDisplayName}
          placeholder="e.g., Local Whisper"
        />
      </div>
      <button
        class="save-custom-btn"
        onclick={saveCustomSttEndpoint}
        disabled={loading || !customSttBaseUrl.trim()}
      >
        {loading ? 'Saving...' : 'Save & Activate'}
      </button>
    </div>
  {/if}

  {#if loading}
    <div class="loading">Processing...</div>
  {/if}
</div>

{#if showApiKeyModal}
  <div class="modal-overlay" onclick={closeModal} onkeydown={(e) => e.key === 'Escape' && closeModal()} role="presentation">
    <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" tabindex="-1">
      <h3>{editingExistingKey ? 'Update' : 'Configure'} {selectedProvider?.name}</h3>
      <p>{editingExistingKey ? 'Enter a new API key:' : 'Enter your API key to enable this provider:'}</p>

      <div class="api-key-input-wrapper">
        <input
          type={showApiKey ? 'text' : 'password'}
          bind:value={apiKeyInput}
          placeholder="API Key"
          class="api-key-input"
          onkeydown={(e) => e.key === 'Enter' && saveApiKey()}
        />
        <button
          class="toggle-visibility-btn"
          onclick={toggleApiKeyVisibility}
          type="button"
          title={showApiKey ? 'Hide API key' : 'Show API key'}
        >
          {showApiKey ? '👁️' : '👁️‍🗨️'}
        </button>
      </div>

      <div class="modal-actions">
        <button class="btn btn-secondary" onclick={closeModal}>Cancel</button>
        <button class="btn btn-primary" onclick={saveApiKey} disabled={loading}>
          {loading ? 'Saving...' : editingExistingKey ? 'Update Key' : 'Save & Activate'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .provider-config {
    padding: 20px;
    max-width: 600px;
  }

  h2 {
    margin-bottom: 20px;
    font-size: 24px;
    font-weight: bold;
    color: #fff;
  }

  .alert {
    padding: 12px 16px;
    margin-bottom: 16px;
    border-radius: 8px;
    font-size: 14px;
  }

  .alert-error {
    background: rgba(239, 68, 68, 0.2);
    border: 1px solid rgba(239, 68, 68, 0.5);
    color: #fca5a5;
  }

  .alert-success {
    background: rgba(34, 197, 94, 0.2);
    border: 1px solid rgba(34, 197, 94, 0.5);
    color: #86efac;
  }

  .providers-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .provider-card {
    padding: 16px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.05);
    border: 2px solid rgba(255, 255, 255, 0.1);
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .provider-card:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.2);
    transform: translateY(-2px);
  }

  .provider-card.active {
    background: rgba(59, 130, 246, 0.2);
    border-color: rgba(59, 130, 246, 0.6);
  }

  .provider-card.configured {
    border-color: rgba(34, 197, 94, 0.3);
  }

  .provider-card.not-configured {
    border-color: rgba(239, 68, 68, 0.3);
  }

  .provider-card.unavailable {
    opacity: 0.5;
    cursor: not-allowed;
    border-color: rgba(255, 255, 255, 0.05);
  }

  .provider-card.unavailable:hover {
    transform: none;
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.05);
  }

  .provider-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
  }

  .provider-actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .edit-key-btn,
  .download-btn {
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.2);
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.7);
    font-size: 12px;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .edit-key-btn:hover,
  .download-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.15);
    color: #fff;
    border-color: rgba(255, 255, 255, 0.4);
  }

  .download-btn {
    background: rgba(59, 130, 246, 0.2);
    border-color: rgba(59, 130, 246, 0.4);
    color: #93c5fd;
  }

  .download-btn:hover:not(:disabled) {
    background: rgba(59, 130, 246, 0.3);
    border-color: rgba(59, 130, 246, 0.6);
  }

  .download-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  h3 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    color: #fff;
  }

  .badge {
    padding: 4px 12px;
    border-radius: 12px;
    font-size: 12px;
    font-weight: 500;
  }

  .badge-active {
    background: rgba(59, 130, 246, 0.3);
    color: #93c5fd;
  }

  .badge-configured {
    background: rgba(34, 197, 94, 0.3);
    color: #86efac;
  }

  .badge-not-configured {
    background: rgba(239, 68, 68, 0.3);
    color: #fca5a5;
  }

  .badge-unavailable {
    background: rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.5);
  }

  .provider-details {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .provider-type {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.6);
  }

  .provider-note {
    font-size: 12px;
    color: rgba(34, 197, 94, 0.8);
  }

  .download-status {
    margin-top: 10px;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.6);
  }

  .download-success {
    margin-top: 10px;
    font-size: 12px;
    color: #86efac;
  }

  .download-error {
    margin-top: 10px;
    font-size: 12px;
    color: #fca5a5;
  }

  .progress-bar-container {
    margin-top: 8px;
    height: 6px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 3px;
    overflow: hidden;
  }

  .progress-bar {
    height: 100%;
    background: #3b82f6;
    border-radius: 3px;
    transition: width 0.3s ease;
  }

  .locale-selector {
    margin-top: 12px;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .locale-selector label {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.7);
  }

  .locale-selector select {
    padding: 4px 8px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.2);
    background: rgba(0, 0, 0, 0.3);
    color: #fff;
    font-size: 13px;
    cursor: pointer;
  }

  .locale-selector select:focus {
    outline: none;
    border-color: rgba(59, 130, 246, 0.6);
  }

  .custom-endpoint-form {
    margin-top: 16px;
    padding: 16px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
  }

  .form-description {
    margin: 0 0 16px;
    color: rgba(255, 255, 255, 0.5);
    font-size: 13px;
  }

  .form-group {
    margin-bottom: 12px;
  }

  .form-group label {
    display: block;
    margin-bottom: 4px;
    font-size: 13px;
    color: rgba(255, 255, 255, 0.7);
  }

  .optional {
    color: rgba(255, 255, 255, 0.35);
    font-size: 12px;
  }

  .form-input {
    width: 100%;
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background: rgba(255, 255, 255, 0.05);
    color: #fff;
    font-size: 14px;
    outline: none;
    transition: border-color 0.2s ease;
    box-sizing: border-box;
  }

  .form-input:focus {
    border-color: rgba(59, 130, 246, 0.6);
  }

  .form-input::placeholder {
    color: rgba(255, 255, 255, 0.3);
  }

  .save-custom-btn {
    width: 100%;
    padding: 10px;
    border-radius: 8px;
    border: none;
    background: rgba(59, 130, 246, 0.3);
    color: #93c5fd;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
    margin-top: 4px;
  }

  .save-custom-btn:hover:not(:disabled) {
    background: rgba(59, 130, 246, 0.5);
  }

  .save-custom-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .loading {
    text-align: center;
    padding: 12px;
    color: rgba(255, 255, 255, 0.7);
    font-size: 14px;
  }

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
    background: #1f2937;
    padding: 24px;
    border-radius: 16px;
    max-width: 400px;
    width: 90%;
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .modal h3 {
    margin-bottom: 12px;
    font-size: 20px;
  }

  .modal p {
    margin-bottom: 16px;
    color: rgba(255, 255, 255, 0.7);
    font-size: 14px;
  }

  .api-key-input-wrapper {
    position: relative;
    margin-bottom: 20px;
  }

  .api-key-input {
    width: 100%;
    padding: 12px;
    padding-right: 48px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.2);
    background: rgba(0, 0, 0, 0.3);
    color: #fff;
    font-size: 14px;
  }

  .api-key-input:focus {
    outline: none;
    border-color: rgba(59, 130, 246, 0.6);
  }

  .toggle-visibility-btn {
    position: absolute;
    right: 8px;
    top: 50%;
    transform: translateY(-50%);
    background: none;
    border: none;
    cursor: pointer;
    padding: 8px;
    font-size: 16px;
    opacity: 0.6;
    transition: opacity 0.2s ease;
  }

  .toggle-visibility-btn:hover {
    opacity: 1;
  }

  .modal-actions {
    display: flex;
    gap: 12px;
    justify-content: flex-end;
  }

  .btn {
    padding: 10px 20px;
    border-radius: 8px;
    border: none;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-primary {
    background: #3b82f6;
    color: #fff;
  }

  .btn-primary:hover:not(:disabled) {
    background: #2563eb;
  }

  .btn-secondary {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }

  .btn-secondary:hover {
    background: rgba(255, 255, 255, 0.15);
  }
</style>
