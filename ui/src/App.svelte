<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import FloatingOverlay from './components/overlay/FloatingOverlay.svelte';
  import SettingsPanel from './components/settings/SettingsPanel.svelte';
  import HistoryPanel from './components/history/HistoryPanel.svelte';

  const params = new URLSearchParams(window.location.search);
  const view = params.get('view');

  async function closeSettingsWindow() {
    try {
      await getCurrentWindow().close();
    } catch (error) {
      console.warn('Failed to close settings window:', error);
    }
  }
</script>

{#if view === 'settings'}
  <SettingsPanel visible={true} onClose={closeSettingsWindow} />
{:else if view === 'history'}
  <HistoryPanel />
{:else}
  <FloatingOverlay />
{/if}
