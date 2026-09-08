<script lang="ts">
  import { useLifecycle } from '../../lib/lifecycle';
  import { safeInvoke as invoke } from '../../lib/tauri';
  import { onMount } from 'svelte';
  import PageHeader from './ui/PageHeader.svelte';
  import SectionHeader from './ui/SectionHeader.svelte';
  import StatusRow from './ui/StatusRow.svelte';

  const lifecycle = useLifecycle();

  let currentOutputMode = $state('clipboard');
  let saveHistory = $state(true);
  let chineseConversion = $state('traditional');

  const chineseConversions = [
    { id: 'traditional', name: 'Traditional Chinese (Taiwan)' },
    { id: 'none', name: 'No conversion' },
  ];
  let loading = $state(false);
  let error = $state('');
  let success = $state('');

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

  async function loadConfig(): Promise<void> {
    try {
      const config = await invoke<{ output_mode: string; save_history: boolean; chinese_conversion?: string }>('get_config');
      currentOutputMode = config.output_mode.toLowerCase();
      saveHistory = config.save_history !== false;
      chineseConversion = (config.chinese_conversion || 'traditional').toLowerCase();
    } catch (err: unknown) {
      error = `Failed to load config: ${err}`;
      console.error(error);
    }
  }

  async function selectOutputMode(modeId: string): Promise<void> {
    try {
      loading = true;
      error = '';
      success = '';

      await invoke('set_output_mode', { mode: modeId });
      currentOutputMode = modeId;

      const modeName = outputModes.find((m: typeof outputModes[number]) => m.id === modeId)?.name || modeId;
      success = `Output mode set to: ${modeName}`;
      lifecycle.timeout(() => { success = ''; }, 3000, 'success');
    } catch (err: unknown) {
      error = `Failed to set output mode: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  async function selectChineseConversion(mode: string): Promise<void> {
    try {
      loading = true;
      error = '';
      success = '';
      await invoke('set_chinese_conversion', { mode });
      chineseConversion = mode;
      const name = chineseConversions.find((c) => c.id === mode)?.name || mode;
      success = `Chinese output set to: ${name}`;
      lifecycle.timeout(() => { success = ''; }, 3000, 'success');
    } catch (err: unknown) {
      error = `Failed to set Chinese conversion: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  async function toggleSaveHistory(): Promise<void> {
    const enabled = !saveHistory;
    try {
      loading = true;
      error = '';
      success = '';
      await invoke('set_save_history', { enabled });
      saveHistory = enabled;
      success = enabled ? 'New transcriptions will be saved to history' : 'New transcriptions will not be saved';
      lifecycle.timeout(() => { success = ''; }, 3000, 'success');
    } catch (err: unknown) {
      error = `Failed to update history setting: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }
</script>

<div class="page">
  <PageHeader title="Output Mode" description="Choose how transcribed text is delivered" />

  {#if error}
    <div class="alert alert-error">{error}</div>
  {/if}

  {#if success}
    <div class="alert alert-success">{success}</div>
  {/if}

  <SectionHeader label="OUTPUT METHOD" />
  <div class="section-rows">
    {#each outputModes as mode (mode.id)}
      <StatusRow
        label={mode.name}
        value={mode.id}
        status={currentOutputMode === mode.id ? 'green' : 'none'}
        statusText={currentOutputMode === mode.id ? 'Active' : ''}
        onclick={() => selectOutputMode(mode.id)}
      />
    {/each}
  </div>

  <SectionHeader label="CHINESE OUTPUT" />
  <div class="section-rows">
    {#each chineseConversions as conversion (conversion.id)}
      <StatusRow
        label={conversion.name}
        status={chineseConversion === conversion.id ? 'green' : 'none'}
        statusText={chineseConversion === conversion.id ? 'Active' : ''}
        onclick={() => selectChineseConversion(conversion.id)}
      />
    {/each}
  </div>

  <SectionHeader label="HISTORY" />
  <div class="section-rows">
    <StatusRow
      label="Save transcription history"
      status={saveHistory ? 'green' : 'none'}
      statusText={saveHistory ? 'On' : 'Off'}
      onclick={toggleSaveHistory}
    />
  </div>

  {#if loading}
    <div class="loading">Updating...</div>
  {/if}
</div>

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
    color: #fca5a5;
  }

  .alert-success {
    background: rgba(34, 197, 94, 0.15);
    color: #86efac;
  }

  .section-rows {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .loading {
    text-align: center;
    padding: 8px;
    color: var(--text-muted);
    font-size: 12px;
  }
</style>
