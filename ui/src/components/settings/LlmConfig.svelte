<script lang="ts">
  import Alert from './ui/Alert.svelte';
  import { useLifecycle } from '../../lib/lifecycle';
  import { createStatus } from '../../lib/status.svelte';
  import { trapFocus } from '../../lib/focus';
  import { safeInvoke as invoke } from '../../lib/tauri';
  import { onMount } from 'svelte';
  import PageHeader from './ui/PageHeader.svelte';
  import SectionHeader from './ui/SectionHeader.svelte';
  import StatusRow from './ui/StatusRow.svelte';
  import ActionRow from './ui/ActionRow.svelte';

  const lifecycle = useLifecycle();
  const status = createStatus(lifecycle);

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
  // The model row saves independently of the rest of the page.
  let modelLoading = $state(false);

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
    await status.run('Failed to load LLM processors', async () => {
      processors = await invoke<LlmProcessorInfo[]>('get_llm_processors');
    });
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
      status.fail(`Failed to load config: ${err}`);
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
      status.fail(`${processor.name} is not installed. Please install it first.`, 5000);
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
      status.fail(`${processor.name} is not available on this system.`, 5000);
      return;
    }

    await status.run('Failed to switch processor', async () => {
      await invoke('set_llm_processor', { processor: processorId });
      currentProcessor = processorId;
      updateDefaultModel();
      status.confirm(`Switched to ${processor.name}`);
    });
  }

  function editApiKey(processor: LlmProcessorInfo) {
    selectedProvider = processor;
    showApiKeyModal = true;
    editingExistingKey = true;
    apiKeyInput = '';
  }

  async function saveApiKey() {
    if (!apiKeyInput.trim()) {
      status.fail('API key cannot be empty');
      return;
    }

    if (!selectedProvider || !selectedProvider.api_key_name) return;

    const provider = selectedProvider;
    const updating = editingExistingKey;
    await status.run('Failed to save API key', async () => {
      await invoke('save_api_key', {
        provider: provider.api_key_name,
        apiKey: apiKeyInput
      });
      await invoke('set_llm_processor', { processor: provider.id });

      currentProcessor = provider.id;
      updateDefaultModel();
      showApiKeyModal = false;
      await loadProcessors();
      status.confirm(updating
        ? `Updated API key for ${provider.name}`
        : `Configured and activated ${provider.name}`);
    });
  }

  function closeModal() {
    showApiKeyModal = false;
    apiKeyInput = '';
    showApiKey = false;
    editingExistingKey = false;
    status.reset();
  }

  async function saveModel() {
    modelLoading = true;
    await status.run('Failed to set model', async () => {
      await invoke('set_llm_model', { model: currentModel });
      status.confirm(currentModel
        ? `Model set to ${currentModel}`
        : `Reset to default model (${defaultModel})`);
    });
    modelLoading = false;
  }

  async function saveCustomEndpoint() {
    if (!customBaseUrl.trim()) {
      status.fail('Base URL is required for custom endpoint');
      return;
    }

    await status.run('Failed to save custom endpoint', async () => {
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
      customApiKey = '';
      status.confirm(`Custom endpoint activated: ${customDisplayName || customBaseUrl}`);
    });
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

  <Alert error={status.error} success={status.success} />

  <!-- LOCAL CLI -->
  {#if cliProcessors.length > 0}
    <div class="section">
      <SectionHeader label="LOCAL CLI" />
      <div class="section-rows">
        {#each cliProcessors as processor (processor.id)}
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
        {#each apiProcessors as processor (processor.id)}
          <StatusRow
            label={processor.name}
            value={processor.default_model}
            status={getStatus(processor)}
            statusText={getStatusText(processor)}
            onclick={() => selectProcessor(processor.id)}
          >
            {#if processor.requires_api_key && processor.configured}
              <button class="inline-btn" onclick={(e) => { e.stopPropagation(); editApiKey(processor); }}>
                Edit Key
              </button>
            {/if}
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
        {#each localProcessors as processor (processor.id)}
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
        <button class="btn btn-block btn-primary" onclick={saveCustomEndpoint} disabled={status.busy || !customBaseUrl.trim()}>
          {status.busy ? 'Saving...' : 'Save & Activate'}
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
  <div class="modal-overlay" onclick={closeModal} onkeydown={(e) => e.key === 'Escape' && closeModal()} role="presentation">
    <div class="modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => { if (e.key === 'Escape') closeModal(); e.stopPropagation(); }} use:trapFocus role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="llm-api-key-title">
      <h3 id="llm-api-key-title">{editingExistingKey ? 'Update' : 'Configure'} {selectedProvider?.name}</h3>
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
        <button class="btn btn-md btn-secondary" onclick={closeModal}>Cancel</button>
        <button class="btn btn-md btn-primary" onclick={saveApiKey} disabled={status.busy}>
          {status.busy ? 'Saving...' : editingExistingKey ? 'Update Key' : 'Save & Activate'}
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
    background: var(--surface-raised);
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






</style>
