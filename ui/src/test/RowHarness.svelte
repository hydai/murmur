<script lang="ts">
  import Row from '../components/ui/Row.svelte';

  let { select, edit, current = false, detail, accessory = false }: {
    select: () => void;
    edit: () => void;
    current?: boolean;
    detail?: string;
    /** Gives the row a status at the end of its button, as a list that leads to a page does. */
    accessory?: boolean;
  } = $props();
</script>

{#snippet status()}
  <span class="accessory-content">Edited</span>
{/snippet}

<Row label="Cloud provider" {detail} {current} onclick={select} accessory={accessory ? status : undefined}>
  {#snippet trailing()}
    <button type="button" onclick={edit}>Edit key</button>
  {/snippet}
</Row>
