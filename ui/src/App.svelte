<script lang="ts">
  import { onMount } from 'svelte';
  import { safeInvoke as invoke } from './lib/tauri';
  import FloatingOverlay from './components/overlay/FloatingOverlay.svelte';
  import SettingsPanel from './components/settings/SettingsPanel.svelte';

  const params = new URLSearchParams(window.location.search);
  const view = params.get('view');

  let status = $state('Initializing...');

  onMount(async () => {
    if (view === 'settings') return;
    try {
      status = await invoke<string>('get_status');
    } catch (err) {
      console.error('Failed to get status:', err);
      status = 'Error';
    }
  });

  async function closeSettingsWindow() {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    getCurrentWindow().close();
  }
</script>

{#if view === 'settings'}
  <SettingsPanel visible={true} onClose={closeSettingsWindow} />
{:else}
  <FloatingOverlay {status} />
{/if}
