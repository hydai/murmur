<script lang="ts">
  import StatusRow from '../components/settings/ui/StatusRow.svelte';
  let { select, edit }: { select: () => void; edit: () => void } = $props();
</script>

<StatusRow label="Cloud provider" onclick={select}>
  {#snippet children()}
    <button type="button" onclick={edit}>Edit key</button>
  {/snippet}
</StatusRow>
