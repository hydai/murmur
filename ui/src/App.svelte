<script lang="ts">
  import { onMount } from 'svelte';
  import { safeInvoke as invoke } from './lib/tauri';
  import FloatingOverlay from './components/overlay/FloatingOverlay.svelte';

  let status = $state('Initializing...');

  onMount(async () => {
    try {
      status = await invoke<string>('get_status');
    } catch (err) {
      console.error('Failed to get status:', err);
      status = 'Error';
    }
  });
</script>

<FloatingOverlay {status} />
