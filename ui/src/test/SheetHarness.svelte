<script lang="ts">
  import Sheet from '../components/ui/Sheet.svelte';

  let { onclose, onsubmit }: {
    onclose: () => void;
    onsubmit: () => void;
  } = $props();
</script>

<Sheet title="Add Word" {onclose} {onsubmit}>
  <input id="term" />
  {#snippet actions()}
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="submit" class="btn btn-primary">Add Word</button>
  {/snippet}
</Sheet>
