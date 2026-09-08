<script lang="ts">
  import FloatingOverlay from './components/overlay/FloatingOverlay.svelte';
  import SettingsPanel from './components/settings/SettingsPanel.svelte';
  import HistoryPanel from './components/history/HistoryPanel.svelte';

  // Loaded by every window; ?view= selects which one this is. Keep it free of
  // anything that needs a capability, since a grant here would have to be made
  // to all three.
  const params = new URLSearchParams(window.location.search);
  const view = params.get('view');
</script>

{#if view === 'settings'}
  <SettingsPanel visible={true} />
{:else if view === 'history'}
  <HistoryPanel />
{:else}
  <FloatingOverlay />
{/if}
