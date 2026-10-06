<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { Copy, Trash2 } from 'lucide-svelte';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import { useLifecycle } from '../../lib/lifecycle';
  import { createStatus } from '../../lib/status.svelte';
  import { safeInvoke as invoke } from '../../lib/tauri';
  import type { PaneId } from '../settings/navigation';
  import Group from '../ui/Group.svelte';
  import Pane from '../ui/Pane.svelte';
  import Row from '../ui/Row.svelte';
  import SearchField from '../ui/SearchField.svelte';
  import Sheet from '../ui/Sheet.svelte';
  import { formatProcessingTime, groupByDay, showsOriginal, type HistoryEntry } from './historyGroups';

  let { onnavigate }: {
    /** Where the pane sends the user, such as General for the setting that turns saving on. */
    onnavigate?: (pane: PaneId) => void;
  } = $props();

  let entries: HistoryEntry[] = $state([]);
  let searchQuery = $state('');
  let loading = $state(false);
  // False until the backend has answered once, so "No transcription history yet" is only said of a history that is known to be empty.
  let loaded = $state(false);
  // Only a history that is known to be off says so; one that could not be asked is taken to be on.
  let savingOff = $state(false);
  let showClearModal = $state(false);
  let expandedId: string | null = $state(null);
  let visibleLimit = $state(50);
  let mutating = $state(false);
  let hasMore = $state(true);
  // The toolbar's search field, for putting the focus back when what held it is gone.
  let searchSlot = $state<HTMLElement>();
  const PAGE_SIZE = 50;

  const MAX_ENTRIES = 500;
  // The interface is English whatever language the system is set to, so the days and the times are too.
  const LOCALE = 'en-US';
  const lifecycle = useLifecycle();
  // Only the toast comes from the shared helper: `loading` here also guards a
  // debounced, cancellable request, which status.run's unconditional reset
  // would break.
  const status = createStatus(lifecycle);
  let requestId = 0;

  // Worked out again whenever the entries change, with the day they are measured from.
  const groups = $derived(groupByDay(entries, new Date(), LOCALE));

  onMount(async () => {
    // Not waited for: the list does not depend on it.
    void loadSavingState();
    await loadHistory();
  });

  async function loadSavingState() {
    try {
      const config = await invoke<{ save_history?: boolean } | null>('get_config');
      if (lifecycle.disposed) return;
      savingOff = config?.save_history === false;
    } catch (err) {
      console.warn('Failed to read the history setting:', err);
    }
  }

  async function loadHistory(limit = PAGE_SIZE) {
    const request = ++requestId;
    const query = searchQuery.trim();
    loading = true;
    status.reset();
    try {
      const result = query
        ? await invoke<HistoryEntry[]>('search_history', { query })
        : await invoke<HistoryEntry[]>('get_history', { offset: 0, limit });
      if (lifecycle.disposed || request !== requestId) return;
      // Replace one consistent snapshot so insertions/deletions cannot shift pages.
      entries = result || [];
      visibleLimit = limit;
      hasMore = !query && entries.length === limit && limit < MAX_ENTRIES;
      loaded = true;
    } catch (err) {
      if (lifecycle.disposed || request !== requestId) return;
      status.fail(`Failed to load history: ${err}`);
    } finally {
      if (!lifecycle.disposed && request === requestId) loading = false;
    }
  }

  async function loadMore() {
    if (loading || searchQuery.trim()) return;
    // The visible limit only advances on success, so retries request the same range.
    await loadHistory(Math.min(visibleLimit + PAGE_SIZE, MAX_ENTRIES));
  }

  function onSearchInput() {
    requestId++;
    loading = true;
    lifecycle.timeout(() => { void loadHistory(); }, 300, 'search');
  }

  async function copyText(text: string) {
    status.reset();
    try {
      await writeText(text);
      status.confirm('Copied');
    } catch (err) {
      status.fail(`Failed to copy: ${err}`);
    }
  }

  async function deleteEntry(id: string) {
    if (loading || mutating) return;
    // A keyboard user is on one of the entry's own buttons, and a deleted entry takes the focus with it.
    const fromList = document.activeElement?.closest('.entry-card') != null;
    mutating = true;
    loading = true;
    requestId++;
    status.reset();
    try {
      await invoke('delete_history_entry', { id });
      if (lifecycle.disposed) return;
      entries = entries.filter(entry => entry.id !== id);
      if (expandedId === id) expandedId = null;
      await loadHistory(visibleLimit);
      status.confirm('Deleted');
    } catch (err) {
      status.fail(`Failed to delete: ${err}`);
    } finally {
      mutating = false;
      loading = false;
    }
    if (fromList) void keepFocus();
  }

  async function clearAll() {
    if (loading || mutating) return;
    mutating = true;
    loading = true;
    requestId++;
    lifecycle.cancelTimeout('search');
    status.reset();
    try {
      await invoke('clear_history');
      if (lifecycle.disposed) return;
      entries = [];
      visibleLimit = PAGE_SIZE;
      hasMore = false;
      expandedId = null;
      showClearModal = false;
      status.confirm('History cleared');
      // Clear… is disabled now, so the sheet cannot give the focus back to it.
      void keepFocus();
    } catch (err) {
      status.fail(`Failed to clear history: ${err}`);
    } finally {
      mutating = false;
      loading = false;
    }
  }

  function openClear() {
    status.reset();
    showClearModal = true;
  }

  /** Cancel: the sheet goes, and so does a failure it may have caused. */
  function closeClear() {
    showClearModal = false;
    status.reset();
  }

  /**
   * What held the focus can be gone, or disabled, when an action ends, and the focus then
   * falls to the page. The search field is the nearest place the keyboard can carry on from.
   */
  async function keepFocus() {
    await tick();
    const active = document.activeElement;
    if (!active || active === document.body || (active instanceof HTMLButtonElement && active.disabled)) {
      searchSlot?.querySelector('input')?.focus();
    }
  }

  function toggleExpand(id: string) {
    expandedId = expandedId === id ? null : id;
  }

  const clock = (timestamp_ms: number) =>
    new Date(timestamp_ms).toLocaleTimeString(LOCALE, { hour: '2-digit', minute: '2-digit' });
