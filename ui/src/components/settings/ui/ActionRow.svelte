<script lang="ts">
  import { Plus } from 'lucide-svelte';

  let {
    label,
    icon,
    onclick,
  }: {
    label: string;
    icon?: any;
    onclick?: () => void;
  } = $props();

  const IconComponent = $derived(icon ?? Plus);
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="action-row" class:clickable={!!onclick} onclick={onclick}>
  <IconComponent size={14} color="var(--text-muted)" />
  <span class="label">{label}</span>
</div>

<style>
  .action-row {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 38px;
    padding: 0 12px;
    background: var(--bg-card);
    border-radius: 8px;
    border: 1px solid var(--border);
    width: 100%;
    transition: background 0.15s ease;
  }

  .action-row.clickable {
    cursor: pointer;
  }

  .action-row.clickable:hover {
    background: #1a1a2e;
  }

  .label {
    font-size: 12px;
    color: var(--text-muted);
  }
</style>
