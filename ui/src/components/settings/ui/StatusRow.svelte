<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    label,
    value = '',
    status = 'none',
    statusText = '',
    onclick,
    disabled = false,
    children,
  }: {
    label: string;
    value?: string;
    status?: 'green' | 'yellow' | 'red' | 'none';
    statusText?: string;
    onclick?: () => void;
    disabled?: boolean;
    children?: Snippet;
  } = $props();

  const statusColors: Record<string, string> = {
    green: 'var(--status-green)',
    yellow: 'var(--status-yellow)',
    red: 'var(--status-red)',
    none: 'var(--text-muted)',
  };
</script>

{#snippet contents()}
  <span class="dot" style="background: {statusColors[status]}"></span>
  <span class="label">{label}</span>
  <span class="spacer"></span>
  {#if value}<span class="value">{value}</span>{/if}
  {#if statusText}
    <span class="status-text" style="color: {statusColors[status]}">{statusText}</span>
  {/if}
{/snippet}

<div class="status-row">
  {#if onclick}
    <button type="button" class="status-main" {onclick} {disabled}>
      {@render contents()}
    </button>
  {:else}
    <div class="status-main">{@render contents()}</div>
  {/if}
  {#if children}
    <div class="row-actions">{@render children()}</div>
  {/if}
</div>

<style>
  .status-row {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 38px;
    background: var(--bg-card);
    border-radius: 8px;
    width: 100%;
    transition: background 0.15s ease;
  }

  .status-main {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
    min-width: 0;
    height: 100%;
    padding: 0 12px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    text-align: left;
    font: inherit;
  }

  button.status-main { cursor: pointer; }
  button.status-main:hover:not(:disabled) { background: #1a1a2e; }
  button.status-main:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  button.status-main:disabled { opacity: 0.6; cursor: default; }
  .row-actions { display: flex; align-items: center; gap: 8px; padding-right: 12px; }

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
