<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  let currentOutputMode = 'clipboard';
  let loading = false;
  let error = '';
  let success = '';

  const outputModes = [
    {
      id: 'clipboard',
      name: 'Clipboard Only',
      description: 'Processed text is copied to clipboard. You can paste it anywhere with Cmd+V.',
      icon: '📋'
    },
    {
      id: 'keyboard',
      name: 'Keyboard Simulation',
      description: 'Processed text is typed automatically at cursor position.',
      icon: '⌨️'
    },
    {
      id: 'both',
      name: 'Clipboard + Keyboard',
      description: 'Text is both copied to clipboard and typed automatically.',
      icon: '📋⌨️'
    }
  ];

  onMount(async () => {
    await loadConfig();
  });

  async function loadConfig() {
    try {
      const config = await invoke('get_config');
      currentOutputMode = config.output_mode.toLowerCase();
    } catch (err) {
      error = `Failed to load config: ${err}`;
      console.error(error);
    }
  }

  async function selectOutputMode(modeId) {
    try {
      loading = true;
      error = '';
      success = '';

      await invoke('set_output_mode', { mode: modeId });
      currentOutputMode = modeId;

      const modeName = outputModes.find(m => m.id === modeId)?.name || modeId;
      success = `Output mode set to: ${modeName}`;
      setTimeout(() => { success = ''; }, 3000);
    } catch (err) {
      error = `Failed to set output mode: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  function getModeStatusClass(mode) {
    return currentOutputMode === mode.id ? 'active' : '';
  }
</script>

<div class="output-config">
  <h2>Output Mode</h2>
  <p class="description">
    Choose how processed text should be delivered after transcription and LLM processing.
  </p>

  {#if error}
    <div class="alert alert-error">{error}</div>
  {/if}

  {#if success}
    <div class="alert alert-success">{success}</div>
  {/if}

  <div class="modes-list">
    {#each outputModes as mode}
      <div
        class="mode-card {getModeStatusClass(mode)}"
        on:click={() => selectOutputMode(mode.id)}
        on:keypress={(e) => e.key === 'Enter' && selectOutputMode(mode.id)}
        role="button"
        tabindex="0"
      >
        <div class="mode-header">
          <div class="mode-title">
            <span class="mode-icon">{mode.icon}</span>
            <h3>{mode.name}</h3>
          </div>
          {#if currentOutputMode === mode.id}
            <span class="badge badge-active">Active</span>
          {/if}
        </div>
        <p class="mode-description">{mode.description}</p>
      </div>
    {/each}
  </div>

  {#if loading}
    <div class="loading">Updating...</div>
  {/if}
</div>

<style>
  .output-config {
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

  .modes-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .mode-card {
    padding: 16px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.05);
    border: 2px solid rgba(255, 255, 255, 0.1);
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .mode-card:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.2);
    transform: translateY(-2px);
  }

  .mode-card.active {
    background: rgba(59, 130, 246, 0.2);
    border-color: rgba(59, 130, 246, 0.6);
  }

  .mode-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
  }

  .mode-title {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .mode-icon {
    font-size: 24px;
  }

  h3 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    color: #fff;
  }

  .mode-description {
    margin: 0;
    font-size: 14px;
    color: rgba(255, 255, 255, 0.6);
    line-height: 1.5;
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

  .loading {
    text-align: center;
    padding: 12px;
    color: rgba(255, 255, 255, 0.7);
    font-size: 14px;
  }
</style>
