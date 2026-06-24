<script lang="ts">
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

  let logs = $state<DiagnosticLogEntry[]>([]);
  let loading = $state(false);
  let error = $state('');
  let success = $state('');

  let newestFirstLogs = $derived([...logs].reverse());

  onMount(() => {
    loadLogs();
  });

  async function loadLogs() {
    try {
      loading = true;
      error = '';
      logs = await invoke<DiagnosticLogEntry[]>('get_diagnostic_logs');
    } catch (err) {
      error = `Failed to load diagnostics: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  async function clearLogs() {
    try {
      loading = true;
      error = '';
      success = '';
      await invoke('clear_diagnostic_logs');
      logs = [];
      success = 'Logs cleared';
      setTimeout(() => { success = ''; }, 3000);
    } catch (err) {
      error = `Failed to clear diagnostics: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  async function copyLogs() {
    try {
      error = '';
      success = '';
      await writeText(formatDiagnosticLogsForClipboard(logs));
      success = 'Diagnostics copied';
      setTimeout(() => { success = ''; }, 3000);
    } catch (err) {
      error = `Failed to copy diagnostics: ${err}`;
      console.error(error);
    }
  }
</script>

<PageHeader title="Diagnostics" description="Review recent warnings and errors for troubleshooting" />

{#if error}
  <div class="alert alert-error">{error}</div>
{/if}
{#if success}
  <div class="alert alert-success">{success}</div>
{/if}

<div class="section">
  <SectionHeader label="RECENT WARNINGS & ERRORS" />
  <div class="toolbar">
    <button class="tool-btn" onclick={loadLogs} disabled={loading}>
      {loading ? 'Refreshing...' : 'Refresh'}
    </button>
    <button class="tool-btn" onclick={copyLogs} disabled={loading || logs.length === 0}>
      Copy
    </button>
    <button class="tool-btn danger" onclick={clearLogs} disabled={loading || logs.length === 0}>
      Clear
    </button>
  </div>

  {#if logs.length === 0}
    <div class="empty-state">
      <span>No warnings or errors recorded in this session.</span>
    </div>
  {:else}
    <div class="log-list">
      {#each newestFirstLogs as log}
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

  .alert {
    padding: 10px 12px;
    border-radius: 8px;
    margin-bottom: 12px;
    font-size: 13px;
  }

  .alert-error {
    background: rgba(239, 68, 68, 0.1);
    color: var(--status-red);
    border: 1px solid rgba(239, 68, 68, 0.3);
  }

  .alert-success {
    background: rgba(34, 197, 94, 0.1);
    color: var(--status-green);
    border: 1px solid rgba(34, 197, 94, 0.3);
  }
</style>
