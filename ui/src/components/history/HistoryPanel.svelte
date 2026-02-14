<script>
  import { safeInvoke as invoke } from '../../lib/tauri';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import { onMount } from 'svelte';

  let entries = [];
  let searchQuery = '';
  let loading = false;
  let error = '';
  let success = '';
  let showClearModal = false;
  let expandedId = null;
  let offset = 0;
  let hasMore = true;
  const PAGE_SIZE = 50;

  onMount(async () => {
    await loadHistory();
  });

  async function loadHistory() {
    try {
      loading = true;
      error = '';
      offset = 0;
      const result = await invoke('get_history', { offset: 0, limit: PAGE_SIZE });
      entries = result || [];
      hasMore = entries.length === PAGE_SIZE;
    } catch (err) {
      error = `Failed to load history: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  async function loadMore() {
    try {
      loading = true;
      offset += PAGE_SIZE;
      const result = await invoke('get_history', { offset, limit: PAGE_SIZE });
      const newEntries = result || [];
      entries = [...entries, ...newEntries];
      hasMore = newEntries.length === PAGE_SIZE;
    } catch (err) {
      error = `Failed to load more: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  async function handleSearch() {
    if (!searchQuery.trim()) {
      await loadHistory();
      return;
    }
    try {
      loading = true;
      error = '';
      const result = await invoke('search_history', { query: searchQuery });
      entries = result || [];
      hasMore = false; // search returns all matches
    } catch (err) {
      error = `Search failed: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  let searchTimer;
  function onSearchInput() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(handleSearch, 300);
  }

  async function copyText(text) {
    try {
      await writeText(text);
      success = 'Copied to clipboard';
      setTimeout(() => { success = ''; }, 2000);
    } catch (err) {
      error = `Failed to copy: ${err}`;
    }
  }

  async function deleteEntry(id) {
    try {
      await invoke('delete_history_entry', { id });
      entries = entries.filter(e => e.id !== id);
      success = 'Entry deleted';
      setTimeout(() => { success = ''; }, 2000);
    } catch (err) {
      error = `Failed to delete: ${err}`;
      console.error(error);
    }
  }

  async function clearAll() {
    try {
      loading = true;
      await invoke('clear_history');
      entries = [];
      showClearModal = false;
      success = 'History cleared';
      setTimeout(() => { success = ''; }, 2000);
    } catch (err) {
      error = `Failed to clear history: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  function toggleExpand(id) {
    expandedId = expandedId === id ? null : id;
  }

  function formatTime(timestamp_ms) {
    const date = new Date(timestamp_ms);
    const now = new Date();
    const diffMs = now - date;
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

    const time = date.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });

    if (diffDays === 0) return `Today ${time}`;
    if (diffDays === 1) return `Yesterday ${time}`;
    if (diffDays < 7) return `${diffDays}d ago ${time}`;
    return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' }) + ` ${time}`;
  }
</script>

<div class="history-panel">
  <div class="header">
    <h2>History</h2>
    {#if entries.length > 0}
      <button class="btn btn-danger-outline" on:click={() => { showClearModal = true; }}>
        Clear All
      </button>
    {/if}
  </div>

  {#if error}
    <div class="alert alert-error">{error}</div>
  {/if}

  {#if success}
    <div class="alert alert-success">{success}</div>
  {/if}

  <div class="search-box">
    <input
      type="text"
      bind:value={searchQuery}
      on:input={onSearchInput}
      placeholder="Search transcriptions..."
      class="search-input"
    />
  </div>

  {#if entries.length === 0 && !loading}
    <div class="empty-state">
      {#if searchQuery.trim()}
        <p>No transcriptions match your search.</p>
      {:else}
        <p>No transcription history yet.</p>
        <p class="hint">Completed transcriptions will appear here.</p>
      {/if}
    </div>
  {:else}
    <div class="entries-list">
      {#each entries as entry (entry.id)}
        <div class="entry-card">
          <div class="entry-header">
            <span class="entry-time">{formatTime(entry.timestamp_ms)}</span>
            <div class="entry-meta">
              {#if entry.command_name}
                <span class="command-badge">{entry.command_name}</span>
              {/if}
              <span class="processing-time">{entry.processing_time_ms}ms</span>
            </div>
          </div>

          <div class="entry-text">{entry.final_text}</div>

          {#if entry.raw_text}
            <button class="btn-link" on:click={() => toggleExpand(entry.id)}>
              {expandedId === entry.id ? 'Hide raw' : 'Show raw transcription'}
            </button>
            {#if expandedId === entry.id}
              <div class="raw-text">{entry.raw_text}</div>
            {/if}
          {/if}

          <div class="entry-actions">
            <button class="btn-icon" on:click={() => copyText(entry.final_text)} title="Copy">
              📋
            </button>
            <button class="btn-icon btn-danger" on:click={() => deleteEntry(entry.id)} title="Delete">
              ✕
            </button>
          </div>
        </div>
      {/each}

      {#if hasMore && !searchQuery.trim()}
        <button class="btn btn-secondary load-more" on:click={loadMore} disabled={loading}>
          {loading ? 'Loading...' : 'Load more'}
        </button>
      {/if}
    </div>
  {/if}
</div>

<!-- Clear All Confirmation Modal -->
{#if showClearModal}
  <div class="modal-overlay" on:click={() => { showClearModal = false; }} on:keypress={(e) => e.key === 'Escape' && (showClearModal = false)} role="presentation">
    <div class="modal modal-small" on:click|stopPropagation role="dialog">
      <h3>Clear All History</h3>
      <p>Are you sure you want to delete all transcription history? This cannot be undone.</p>

      <div class="modal-actions">
        <button class="btn btn-secondary" on:click={() => { showClearModal = false; }}>Cancel</button>
        <button class="btn btn-danger" on:click={clearAll} disabled={loading}>
          {loading ? 'Clearing...' : 'Clear All'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .history-panel {
    padding: 20px;
    max-width: 800px;
    height: 100vh;
    overflow-y: auto;
    box-sizing: border-box;
  }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 20px;
  }

  h2 {
    margin: 0;
    font-size: 24px;
    font-weight: bold;
    color: #fff;
  }

  .alert {
    padding: 12px 16px;
    margin-bottom: 16px;
    border-radius: 8px;
    font-size: 14px;
  }

  .alert-error {
    background: rgba(239, 68, 68, 0.2);
    border: 1px solid rgba(239, 68, 68, 0.5);
    color: #fca5a5;
  }

  .alert-success {
    background: rgba(34, 197, 94, 0.2);
    border: 1px solid rgba(34, 197, 94, 0.5);
    color: #86efac;
  }

  .search-box {
    margin-bottom: 20px;
  }

  .search-input {
    width: 100%;
    padding: 12px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.2);
    background: rgba(0, 0, 0, 0.3);
    color: #fff;
    font-size: 14px;
    box-sizing: border-box;
  }

  .search-input:focus {
    outline: none;
    border-color: rgba(59, 130, 246, 0.6);
  }

  .empty-state {
    text-align: center;
    padding: 60px 20px;
    color: rgba(255, 255, 255, 0.6);
  }

  .empty-state p {
    margin: 8px 0;
  }

  .empty-state .hint {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.4);
  }

  .entries-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding-bottom: 20px;
  }

  .entry-card {
    padding: 16px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    transition: all 0.2s ease;
    position: relative;
  }

  .entry-card:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.2);
  }

  .entry-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
  }

  .entry-time {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
  }

  .entry-meta {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .command-badge {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 10px;
    background: rgba(59, 130, 246, 0.2);
    border: 1px solid rgba(59, 130, 246, 0.4);
    color: #93c5fd;
  }

  .processing-time {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.35);
  }

  .entry-text {
    font-size: 14px;
    color: #fff;
    line-height: 1.6;
    margin-bottom: 8px;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .btn-link {
    background: none;
    border: none;
    color: rgba(59, 130, 246, 0.8);
    font-size: 12px;
    cursor: pointer;
    padding: 0;
    margin-bottom: 8px;
  }

  .btn-link:hover {
    color: #3b82f6;
    text-decoration: underline;
  }

  .raw-text {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.5);
    background: rgba(0, 0, 0, 0.2);
    padding: 10px 12px;
    border-radius: 6px;
    margin-bottom: 8px;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .entry-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  .btn-icon {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.7);
    width: 32px;
    height: 32px;
    border-radius: 6px;
    font-size: 14px;
    cursor: pointer;
    transition: all 0.2s ease;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
  }

  .btn-icon:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }

  .btn-icon.btn-danger:hover {
    background: rgba(239, 68, 68, 0.2);
    border-color: rgba(239, 68, 68, 0.5);
    color: #fca5a5;
  }

  .btn {
    padding: 10px 20px;
    border-radius: 8px;
    border: none;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-secondary {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }

  .btn-secondary:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.15);
  }

  .btn-danger {
    background: rgba(239, 68, 68, 0.3);
    color: #fca5a5;
    border: 1px solid rgba(239, 68, 68, 0.5);
  }

  .btn-danger:hover:not(:disabled) {
    background: rgba(239, 68, 68, 0.4);
  }

  .btn-danger-outline {
    background: transparent;
    color: #fca5a5;
    border: 1px solid rgba(239, 68, 68, 0.4);
  }

  .btn-danger-outline:hover {
    background: rgba(239, 68, 68, 0.15);
  }

  .load-more {
    align-self: center;
  }

  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal {
    background: #1f2937;
    padding: 24px;
    border-radius: 16px;
    max-width: 500px;
    width: 90%;
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .modal-small {
    max-width: 400px;
  }

  .modal h3 {
    margin: 0 0 20px 0;
    font-size: 20px;
    color: #fff;
  }

  .modal p {
    margin: 0 0 20px 0;
    color: rgba(255, 255, 255, 0.7);
    font-size: 14px;
  }

  .modal-actions {
    display: flex;
    gap: 12px;
    justify-content: flex-end;
  }
</style>