</script>

<Pane title="History" {status}>
  {#snippet actions()}
    <!-- The wrapper is how `keepFocus` finds the field. It takes no part in the layout. -->
    <div class="search-slot" bind:this={searchSlot}>
      <SearchField
        bind:value={searchQuery}
        label="Search history"
        placeholder="Search"
        disabled={mutating}
        oninput={onSearchInput}
      />
    </div>
    <button type="button" class="btn btn-small" disabled={entries.length === 0} onclick={openClear}>Clear…</button>
  {/snippet}

  {#if savingOff}
    <Group>
      <Row label="History saving is off" detail="New transcriptions aren't saved.">
        {#snippet trailing()}
          <button type="button" class="btn btn-small" onclick={() => onnavigate?.('general')}>
            Turn On in General
          </button>
        {/snippet}
      </Row>
    </Group>
  {/if}

  {#if loaded}
    {#if entries.length === 0}
      <div class="empty">
        {#if searchQuery.trim()}
          <p class="empty-hint">No transcriptions match your search.</p>
        {:else}
          <p class="empty-title">No transcription history yet.</p>
          <p class="empty-hint">Completed transcriptions will appear here.</p>
        {/if}
      </div>
    {:else}
      {#each groups as group (group.label)}
        <div class="day-section">
          <h2 class="day">{group.label}</h2>
          <Group>
            {#each group.entries as entry (entry.id)}
              {@const original = showsOriginal(entry)}
              <article class="entry-card">
                <p class="entry-text">{entry.final_text}</p>
                <p class="entry-meta">
                  {clock(entry.timestamp_ms)}
                  {#if entry.command_name}
                    · <span class="entry-command">{entry.command_name}</span>
                  {/if}
                  · {formatProcessingTime(entry.processing_time_ms)}
                  {#if original}
                    · <button type="button" class="entry-toggle" onclick={() => toggleExpand(entry.id)}>
                      {expandedId === entry.id ? 'Hide original' : 'Show original'}
                    </button>
                  {/if}
                </p>
                {#if original && expandedId === entry.id}
                  <div class="entry-original">
                    <p class="entry-original-title">Original</p>
                    <p class="entry-original-text">{entry.raw_text}</p>
                  </div>
                {/if}
                <!-- Always in the page, only dimmed: a hidden button could not be reached with the keyboard. -->
                <div class="entry-actions">
                  <button
                    type="button"
                    class="entry-action"
                    title="Copy"
                    aria-label="Copy"
                    onclick={() => copyText(entry.final_text)}
                  >
                    <Copy size={13} aria-hidden="true" />
                  </button>
                  <button
                    type="button"
                    class="entry-action entry-delete"
                    title="Delete"
                    aria-label="Delete"
                    onclick={() => deleteEntry(entry.id)}
                  >
                    <Trash2 size={13} aria-hidden="true" />
                  </button>
                </div>
              </article>
            {/each}
          </Group>
        </div>
      {/each}

      {#if hasMore && !searchQuery.trim()}
        <button type="button" class="btn load-more" onclick={loadMore}>Load More</button>
      {/if}
    {/if}
  {/if}

  {#if showClearModal}
    <Sheet title="Clear all history?" onclose={closeClear} onsubmit={clearAll}>
      <p class="sheet-message">This deletes every saved transcription and can't be undone.</p>
      {#snippet actions()}
        <button type="button" class="btn" onclick={closeClear}>Cancel</button>
        <button type="submit" class="btn btn-destructive" disabled={loading}>Clear History</button>
      {/snippet}
    </Sheet>
  {/if}
</Pane>

<style>
  /* Only there to be found; the search field is laid out as if it stood in the toolbar itself. */
  .search-slot {
    display: contents;
  }

  /* Fills what is left of the pane under the toolbar, with its text in the middle. */
  .empty {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 0 16px 32px;
    text-align: center;
  }

  .empty-title {
    font-size: 13px;
    font-weight: 600;
  }

  .empty-hint {
    max-width: 320px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .day {
    margin-bottom: 7px;
    padding: 0 4px;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
  }

  .entry-card {
    position: relative;
    padding: 9px 12px 10px;
    transition: background-color 0.15s ease;
  }

  .entry-card:hover {
    background: var(--fill-selected);
  }

  .entry-text {
    /* Room for Copy and Delete, which sit over the corner, so the text never runs under them. */
    padding-right: 56px;
    font-size: 13px;
    line-height: 1.4;
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }

  .entry-meta {
    margin-top: 3px;
    font-size: 11.5px;
    line-height: 1.4;
    color: var(--text-secondary);
  }

  .entry-command {
    display: inline-block;
    padding: 0 5px;
    border-radius: 4px;
    background: var(--fill-selected);
    color: var(--text-primary);
    line-height: 16px;
  }

  /* The label is the command as the backend names it, in lower case. */
  .entry-command::first-letter {
    text-transform: uppercase;
  }

  .entry-toggle {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    font: inherit;
  }

  .entry-original {
    margin-top: 8px;
    padding: 8px 10px;
    border-radius: 8px;
    background: var(--window-bg);
  }

  .entry-original-title {
    font-size: 11.5px;
    font-weight: 600;
  }

  .entry-original-text {
    margin-top: 2px;
    font-size: 12px;
    color: var(--text-secondary);
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }

  /* Dimmed, not hidden, until the pointer is over the entry or the keyboard is inside it. */
  .entry-actions {
    position: absolute;
    top: 8px;
    right: 10px;
    display: flex;
    gap: 4px;
    opacity: 0;
    transition: opacity 0.15s ease;
  }

  .entry-card:hover .entry-actions,
  .entry-card:focus-within .entry-actions {
    opacity: 1;
  }

  /* The look of a pane button, as a square that holds only an icon. */
  .entry-action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 0;
    border-radius: 6px;
    background: var(--control-bg);
    box-shadow: 0 0 0 .5px var(--control-border), 0 .5px 1.5px rgba(0, 0, 0, .14);
    color: var(--text-secondary);
  }

  .entry-delete:hover {
    color: var(--danger);
  }

  .load-more {
    /* The pane body is a column that scrolls, and a button that may shrink would be squeezed once the list is long. */
    flex: none;
    align-self: center;
  }

  .sheet-message {
    font-size: 12px;
    color: var(--text-secondary);
  }

  @media (prefers-reduced-motion: reduce) {
    .entry-card,
    .entry-actions {
      transition: none;
    }
  }
</style>
