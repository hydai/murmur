<script lang="ts">
  import type { Snippet } from 'svelte';

  /**
   * Transitional: holds a page that has not moved to `Pane` yet. It keeps the
   * old dark look, and gives the page the 52px of window chrome the native
   * title bar no longer provides. Each page leaves it when it migrates.
   */
  let { children }: { children: Snippet } = $props();
</script>

<div class="legacy-pane">
  <div class="drag-strip" data-tauri-drag-region></div>
  <div class="legacy-content">{@render children()}</div>
</div>

<style>
  /* Fills the shell's content column, as Pane does. */
  .legacy-pane {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100vh;
    background: var(--bg-primary);
  }

  /* Where the toolbar goes once the page migrates; here it only lets the window be dragged. */
  .drag-strip {
    flex: none;
    height: 52px;
  }

  .legacy-content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 20px 28px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
</style>
