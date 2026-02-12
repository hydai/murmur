<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  let providers = [];
  let currentProvider = '';
  let showApiKeyModal = false;
  let selectedProvider = null;
  let apiKeyInput = '';
  let showApiKey = false;
  let loading = false;
  let error = '';
  let success = '';

  onMount(async () => {
    await loadProviders();
    await loadConfig();
  });

  async function loadProviders() {
    try {
      providers = await invoke('get_stt_providers');
    } catch (err) {
      error = `Failed to load providers: ${err}`;
      console.error(error);
    }
  }

  async function loadConfig() {
    try {
      const config = await invoke('get_config');
      currentProvider = config.stt_provider.toLowerCase();
    } catch (err) {
      error = `Failed to load config: ${err}`;
      console.error(error);
    }
  }

  async function selectProvider(providerId) {
    const provider = providers.find(p => p.id === providerId);

    if (!provider.configured) {
      // Show API key input modal
      selectedProvider = provider;
      showApiKeyModal = true;
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
      success = `Switched to ${provider.name}`;
      setTimeout(() => { success = ''; }, 3000);
    } catch (err) {
      error = `Failed to switch provider: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  async function saveApiKey() {
    if (!apiKeyInput.trim()) {
      error = 'API key cannot be empty';
      return;
    }

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
      success = `Configured and activated ${selectedProvider.name}`;

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
    error = '';
  }

  function toggleApiKeyVisibility() {
    showApiKey = !showApiKey;
  }

  function getProviderStatusClass(provider) {
    if (currentProvider === provider.id) {
      return 'active';
    }
    if (provider.configured) {
      return 'configured';
    }
    return 'not-configured';
  }

  function getProviderTypeLabel(providerType) {
    return providerType === 'streaming' ? 'Streaming' : 'Batch';
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
        on:click={() => selectProvider(provider.id)}
        on:keypress={(e) => e.key === 'Enter' && selectProvider(provider.id)}
        role="button"
        tabindex="0"
      >
        <div class="provider-header">
          <h3>{provider.name}</h3>
          {#if currentProvider === provider.id}
            <span class="badge badge-active">Active</span>
          {:else if provider.configured}
            <span class="badge badge-configured">Configured</span>
          {:else}
            <span class="badge badge-not-configured">Not Configured</span>
          {/if}
        </div>
        <div class="provider-details">
          <span class="provider-type">{getProviderTypeLabel(provider.provider_type)}</span>
        </div>
      </div>
    {/each}
  </div>

  {#if loading}
    <div class="loading">Processing...</div>
  {/if}
</div>

{#if showApiKeyModal}
  <div class="modal-overlay" on:click={closeModal} on:keypress={(e) => e.key === 'Escape' && closeModal()} role="presentation">
    <div class="modal" on:click|stopPropagation role="dialog">
      <h3>Configure {selectedProvider?.name}</h3>
      <p>Enter your API key to enable this provider:</p>

      <div class="api-key-input-wrapper">
        <input
          type={showApiKey ? 'text' : 'password'}
          bind:value={apiKeyInput}
          placeholder="API Key"
          class="api-key-input"
          on:keypress={(e) => e.key === 'Enter' && saveApiKey()}
        />
        <button
          class="toggle-visibility-btn"
          on:click={toggleApiKeyVisibility}
          type="button"
          title={showApiKey ? 'Hide API key' : 'Show API key'}
        >
          {showApiKey ? '👁️' : '👁️‍🗨️'}
        </button>
      </div>

      <div class="modal-actions">
        <button class="btn btn-secondary" on:click={closeModal}>Cancel</button>
        <button class="btn btn-primary" on:click={saveApiKey} disabled={loading}>
          {loading ? 'Saving...' : 'Save & Activate'}
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

  .provider-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
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

  .provider-details {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .provider-type {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.6);
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
