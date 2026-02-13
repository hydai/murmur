<script lang="ts">
  import ProviderConfig from './ProviderConfig.svelte';
  import DictionaryEditor from './DictionaryEditor.svelte';
  import LlmConfig from './LlmConfig.svelte';
  import HotkeyConfig from './HotkeyConfig.svelte';
  import OutputConfig from './OutputConfig.svelte';

  let { visible, onClose }: { visible: boolean; onClose: () => void } = $props();

  // Detect standalone mode (rendered in its own window via ?view=settings)
  const standalone = typeof window !== 'undefined'
    && new URLSearchParams(window.location.search).get('view') === 'settings';

  let activeTab = $state('providers');

  function switchTab(tab: string) {
    activeTab = tab;
  }
</script>

{#if visible}
  {#if standalone}
    <!-- Standalone window: no overlay backdrop, fill the window -->
    <div class="settings-standalone">
      <div class="settings-header">
        <h1>Settings</h1>
        <button class="close-btn" onclick={onClose}>✕</button>
      </div>

      <div class="settings-tabs">
        <button class="tab-button {activeTab === 'providers' ? 'active' : ''}" onclick={() => switchTab('providers')}>STT Providers</button>
        <button class="tab-button {activeTab === 'llm' ? 'active' : ''}" onclick={() => switchTab('llm')}>LLM Processor</button>
        <button class="tab-button {activeTab === 'hotkey' ? 'active' : ''}" onclick={() => switchTab('hotkey')}>Hotkey</button>
        <button class="tab-button {activeTab === 'output' ? 'active' : ''}" onclick={() => switchTab('output')}>Output</button>
        <button class="tab-button {activeTab === 'dictionary' ? 'active' : ''}" onclick={() => switchTab('dictionary')}>Dictionary</button>
      </div>

      <div class="settings-content">
        {#if activeTab === 'providers'}
          <ProviderConfig />
        {:else if activeTab === 'llm'}
          <LlmConfig />
        {:else if activeTab === 'hotkey'}
          <HotkeyConfig />
        {:else if activeTab === 'output'}
          <OutputConfig />
        {:else if activeTab === 'dictionary'}
          <DictionaryEditor />
        {/if}
      </div>
    </div>
  {:else}
    <!-- Inline overlay mode (original behavior) -->
    <div class="settings-overlay" onclick={onClose} onkeydown={(e) => e.key === 'Escape' && onClose()} role="presentation">
      <div class="settings-panel" onclick={(e) => e.stopPropagation()} role="dialog">
        <div class="settings-header">
          <h1>Settings</h1>
          <button class="close-btn" onclick={onClose}>✕</button>
        </div>

        <div class="settings-tabs">
          <button class="tab-button {activeTab === 'providers' ? 'active' : ''}" onclick={() => switchTab('providers')}>STT Providers</button>
          <button class="tab-button {activeTab === 'llm' ? 'active' : ''}" onclick={() => switchTab('llm')}>LLM Processor</button>
          <button class="tab-button {activeTab === 'hotkey' ? 'active' : ''}" onclick={() => switchTab('hotkey')}>Hotkey</button>
          <button class="tab-button {activeTab === 'output' ? 'active' : ''}" onclick={() => switchTab('output')}>Output</button>
          <button class="tab-button {activeTab === 'dictionary' ? 'active' : ''}" onclick={() => switchTab('dictionary')}>Dictionary</button>
        </div>

        <div class="settings-content">
          {#if activeTab === 'providers'}
            <ProviderConfig />
          {:else if activeTab === 'llm'}
            <LlmConfig />
          {:else if activeTab === 'hotkey'}
            <HotkeyConfig />
          {:else if activeTab === 'output'}
            <OutputConfig />
          {:else if activeTab === 'dictionary'}
            <DictionaryEditor />
          {/if}
        </div>
      </div>
    </div>
  {/if}
{/if}

<style>
  .settings-standalone {
    background: #1a1a1a;
    width: 100%;
    height: 100vh;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .settings-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.8);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
  }

  .settings-panel {
    background: #1a1a1a;
    border-radius: 16px;
    width: 90%;
    max-width: 700px;
    max-height: 90vh;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .settings-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px 24px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  }

  h1 {
    margin: 0;
    font-size: 24px;
    font-weight: bold;
    color: #fff;
  }

  .close-btn {
    background: none;
    border: none;
    font-size: 24px;
    color: rgba(255, 255, 255, 0.6);
    cursor: pointer;
    padding: 0;
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 8px;
    transition: all 0.2s ease;
  }

  .close-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }

  .settings-tabs {
    display: flex;
    gap: 4px;
    padding: 0 24px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  }

  .tab-button {
    background: none;
    border: none;
    padding: 12px 20px;
    font-size: 14px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.6);
    cursor: pointer;
    border-bottom: 2px solid transparent;
    transition: all 0.2s ease;
  }

  .tab-button:hover {
    color: rgba(255, 255, 255, 0.9);
  }

  .tab-button.active {
    color: #3b82f6;
    border-bottom-color: #3b82f6;
  }

  .settings-content {
    padding: 24px;
    overflow-y: auto;
    flex: 1;
  }
</style>
