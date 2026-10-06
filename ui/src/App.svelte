<script lang="ts">
  import RecordingCapsule from './components/capsule/RecordingCapsule.svelte';
  import SettingsPanel from './components/settings/SettingsPanel.svelte';

  // Loaded by both windows; ?view= selects which one this is. The capsule
  // window calls no commands, so keep this free of anything that needs a
  // capability: a grant here would have to be made to both windows. `history`
  // is the old history window's URL: History is a pane of the settings window
  // now, so it gets the same shell.
  const params = new URLSearchParams(window.location.search);
  const view = params.get('view');
</script>

{#if view === 'settings' || view === 'history'}
  <SettingsPanel />
{:else}
  <RecordingCapsule />
{/if}
