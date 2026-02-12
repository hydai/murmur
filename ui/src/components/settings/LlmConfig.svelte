<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  let processors = [];
  let currentProcessor = '';
  let loading = false;
  let error = '';
  let success = '';

  onMount(async () => {
    await loadProcessors();
    await loadConfig();
  });

  async function loadProcessors() {
    try {
      processors = await invoke('get_llm_processors');
    } catch (err) {
      error = `Failed to load LLM processors: ${err}`;
      console.error(error);
    }
  }

  async function loadConfig() {
    try {
      const config = await invoke('get_config');
      currentProcessor = config.llm_processor.toLowerCase();
    } catch (err) {
      error = `Failed to load config: ${err}`;
      console.error(error);
    }
  }

  async function selectProcessor(processorId) {
    const processor = processors.find(p => p.id === processorId);

    if (!processor.available) {
      error = `${processor.name} is not installed. Please install it first.`;
      setTimeout(() => { error = ''; }, 5000);
      return;
    }

    try {
      loading = true;
      error = '';
      success = '';

      await invoke('set_llm_processor', { processor: processorId });
      currentProcessor = processorId;
      success = `Switched to ${processor.name}`;
      setTimeout(() => { success = ''; }, 3000);
    } catch (err) {
      error = `Failed to switch processor: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  function getProcessorStatusClass(processor) {
    if (currentProcessor === processor.id) {
      return 'active';
    }
    if (processor.available) {
      return 'available';
    }
    return 'not-available';
  }

  function getInstallCommand(processorId) {
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
    Select which local CLI tool to use for text post-processing.
    The processor must be installed and available in your PATH.
  </p>

  {#if error}
    <div class="alert alert-error">{error}</div>
  {/if}

  {#if success}
    <div class="alert alert-success">{success}</div>
  {/if}

  <div class="processors-list">
    {#each processors as processor}
      <div
        class="processor-card {getProcessorStatusClass(processor)}"
        on:click={() => selectProcessor(processor.id)}
        on:keypress={(e) => e.key === 'Enter' && selectProcessor(processor.id)}
        role="button"
        tabindex="0"
      >
        <div class="processor-header">
          <h3>{processor.name}</h3>
          {#if currentProcessor === processor.id}
            <span class="badge badge-active">Active</span>
          {:else if processor.available}
            <span class="badge badge-available">Available</span>
          {:else}
            <span class="badge badge-not-available">Not Installed</span>
          {/if}
        </div>
        {#if !processor.available}
          <div class="install-hint">
            <span class="hint-icon">ℹ️</span>
            {getInstallCommand(processor.id)}
          </div>
        {/if}
      </div>
    {/each}
  </div>

  {#if loading}
    <div class="loading">Processing...</div>
  {/if}
</div>

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

  .processors-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .processor-card {
    padding: 16px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.05);
    border: 2px solid rgba(255, 255, 255, 0.1);
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .processor-card:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.2);
    transform: translateY(-2px);
  }

  .processor-card.active {
    background: rgba(59, 130, 246, 0.2);
    border-color: rgba(59, 130, 246, 0.6);
  }

  .processor-card.available {
    border-color: rgba(34, 197, 94, 0.3);
  }

  .processor-card.not-available {
    border-color: rgba(239, 68, 68, 0.3);
    opacity: 0.8;
  }

  .processor-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
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

  .badge-available {
    background: rgba(34, 197, 94, 0.3);
    color: #86efac;
  }

  .badge-not-available {
    background: rgba(239, 68, 68, 0.3);
    color: #fca5a5;
  }

  .install-hint {
    margin-top: 12px;
    padding: 8px 12px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 6px;
    font-size: 13px;
    color: rgba(255, 255, 255, 0.6);
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }

  .hint-icon {
    font-size: 14px;
    flex-shrink: 0;
  }

  .loading {
    text-align: center;
    padding: 12px;
    color: rgba(255, 255, 255, 0.7);
    font-size: 14px;
  }
</style>
