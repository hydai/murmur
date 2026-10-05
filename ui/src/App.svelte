<script lang="ts">
  import FloatingOverlay from './components/overlay/FloatingOverlay.svelte';
  import SettingsPanel from './components/settings/SettingsPanel.svelte';

  // Loaded by both windows; ?view= selects which one this is. Keep it free of
  // anything that needs a capability, since a grant here would have to be made
  // to both. `history` is the old history window's URL: History is a pane of
  // the settings window now, so it gets the same shell.
  const params = new URLSearchParams(window.location.search);
  const view = params.get('view');
</script>

{#if view === 'settings' || view === 'history'}
  <SettingsPanel />
{:else}
  <FloatingOverlay />
{/if}
