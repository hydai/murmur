<script lang="ts">
  import Row from '../components/ui/Row.svelte';

  let { select, edit, current = false, detail }: {
    select: () => void;
    edit: () => void;
    current?: boolean;
    detail?: string;
  } = $props();
</script>

<Row label="Cloud provider" {detail} {current} onclick={select}>
  {#snippet trailing()}
    <button type="button" onclick={edit}>Edit key</button>
  {/snippet}
</Row>
