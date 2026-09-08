<script lang="ts">
  interface Props {
    /** Rendered when non-empty. Kept as separate slots so a page can show a
     *  failure and a confirmation at once, as the settings pages already do. */
    error?: string;
    success?: string;
    warning?: string;
  }

  let { error = '', success = '', warning = '' }: Props = $props();
</script>

{#if error}
  <div class="alert alert-error">{error}</div>
{/if}
{#if success}
  <div class="alert alert-success">{success}</div>
{/if}
{#if warning}
  <div class="alert alert-warning">{warning}</div>
{/if}

<style>
  .alert {
    padding: 10px 14px;
    border-radius: 8px;
    font-size: 12px;
  }

  .alert-error {
    background: color-mix(in srgb, var(--status-red) 15%, transparent);
    border: 1px solid color-mix(in srgb, var(--status-red) 40%, transparent);
    color: var(--status-red-text);
  }

  .alert-success {
    background: color-mix(in srgb, var(--status-green) 15%, transparent);
    border: 1px solid color-mix(in srgb, var(--status-green) 40%, transparent);
    color: var(--status-green-text);
  }

  .alert-warning {
    background: color-mix(in srgb, var(--status-yellow) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--status-yellow) 40%, transparent);
    color: var(--status-yellow-text);
  }
</style>
