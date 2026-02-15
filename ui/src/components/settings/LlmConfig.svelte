<script lang="ts">
  import { safeInvoke as invoke } from '../../lib/tauri';
  import { onMount } from 'svelte';

  interface LlmProcessorInfo {
    name: string;
    id: string;
    available: boolean;
    default_model: string;
    provider_type: string;
    requires_api_key: boolean;
    configured: boolean;
    api_key_name: string | null;
  }

  let processors = $state<LlmProcessorInfo[]>([]);
  let currentProcessor = $state('');
  let currentModel = $state('');
  let defaultModel = $state('');
  let loading = $state(false);
  let modelLoading = $state(false);
  let error = $state('');
  let success = $state('');

  // API key modal state
  let showApiKeyModal = $state(false);
  let selectedProvider = $state<LlmProcessorInfo | null>(null);
  let apiKeyInput = $state('');
  let showApiKey = $state(false);
  let editingExistingKey = $state(false);

  // Custom endpoint state
  let showCustomSection = $state(false);
  let customBaseUrl = $state('');
  let customDisplayName = $state('');
  let customApiKey = $state('');

  onMount(async () => {
    await loadProcessors();
    await loadConfig();
  });

  async function loadProcessors() {
    try {
      processors = await invoke<LlmProcessorInfo[]>('get_llm_processors');
    } catch (err) {
      error = `Failed to load LLM processors: ${err}`;
      console.error(error);
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
        };
      }>('get_config');
      currentProcessor = config.llm_processor.toLowerCase();
      currentModel = config.llm_model || '';
      customBaseUrl = config.http_llm_config?.custom_base_url || '';
      customDisplayName = config.http_llm_config?.custom_display_name || '';
      // Auto-expand custom section if custom_api is selected
      if (currentProcessor === 'custom_api') {
        showCustomSection = true;
      }
      updateDefaultModel();
    } catch (err) {
      error = `Failed to load config: ${err}`;
      console.error(error);
    }
  }

  function updateDefaultModel() {
    const active = processors.find(p => p.id === currentProcessor);
    defaultModel = active?.default_model || '';
  }

  // Split processors by type
  function getCliProcessors(): LlmProcessorInfo[] {
    return processors.filter(p => p.provider_type === 'cli' || p.provider_type === 'local');
  }

  function getApiProcessors(): LlmProcessorInfo[] {
    return processors.filter(p => p.provider_type === 'http');
  }

  function getCustomProcessor(): LlmProcessorInfo | undefined {
    return processors.find(p => p.provider_type === 'custom');
  }

  async function selectProcessor(processorId: string) {
    const processor = processors.find(p => p.id === processorId);
    if (!processor) return;

    // CLI processors: check availability
    if (processor.provider_type === 'cli' && !processor.available) {
      error = `${processor.name} is not installed. Please install it first.`;
      setTimeout(() => { error = ''; }, 5000);
      return;
    }

    // HTTP processors: check if API key is configured
    if (processor.requires_api_key && !processor.configured) {
      selectedProvider = processor;
      showApiKeyModal = true;
      editingExistingKey = false;
      apiKeyInput = '';
      return;
    }

    // Local (Apple) processors
    if (processor.provider_type === 'local' && !processor.available) {
      error = `${processor.name} is not available on this system.`;
      setTimeout(() => { error = ''; }, 5000);
      return;
    }

    try {
      loading = true;
      error = '';
      success = '';

      await invoke('set_llm_processor', { processor: processorId });
      currentProcessor = processorId;
      updateDefaultModel();
      success = `Switched to ${processor.name}`;
      setTimeout(() => { success = ''; }, 3000);
    } catch (err) {
      error = `Failed to switch processor: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  function editApiKey(processor: LlmProcessorInfo) {
    selectedProvider = processor;
    showApiKeyModal = true;
    editingExistingKey = true;
    apiKeyInput = '';
  }

  async function saveApiKey() {
    if (!apiKeyInput.trim()) {
      error = 'API key cannot be empty';
      return;
    }

    if (!selectedProvider || !selectedProvider.api_key_name) return;

    try {
      loading = true;
      error = '';
      success = '';

      // Save the API key
      await invoke('save_api_key', {
        provider: selectedProvider.api_key_name,
        apiKey: apiKeyInput
      });

      // Activate the provider
      await invoke('set_llm_processor', {
        processor: selectedProvider.id
      });

      currentProcessor = selectedProvider.id;
      updateDefaultModel();
      success = editingExistingKey
        ? `Updated API key for ${selectedProvider.name}`
        : `Configured and activated ${selectedProvider.name}`;

      showApiKeyModal = false;
      await loadProcessors();

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

  async function saveModel() {
    try {
      modelLoading = true;
      error = '';
      success = '';

      await invoke('set_llm_model', { model: currentModel });
      success = currentModel
        ? `Model set to ${currentModel}`
        : `Reset to default model (${defaultModel})`;
      setTimeout(() => { success = ''; }, 3000);
    } catch (err) {
      error = `Failed to set model: ${err}`;
      console.error(error);
    } finally {
      modelLoading = false;
    }
  }

  async function saveCustomEndpoint() {
    if (!customBaseUrl.trim()) {
      error = 'Base URL is required for custom endpoint';
      return;
    }

    try {
      loading = true;
      error = '';
      success = '';

      // Save endpoint config
      await invoke('set_custom_llm_endpoint', {
        baseUrl: customBaseUrl,
        displayName: customDisplayName || null,
      });

      // Save API key if provided
      if (customApiKey.trim()) {
        await invoke('save_api_key', {
          provider: 'custom_llm',
          apiKey: customApiKey
        });
      }

      // Activate custom provider
      await invoke('set_llm_processor', { processor: 'custom_api' });
      currentProcessor = 'custom_api';

      await loadProcessors();
      updateDefaultModel();
      success = `Custom endpoint activated: ${customDisplayName || customBaseUrl}`;
      customApiKey = '';
      setTimeout(() => { success = ''; }, 3000);
    } catch (err) {
      error = `Failed to save custom endpoint: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  function getProcessorStatusClass(processor: LlmProcessorInfo): string {
    if (currentProcessor === processor.id) return 'active';
    if (processor.provider_type === 'cli' && processor.available) return 'available';
    if (processor.provider_type === 'cli' && !processor.available) return 'not-available';
    if (processor.provider_type === 'local' && processor.available) return 'available';
    if (processor.provider_type === 'local' && !processor.available) return 'not-available';
    if (processor.configured) return 'configured';
    return 'not-configured';
  }

  function getInstallCommand(processorId: string): string {
    if (processorId === 'gemini') {
      return 'Install from: https://github.com/google/generative-ai-cli';
    } else if (processorId === 'copilot') {
      return 'Install: npm install -g @githubnext/github-copilot-cli';
    }
    return '';
  }
</script>

<div class="llm-config">
  <h2>LLM Processor</h2>
  <p class="description">
    Select which LLM to use for text post-processing.
    Choose from local CLI tools, cloud API providers, or a custom endpoint.
  </p>

  {#if error}
    <div class="alert alert-error">{error}</div>
  {/if}

  {#if success}
    <div class="alert alert-success">{success}</div>
  {/if}

  <!-- Section 1: Local CLI Processors -->
  <div class="section">
    <h3 class="section-title">Local CLI Processors</h3>
    <div class="processors-list">
      {#each getCliProcessors() as processor}
        <div
          class="processor-card {getProcessorStatusClass(processor)}"
          onclick={() => selectProcessor(processor.id)}
          onkeydown={(e) => e.key === 'Enter' && selectProcessor(processor.id)}
          role="button"
          tabindex="0"
        >
          <div class="processor-header">
            <h4>{processor.name}</h4>
            <div class="processor-actions">
              {#if currentProcessor === processor.id}
                <span class="badge badge-active">Active</span>
              {:else if processor.available}
                <span class="badge badge-available">Available</span>
              {:else}
                <span class="badge badge-not-available">Not Installed</span>
              {/if}
            </div>
          </div>
          <div class="processor-details">
            <span class="provider-type">{processor.provider_type === 'local' ? 'On-Device' : 'CLI Tool'}</span>
          </div>
          {#if !processor.available && processor.provider_type === 'cli'}
            <div class="install-hint">
              <span class="hint-icon">i</span>
              {getInstallCommand(processor.id)}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  </div>

  <!-- Section 2: API Providers -->
  <div class="section">
    <h3 class="section-title">API Providers</h3>
    <div class="processors-list">
      {#each getApiProcessors() as processor}
        <div
          class="processor-card {getProcessorStatusClass(processor)}"
          onclick={() => selectProcessor(processor.id)}
          onkeydown={(e) => e.key === 'Enter' && selectProcessor(processor.id)}
          role="button"
          tabindex="0"
        >
          <div class="processor-header">
            <h4>{processor.name}</h4>
            <div class="processor-actions">
              {#if processor.requires_api_key && processor.configured}
                <button
                  class="edit-key-btn"
                  onclick={(e) => { e.stopPropagation(); editApiKey(processor); }}
                  title="Edit API Key"
                >
                  Edit Key
                </button>
              {/if}
              {#if currentProcessor === processor.id}
                <span class="badge badge-active">Active</span>
              {:else if processor.configured}
                <span class="badge badge-configured">Configured</span>
              {:else}
                <span class="badge badge-api-key-required">API Key Required</span>
              {/if}
            </div>
          </div>
          <div class="processor-details">
            <span class="provider-type">HTTP API</span>
            <span class="default-model">Default: {processor.default_model}</span>
          </div>
        </div>
      {/each}
    </div>
  </div>

  <!-- Section 3: Custom Endpoint -->
  <div class="section">
    <button
      class="section-toggle"
      onclick={() => showCustomSection = !showCustomSection}
    >
      <h3 class="section-title">
        Custom Endpoint
        {#if currentProcessor === 'custom_api'}
          <span class="badge badge-active" style="margin-left: 8px; font-size: 11px;">Active</span>
        {/if}
      </h3>
      <span class="toggle-arrow">{showCustomSection ? '▼' : '▶'}</span>
    </button>
    {#if showCustomSection}
      <div class="custom-endpoint-form">
        <p class="form-description">
          Connect to any OpenAI-compatible endpoint (Ollama, LM Studio, Azure OpenAI, etc.)
        </p>
        <div class="form-group">
          <label for="custom-base-url">Base URL</label>
          <input
            id="custom-base-url"
            type="text"
            class="form-input"
            bind:value={customBaseUrl}
            placeholder="http://localhost:11434/v1"
          />
        </div>
        <div class="form-group">
          <label for="custom-api-key">API Key <span class="optional">(optional for local)</span></label>
          <input
            id="custom-api-key"
            type="password"
            class="form-input"
            bind:value={customApiKey}
            placeholder="API key (if required)"
          />
        </div>
        <div class="form-group">
          <label for="custom-display-name">Display Name <span class="optional">(optional)</span></label>
          <input
            id="custom-display-name"
            type="text"
            class="form-input"
            bind:value={customDisplayName}
            placeholder="e.g., Local Ollama"
          />
        </div>
        <button
          class="save-custom-btn"
          onclick={saveCustomEndpoint}
          disabled={loading || !customBaseUrl.trim()}
        >
          {loading ? 'Saving...' : 'Save & Activate'}
        </button>
      </div>
    {/if}
  </div>

  <!-- Model Override (works for all providers) -->
  <div class="model-section">
    <h3 class="model-title">Model Override</h3>
    <p class="model-description">
      Override the default model for the active processor. Leave empty to use the default.
    </p>
    <div class="model-input-row">
      <input
        type="text"
        class="model-input"
        bind:value={currentModel}
        placeholder={defaultModel || 'default'}
        onkeydown={(e) => e.key === 'Enter' && saveModel()}
      />
      <button
        class="model-apply-btn"
        onclick={saveModel}
        disabled={modelLoading}
      >
        {modelLoading ? 'Applying...' : 'Apply'}
      </button>
    </div>
  </div>

  {#if loading}
    <div class="loading">Processing...</div>
  {/if}
</div>

<!-- API Key Modal -->
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
          onclick={() => showApiKey = !showApiKey}
          type="button"
          title={showApiKey ? 'Hide API key' : 'Show API key'}
        >
          {showApiKey ? '🙈' : '👁️'}
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
  .llm-config {
    padding: 20px;
    max-width: 600px;
  }

  h2 {
    margin-bottom: 8px;
    font-size: 24px;
    font-weight: bold;
    color: #fff;
  }

  .description {
    margin-bottom: 20px;
    color: rgba(255, 255, 255, 0.7);
    font-size: 14px;
    line-height: 1.5;
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

  /* Sections */
  .section {
    margin-bottom: 24px;
  }

  .section-title {
    margin: 0 0 12px;
    font-size: 16px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.9);
    display: flex;
    align-items: center;
  }

  .section-toggle {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
    background: none;
    border: none;
    padding: 0;
    margin-bottom: 12px;
    cursor: pointer;
    color: inherit;
  }

  .toggle-arrow {
    color: rgba(255, 255, 255, 0.5);
    font-size: 12px;
  }

  .processors-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .processor-card {
    padding: 14px 16px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.05);
    border: 2px solid rgba(255, 255, 255, 0.1);
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .processor-card:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.2);
    transform: translateY(-1px);
  }

  .processor-card.active {
    background: rgba(59, 130, 246, 0.2);
    border-color: rgba(59, 130, 246, 0.6);
  }

  .processor-card.available,
  .processor-card.configured {
    border-color: rgba(34, 197, 94, 0.3);
  }

  .processor-card.not-available,
  .processor-card.not-configured {
    border-color: rgba(255, 255, 255, 0.08);
    opacity: 0.8;
  }

  .processor-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .processor-actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  h4 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: #fff;
  }

  .processor-details {
    display: flex;
    gap: 12px;
    align-items: center;
    margin-top: 4px;
  }

  .provider-type {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
  }

  .default-model {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.4);
  }

  .badge {
    padding: 3px 10px;
    border-radius: 12px;
    font-size: 11px;
    font-weight: 500;
    white-space: nowrap;
  }

  .badge-active {
    background: rgba(59, 130, 246, 0.3);
    color: #93c5fd;
  }

  .badge-available,
  .badge-configured {
    background: rgba(34, 197, 94, 0.3);
    color: #86efac;
  }

  .badge-not-available {
    background: rgba(239, 68, 68, 0.3);
    color: #fca5a5;
  }

  .badge-api-key-required {
    background: rgba(251, 191, 36, 0.3);
    color: #fde68a;
  }

  .edit-key-btn {
    padding: 3px 8px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.2);
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.7);
    font-size: 11px;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .edit-key-btn:hover {
    background: rgba(255, 255, 255, 0.15);
    color: #fff;
    border-color: rgba(255, 255, 255, 0.4);
  }

  .install-hint {
    margin-top: 10px;
    padding: 6px 10px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 6px;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
    display: flex;
    align-items: flex-start;
    gap: 6px;
  }

  .hint-icon {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.1);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    font-style: italic;
    flex-shrink: 0;
  }

  /* Custom endpoint form */
  .custom-endpoint-form {
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

  /* Model section */
  .model-section {
    margin-top: 24px;
    padding-top: 20px;
    border-top: 1px solid rgba(255, 255, 255, 0.1);
  }

  .model-title {
    margin: 0 0 6px;
    font-size: 16px;
    font-weight: 600;
    color: #fff;
  }

  .model-description {
    margin-bottom: 12px;
    color: rgba(255, 255, 255, 0.5);
    font-size: 13px;
  }

  .model-input-row {
    display: flex;
    gap: 8px;
  }

  .model-input {
    flex: 1;
    padding: 10px 14px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background: rgba(255, 255, 255, 0.05);
    color: #fff;
    font-size: 14px;
    outline: none;
    transition: border-color 0.2s ease;
  }

  .model-input:focus {
    border-color: rgba(59, 130, 246, 0.6);
  }

  .model-input::placeholder {
    color: rgba(255, 255, 255, 0.3);
  }

  .model-apply-btn {
    padding: 10px 20px;
    border-radius: 8px;
    border: none;
    background: rgba(59, 130, 246, 0.3);
    color: #93c5fd;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
    white-space: nowrap;
  }

  .model-apply-btn:hover:not(:disabled) {
    background: rgba(59, 130, 246, 0.5);
  }

  .model-apply-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .loading {
    text-align: center;
    padding: 12px;
    color: rgba(255, 255, 255, 0.7);
    font-size: 14px;
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
    background: #1f2937;
    padding: 24px;
    border-radius: 16px;
    max-width: 400px;
    width: 90%;
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .modal h3 {
    margin: 0 0 12px;
    font-size: 20px;
    font-weight: 600;
    color: #fff;
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
    box-sizing: border-box;
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
