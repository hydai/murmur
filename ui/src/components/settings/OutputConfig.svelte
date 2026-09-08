<script lang="ts">
  import Alert from './ui/Alert.svelte';
  import { useLifecycle } from '../../lib/lifecycle';
  import { createStatus } from '../../lib/status.svelte';
  import { safeInvoke as invoke } from '../../lib/tauri';
  import { onMount } from 'svelte';
  import PageHeader from './ui/PageHeader.svelte';
  import SectionHeader from './ui/SectionHeader.svelte';
  import StatusRow from './ui/StatusRow.svelte';

  const lifecycle = useLifecycle();
  const status = createStatus(lifecycle);

  let currentOutputMode = $state('clipboard');
  let saveHistory = $state(true);
  let chineseConversion = $state('traditional');

  const chineseConversions = [
    { id: 'traditional', name: 'Traditional Chinese (Taiwan)' },
    { id: 'none', name: 'No conversion' },
  ];

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
    await status.run('Failed to load config', async () => {
      const config = await invoke<{ output_mode: string; save_history: boolean; chinese_conversion?: string }>('get_config');
      currentOutputMode = config.output_mode.toLowerCase();
      saveHistory = config.save_history !== false;
      chineseConversion = (config.chinese_conversion || 'traditional').toLowerCase();
    });
  }

  async function selectOutputMode(modeId: string): Promise<void> {
    await status.run('Failed to set output mode', async () => {
      await invoke('set_output_mode', { mode: modeId });
      currentOutputMode = modeId;
      const modeName = outputModes.find((m: typeof outputModes[number]) => m.id === modeId)?.name || modeId;
      status.confirm(`Output mode set to: ${modeName}`);
    });
  }

  async function selectChineseConversion(mode: string): Promise<void> {
    await status.run('Failed to set Chinese conversion', async () => {
      await invoke('set_chinese_conversion', { mode });
      chineseConversion = mode;
      const name = chineseConversions.find((c) => c.id === mode)?.name || mode;
      status.confirm(`Chinese output set to: ${name}`);
    });
  }

  async function toggleSaveHistory(): Promise<void> {
    const enabled = !saveHistory;
    await status.run('Failed to update history setting', async () => {
      await invoke('set_save_history', { enabled });
      saveHistory = enabled;
      status.confirm(enabled
        ? 'New transcriptions will be saved to history'
        : 'New transcriptions will not be saved');
    });
  }
</script>

<div class="page">
  <PageHeader title="Output Mode" description="Choose how transcribed text is delivered" />

  <Alert error={status.error} success={status.success} />

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

  {#if status.busy}
    <div class="loading">Updating...</div>
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 12px;
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
