<script lang="ts">
  import { Check, X } from 'lucide-svelte';

  let {
    success,
    error,
    ondismiss,
  }: {
    success?: string;
    /** Wins over `success`, and stays until dismissed. */
    error?: string;
    /** Called by the error's close button. A confirmation clears itself, so it has none. */
    ondismiss: () => void;
  } = $props();
</script>

{#if error}
  <div class="toast toast-error" role="alert">
    <span class="toast-text">{error}</span>
    <!--
      Pressing it with the mouse must not move the focus: a failure that comes from a
      sheet is shown above the sheet, and focus pulled out of it would leave Escape,
      Enter and the Tab trap with nothing to reach. The click still goes through, and
      the keyboard is unaffected.
    -->
    <button
      type="button"
      class="btn-icon toast-dismiss"
      aria-label="Dismiss"
      onmousedown={(event) => event.preventDefault()}
      onclick={() => ondismiss()}
    >
      <X size={14} aria-hidden="true" />
    </button>
  </div>
{:else if success}
  <div class="toast toast-success" role="status">
    <Check size={14} aria-hidden="true" />
    <span class="toast-text">{success}</span>
  </div>
{/if}

<style>
  /*
   * Pinned to the bottom of the pane, so it stays put while the content
   * scrolls, and centred by its auto margins rather than a transform.
   * It stacks above a sheet's backdrop (z-index 20), so a failure that comes
   * from a sheet stays visible, and can be dismissed, while the sheet is open.
   */
  .toast {
    position: absolute;
    right: 0;
    bottom: 16px;
    left: 0;
    z-index: 30;
    display: flex;
    align-items: center;
    gap: 8px;
    width: fit-content;
    max-width: min(560px, calc(100% - 44px));
    min-height: 30px;
    margin-inline: auto;
    padding: 5px 14px;
    border-radius: 15px;
    background: var(--toast-bg);
    box-shadow: 0 4px 14px rgba(0, 0, 0, .18);
    color: var(--toast-fg);
    font-size: 13px;
    line-height: 1.3;
    animation: appear 0.15s ease;
  }

  .toast-error {
    padding-right: 6px;
  }

  .toast :global(svg) {
    flex: none;
  }

  .toast-text {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .toast-dismiss {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    color: inherit;
  }

  .toast-dismiss:hover {
    background: color-mix(in srgb, var(--toast-fg) 18%, transparent);
  }

  /* The page-wide ring is the accent colour, which the toast's own background hides. */
  .toast-dismiss:focus-visible {
    outline-color: var(--toast-fg);
    outline-offset: 1px;
  }

  @keyframes appear {
    from {
      opacity: 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .toast {
      animation: none;
    }
  }
</style>
