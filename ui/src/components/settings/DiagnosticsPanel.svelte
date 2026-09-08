<script lang="ts">
  import Alert from './ui/Alert.svelte';
  import { useLifecycle } from '../../lib/lifecycle';
  import { createStatus } from '../../lib/status.svelte';
  import { onMount } from 'svelte';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import { safeInvoke as invoke } from '../../lib/tauri';
  import PageHeader from './ui/PageHeader.svelte';
  import SectionHeader from './ui/SectionHeader.svelte';
  import {
    formatDiagnosticLogsForClipboard,
    formatLogTimestamp,
    type DiagnosticLogEntry,
  } from './diagnostics';

  const lifecycle = useLifecycle();
  const status = createStatus(lifecycle);

  let logs = $state<DiagnosticLogEntry[]>([]);

  let newestFirstLogs = $derived([...logs].reverse());

  onMount(() => {
    loadLogs();
  });

  async function loadLogs() {
    await status.run('Failed to load diagnostics', async () => {
      logs = await invoke<DiagnosticLogEntry[]>('get_diagnostic_logs');
    });
  }

  async function clearLogs() {
    await status.run('Failed to clear diagnostics', async () => {
      await invoke('clear_diagnostic_logs');
      logs = [];
      status.confirm('Logs cleared');
    });
  }

  async function copyLogs() {
    await status.run('Failed to copy diagnostics', async () => {
      // Export in the order the panel shows, so the entry the user just
      // read at the top is the first line they paste.
      await writeText(formatDiagnosticLogsForClipboard(newestFirstLogs));
      status.confirm('Diagnostics copied');
    });
  }
</script>

<PageHeader title="Diagnostics" description="Review recent warnings and errors for troubleshooting" />

<Alert error={status.error} success={status.success} />

<div class="section">
  <SectionHeader label="RECENT WARNINGS & ERRORS" />
  <div class="toolbar">
    <button class="tool-btn" onclick={loadLogs} disabled={status.busy}>
      {status.busy ? 'Refreshing...' : 'Refresh'}
    </button>
    <button class="tool-btn" onclick={copyLogs} disabled={status.busy || logs.length === 0}>
      Copy
    </button>
    <button class="tool-btn danger" onclick={clearLogs} disabled={status.busy || logs.length === 0}>
      Clear
    </button>
  </div>

  {#if logs.length === 0}
    <div class="empty-state">
      <span>No warnings or errors recorded in this session.</span>
    </div>
  {:else}
    <div class="log-list">
      <!-- Entries carry no id and the list is replaced wholesale. -->
      {#each newestFirstLogs as log, i (i)}
        <div class="log-row">
          <div class="log-meta">
            <span class:warn={log.level === 'warn'} class:error-level={log.level === 'error'} class="level">
              {log.level.toUpperCase()}
            </span>
            <span class="timestamp">{formatLogTimestamp(log.timestamp_ms)}</span>
            <span class="target">{log.target}</span>
          </div>
          <div class="message">{log.message}</div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .section {
    margin-bottom: 28px;
  }

  .toolbar {
    display: flex;
    gap: 8px;
    margin: 10px 0 12px;
  }

  .tool-btn {
    height: 30px;
    padding: 0 12px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg-card);
    color: var(--text-primary);
    font-size: 12px;
    cursor: pointer;
  }

  .tool-btn:hover:not(:disabled) {
    background: #1a1a2e;
  }

  .tool-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .tool-btn.danger {
    color: var(--status-red);
  }

  .empty-state {
    display: flex;
    align-items: center;
    min-height: 72px;
    padding: 0 14px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg-card);
    color: var(--text-secondary);
    font-size: 13px;
  }

  .log-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .log-row {
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg-card);
  }

  .log-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    margin-bottom: 6px;
  }

  .level {
    flex-shrink: 0;
    min-width: 42px;
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 700;
  }

  .level.warn {
    color: var(--status-yellow);
  }

  .level.error-level {
    color: var(--status-red);
  }

  .timestamp,
  .target {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .target {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .message {
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.45;
    color: var(--text-primary);
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }



</style>
