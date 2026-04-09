<script lang="ts">
  import { safeInvoke as invoke } from '../../lib/tauri';
  import { onMount } from 'svelte';
  import PageHeader from './ui/PageHeader.svelte';
  import SectionHeader from './ui/SectionHeader.svelte';
  import StatusRow from './ui/StatusRow.svelte';
  import ActionRow from './ui/ActionRow.svelte';

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

  let showApiKeyModal = $state(false);
  let selectedProvider = $state<LlmProcessorInfo | null>(null);
  let apiKeyInput = $state('');
  let showApiKey = $state(false);
  let editingExistingKey = $state(false);

  let showCustomSection = $state(false);
  let customBaseUrl = $state('');
  let customDisplayName = $state('');
  let customApiKey = $state('');

  // Derived groups
  let cliProcessors = $derived(processors.filter(p => p.provider_type === 'cli'));
  let localProcessors = $derived(processors.filter(p => p.provider_type === 'local'));
  let apiProcessors = $derived(processors.filter(p => p.provider_type === 'http'));

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

  async function selectProcessor(processorId: string) {
    const processor = processors.find(p => p.id === processorId);
    if (!processor) return;

    if (processor.provider_type === 'cli' && !processor.available) {
      error = `${processor.name} is not installed. Please install it first.`;
      setTimeout(() => { error = ''; }, 5000);
      return;
    }

    if (processor.requires_api_key && !processor.configured) {
      selectedProvider = processor;
      showApiKeyModal = true;
      editingExistingKey = false;
      apiKeyInput = '';
      return;
    }

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

      await invoke('save_api_key', {
        provider: selectedProvider.api_key_name,
        apiKey: apiKeyInput
      });

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

      await invoke('set_custom_llm_endpoint', {
        baseUrl: customBaseUrl,
        displayName: customDisplayName || null,
      });

      if (customApiKey.trim()) {
        await invoke('save_api_key', {
          provider: 'custom_llm',
          apiKey: customApiKey
        });
      }

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

  function getStatus(processor: LlmProcessorInfo): 'green' | 'yellow' | 'red' | 'none' {
    if (currentProcessor === processor.id) return 'green';
    if (processor.provider_type === 'cli' && processor.available) return 'yellow';
    if (processor.provider_type === 'cli' && !processor.available) return 'red';
    if (processor.provider_type === 'local' && processor.available) return 'yellow';
    if (processor.provider_type === 'local' && !processor.available) return 'red';
    if (processor.configured) return 'yellow';
    if (processor.requires_api_key && !processor.configured) return 'red';
    return 'none';
  }

  function getStatusText(processor: LlmProcessorInfo): string {
    if (currentProcessor === processor.id) return 'Active';
    if (processor.provider_type === 'cli' && processor.available) return 'Available';
    if (processor.provider_type === 'cli' && !processor.available) return 'Not Installed';
    if (processor.provider_type === 'local' && processor.available) return 'Ready';
    if (processor.provider_type === 'local' && !processor.available) return 'Unavailable';
    if (processor.configured) return 'Configured';
    if (processor.requires_api_key && !processor.configured) return 'API Key Required';
    return 'Available';
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

<div class="page">
  <PageHeader title="LLM Processor" description="Configure language model for text processing" />

  {#if error}
    <div class="alert alert-error">{error}</div>
  {/if}
  {#if success}
    <div class="alert alert-success">{success}</div>
  {/if}

  <!-- LOCAL CLI -->
  {#if cliProcessors.length > 0}
    <div class="section">
      <SectionHeader label="LOCAL CLI" />
      <div class="section-rows">
        {#each cliProcessors as processor}
          <StatusRow
            label={processor.name}
            value={processor.provider_type === 'cli' ? 'CLI' : 'on-device'}
            status={getStatus(processor)}
            statusText={getStatusText(processor)}
            onclick={() => selectProcessor(processor.id)}
          />
          {#if !processor.available && processor.provider_type === 'cli'}
            <div class="install-hint">{getInstallCommand(processor.id)}</div>
          {/if}
        {/each}
      </div>
    </div>
  {/if}

  <!-- API PROVIDERS -->
  {#if apiProcessors.length > 0}
    <div class="section">
      <SectionHeader label="API PROVIDERS" />
      <div class="section-rows">
        {#each apiProcessors as processor}
          <StatusRow
            label={processor.name}
            value={processor.default_model}
            status={getStatus(processor)}
            statusText={getStatusText(processor)}
            onclick={() => selectProcessor(processor.id)}
          >
            {#snippet children()}
              {#if processor.requires_api_key && processor.configured}
                <button class="inline-btn" onclick={(e) => { e.stopPropagation(); editApiKey(processor); }}>
                  Edit Key
                </button>
              {/if}
            {/snippet}
          </StatusRow>
        {/each}
      </div>
    </div>
  {/if}

  <!-- LOCAL ON-DEVICE -->
  {#if localProcessors.length > 0}
    <div class="section">
      <SectionHeader label="LOCAL ON-DEVICE" />
      <div class="section-rows">
        {#each localProcessors as processor}
          <StatusRow
            label={processor.name}
            value="on-device"
            status={getStatus(processor)}
            statusText={getStatusText(processor)}
            onclick={() => selectProcessor(processor.id)}
          />
        {/each}
      </div>
    </div>
  {/if}

  <!-- CUSTOM ENDPOINT -->
  <div class="section">
    <SectionHeader label="CUSTOM ENDPOINT" />
    <ActionRow
      label="Add custom OpenAI-compatible endpoint"
      onclick={() => { showCustomSection = !showCustomSection; }}
    />

    {#if showCustomSection}
      <div class="custom-form">
        <p class="form-desc">Connect to any OpenAI-compatible endpoint (Ollama, LM Studio, Azure OpenAI, etc.)</p>
        <div class="form-group">
          <label for="custom-base-url">Base URL</label>
          <input id="custom-base-url" type="text" bind:value={customBaseUrl} placeholder="http://localhost:11434/v1" />
        </div>
        <div class="form-group">
          <label for="custom-api-key">API Key <span class="optional">(optional for local)</span></label>
          <input id="custom-api-key" type="password" bind:value={customApiKey} placeholder="API key (if required)" />
        </div>
        <div class="form-group">
          <label for="custom-display-name">Display Name <span class="optional">(optional)</span></label>
          <input id="custom-display-name" type="text" bind:value={customDisplayName} placeholder="e.g., Local Ollama" />
        </div>
        <button class="primary-btn" onclick={saveCustomEndpoint} disabled={loading || !customBaseUrl.trim()}>
          {loading ? 'Saving...' : 'Save & Activate'}
        </button>
      </div>
    {/if}
  </div>

  <div class="separator"></div>

  <!-- MODEL OVERRIDE -->
  <div class="section">
    <SectionHeader label="MODEL OVERRIDE" />
    <div class="model-row">
      <input
        type="text"
        class="model-input"
        bind:value={currentModel}
        placeholder={defaultModel ? `e.g. ${defaultModel}` : 'default'}
        onkeydown={(e) => e.key === 'Enter' && saveModel()}
      />
      <button class="apply-btn" onclick={saveModel} disabled={modelLoading}>
        {modelLoading ? '...' : 'Apply'}
      </button>
    </div>
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
        <button class="visibility-toggle" onclick={() => showApiKey = !showApiKey} type="button">
          {showApiKey ? '🙈' : '👁️'}
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
  .page {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

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

  .separator {
    height: 1px;
    background: var(--border);
    width: 100%;
  }

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

  .inline-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-primary);
  }

  .install-hint {
    padding: 4px 12px;
    font-size: 11px;
    color: var(--text-muted);
  }

  /* Custom form */
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

  /* Model override */
  .model-row {
    display: flex;
    gap: 8px;
  }

  .model-input {
    flex: 1;
    padding: 8px 12px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--bg-card);
    color: var(--text-primary);
    font-size: 12px;
    outline: none;
    transition: border-color 0.15s ease;
  }

  .model-input:focus {
    border-color: rgba(168, 85, 247, 0.6);
  }

  .model-input::placeholder {
    color: var(--text-placeholder);
  }

  .apply-btn {
    padding: 8px 16px;
    border-radius: 8px;
    border: none;
    background: var(--accent);
    color: var(--text-primary);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s ease;
    white-space: nowrap;
  }

  .apply-btn:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .apply-btn:disabled {
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
