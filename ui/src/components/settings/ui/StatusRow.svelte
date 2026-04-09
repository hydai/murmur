<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    label,
    value = '',
    status = 'none',
    statusText = '',
    onclick,
    children,
  }: {
    label: string;
    value?: string;
    status?: 'green' | 'yellow' | 'red' | 'none';
    statusText?: string;
    onclick?: () => void;
    children?: Snippet;
  } = $props();

  const statusColors: Record<string, string> = {
    green: 'var(--status-green)',
    yellow: 'var(--status-yellow)',
    red: 'var(--status-red)',
    none: 'var(--text-muted)',
  };
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="status-row"
  class:clickable={!!onclick}
  onclick={onclick}
>
  {#if status !== 'none'}
    <span class="dot" style="background: {statusColors[status]}"></span>
  {:else}
    <span class="dot" style="background: var(--text-muted)"></span>
  {/if}
  <span class="label">{label}</span>
  <span class="spacer"></span>
  {#if value}
    <span class="value">{value}</span>
  {/if}
  {#if statusText}
    <span class="status-text" style="color: {statusColors[status]}">{statusText}</span>
  {/if}
  {#if children}
    {@render children()}
  {/if}
</div>

<style>
  .status-row {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 38px;
    padding: 0 12px;
    background: var(--bg-card);
    border-radius: 8px;
    width: 100%;
    transition: background 0.15s ease;
  }

  .status-row.clickable {
    cursor: pointer;
  }

  .status-row.clickable:hover {
    background: #1a1a2e;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .label {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
    white-space: nowrap;
  }

  .spacer {
    flex: 1;
  }

  .value {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
    white-space: nowrap;
  }

  .status-text {
    font-size: 11px;
    white-space: nowrap;
  }
</style>
