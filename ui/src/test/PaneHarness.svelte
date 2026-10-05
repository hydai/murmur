<script lang="ts">
  import Pane from '../components/ui/Pane.svelte';

  let { onback }: { onback: () => void } = $props();
</script>

<Pane title="Shorten" backLabel="AI Processing" {onback}>
  {#snippet actions()}
    <button type="button" class="btn btn-small">Save</button>
  {/snippet}
  <p>Prompt editor</p>
</Pane>
