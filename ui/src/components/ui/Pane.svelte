<script lang="ts">
  import type { Snippet } from 'svelte';
  import { ChevronLeft } from 'lucide-svelte';
  import Toast from './Toast.svelte';

  let {
    title,
    onback,
    backLabel,
    actions,
    status,
    children,
  }: {
    title: string;
    /** Makes the toolbar start with a back button, for a page one level down. */
    onback?: () => void;
    /** Where the back button goes, spoken as "Back to {backLabel}". */
    backLabel?: string;
    /** Controls at the right end of the toolbar. */
    actions?: Snippet;
    /** What `createStatus` returns. Its messages appear as a toast over the bottom of the pane. */
    status?: { readonly error: string; readonly success: string; reset(): void };
    children: Snippet;
  } = $props();

  const headingId = $props.id();
</script>

<section class="pane ui-v2" aria-labelledby={headingId}>
  <header class="toolbar" data-tauri-drag-region="deep">
    {#if onback}
      <button
        type="button"
        class="back"
        aria-label={backLabel ? `Back to ${backLabel}` : 'Back'}
        onclick={() => onback()}
      >
        <ChevronLeft size={16} aria-hidden="true" />
      </button>
    {/if}
    <h1 id={headingId}>{title}</h1>
    {#if actions}<div class="toolbar-actions">{@render actions()}</div>{/if}
  </header>
  <div class="pane-body">{@render children()}</div>
  <!-- After the scrolling body, not inside it, so the toast stays put while the content moves. -->
  {#if status}
    <Toast success={status.success} error={status.error} ondismiss={() => status.reset()} />
  {/if}
</section>

<style>
  /* As tall as the window, so the toolbar stays put and only the body scrolls. */
  .pane {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--window-bg);
  }

  .toolbar {
    display: flex;
    flex: none;
    align-items: center;
    gap: 8px;
    height: 52px;
    padding: 0 22px;
  }

  h1 {
    min-width: 0;
    overflow: hidden;
    font-size: 15px;
    font-weight: 600;
    line-height: 1.3;
    text-overflow: ellipsis;
    white-space: nowrap;
    /* A drag handle, like a title bar: dragging it should not select the title. */
    -webkit-user-select: none;
    user-select: none;
  }

  .back {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-secondary);
    transition: background-color 0.15s ease;
  }

  .back:hover {
    background: var(--fill-selected);
  }

  .toolbar-actions {
    display: flex;
    flex: none;
    align-items: center;
    gap: 8px;
    margin-left: auto;
  }

  /* The groups are the body's direct children, 18px apart. */
  .pane-body {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: 18px;
    min-height: 0;
    padding: 0 22px 22px;
    overflow-y: auto;
  }

  @media (prefers-reduced-motion: reduce) {
    .back {
      transition: none;
    }
  }
</style>
