<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    title,
    footer,
    error,
    label,
    children,
  }: {
    title?: string;
    footer?: string;
    /** Shown in place of the footer, so a rejected change is explained next to what it changed. */
    error?: string;
    /** Accessible name of the container, for a group whose title does not say enough. */
    label?: string;
    children: Snippet;
  } = $props();
</script>

<section class="group">
  {#if title}<div class="group-title">{title}</div>{/if}
  <div class="group-body" role={label ? 'group' : undefined} aria-label={label}>
    {@render children()}
  </div>
  {#if error}
    <p class="group-error" role="alert">{error}</p>
  {:else if footer}
    <p class="group-footer">{footer}</p>
  {/if}
</section>

<style>
  .group-title {
    margin-bottom: 7px;
    padding: 0 4px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .group-body {
    overflow: hidden;
    border-radius: 10px;
    background: var(--group-bg);
    box-shadow: 0 0 0 1px var(--separator);
  }

  /* The rows come from the caller's snippet, which this component's scoped styles cannot name. */
  .group-body > :global(* + *) {
    border-top: 1px solid var(--separator);
  }

  .group-footer,
  .group-error {
    margin-top: 6px;
    padding: 0 4px;
    font-size: 11.5px;
  }

  .group-footer {
    color: var(--text-secondary);
  }

  .group-error {
    color: var(--danger);
  }
</style>
