<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import { useLifecycle } from '../../lib/lifecycle';
  import { createStatus } from '../../lib/status.svelte';
  import { safeInvoke as invoke } from '../../lib/tauri';
  import Group from '../ui/Group.svelte';
  import Pane from '../ui/Pane.svelte';
  import {
    formatDiagnosticLogsForClipboard,
    formatLogTimestamp,
    type DiagnosticLogEntry,
  } from './diagnostics';

  /**
   * The log of warnings and errors, a page below About. It has a status of its
   * own, so its toasts end with it.
   */
  let { onback }: { onback: () => void } = $props();

  const lifecycle = useLifecycle();
  const status = createStatus(lifecycle);

  let logs = $state<DiagnosticLogEntry[]>([]);
  // False until the backend has answered once, so "No warnings or errors" is only said of a log that is known to be empty.
  let loaded = $state(false);
  // For putting the focus back once the buttons that need a log go off.
  let refreshButton = $state<HTMLButtonElement>();

  let newestFirstLogs = $derived([...logs].reverse());

  onMount(() => {
    void loadLogs();
  });

  // The buttons are not disabled while one of these runs: a disabled button that
  // holds the focus lets it fall to the page, so a keyboard user would lose their
  // place. A second press is held off here instead.
  async function loadLogs() {
    if (status.busy) return;
    await status.run('Failed to load diagnostics', async () => {
      logs = (await invoke<DiagnosticLogEntry[]>('get_diagnostic_logs')) ?? [];
      loaded = true;
    });
  }

  async function clearLogs() {
    if (status.busy) return;
    const cleared = await status.run('Failed to clear diagnostics', async () => {
      await invoke('clear_diagnostic_logs');
      logs = [];
      status.confirm('Log cleared');
      return true;
    });
    // A log that could not be cleared leaves Clear on, with the focus where it was.
    if (cleared) void focusRefresh();
  }

  /** Clear and Copy go off with the log, and the focus goes with the button that held it. Refresh is what is left to carry on from. */
  async function focusRefresh() {
    await tick();
    refreshButton?.focus();
  }

  async function copyLogs() {
    if (status.busy) return;
    await status.run('Failed to copy diagnostics', async () => {
      // Export in the order the panel shows, so the entry the user just
      // read at the top is the first line they paste.
      await writeText(formatDiagnosticLogsForClipboard(newestFirstLogs));
      status.confirm('Copied');
    });
  }

  /** The backend sends `warn` and `error`; a row says Warning and Error, and any other level as it is, capitalized. */
  function levelLabel(level: string): string {
    if (level === 'warn') return 'Warning';
    return level.charAt(0).toUpperCase() + level.slice(1);
  }
</script>

<Pane title="Diagnostics Log" {onback} backLabel="About" {status}>
  {#snippet actions()}
    <button type="button" class="btn btn-small" bind:this={refreshButton} onclick={loadLogs}>Refresh</button>
    <button type="button" class="btn btn-small" disabled={logs.length === 0} onclick={copyLogs}>Copy</button>
    <button type="button" class="btn btn-small" disabled={logs.length === 0} onclick={clearLogs}>Clear</button>
  {/snippet}

  {#if loaded}
    {#if logs.length === 0}
      <div class="empty">
        <p class="empty-text">No warnings or errors in this session.</p>
      </div>
    {:else}
      <Group>
        <!-- Entries carry no id and the list is replaced wholesale. -->
        {#each newestFirstLogs as log, i (i)}
          <article class="log-row">
            <p class="log-meta">
              <span class="level" class:level-warn={log.level === 'warn'} class:level-error={log.level === 'error'}>{levelLabel(log.level)}</span>
              <span class="timestamp">{formatLogTimestamp(log.timestamp_ms)}</span>
              <span class="target">{log.target}</span>
            </p>
            <p class="message">{log.message}</p>
          </article>
        {/each}
      </Group>
    {/if}
  {/if}
</Pane>

<style>
  /* Fills what is left of the pane under the toolbar, with its text in the middle. */
  .empty {
    display: flex;
    flex: 1 1 auto;
    align-items: center;
    justify-content: center;
    padding: 0 16px 32px;
    text-align: center;
  }

  .empty-text {
    font-size: 12px;
    color: var(--text-secondary);
  }

  .log-row {
    padding: 9px 12px 10px;
  }

  .log-meta {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
    margin-bottom: 3px;
  }

  .level {
    flex: none;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
  }

  .level-warn {
    color: var(--warning);
  }

  .level-error {
    color: var(--danger);
  }

  .timestamp,
  .target {
    font-size: 11.5px;
    color: var(--text-secondary);
    white-space: nowrap;
  }

  .target {
    min-width: 0;
    overflow: hidden;
    font-family: var(--font-mono);
    text-overflow: ellipsis;
  }

  .message {
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.45;
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }
</style>
