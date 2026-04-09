<script lang="ts">
  import { safeInvoke as invoke } from '../../lib/tauri';
  import { onMount } from 'svelte';
  import { BookPlus } from 'lucide-svelte';
  import PageHeader from './ui/PageHeader.svelte';
  import SectionHeader from './ui/SectionHeader.svelte';
  import ActionRow from './ui/ActionRow.svelte';

  interface DictEntry {
    term: string;
    aliases: string[];
    description: string | null;
  }

  let entries = $state<DictEntry[]>([]);
  let filteredEntries = $state<DictEntry[]>([]);
  let searchQuery = $state('');
  let showAddModal = $state(false);
  let showEditModal = $state(false);
  let showDeleteModal = $state(false);
  let currentEntry = $state<DictEntry | null>(null);
  let formData = $state({
    term: '',
    aliases: '',
    description: ''
  });
  let loading = $state(false);
  let error = $state('');
  let success = $state('');

  onMount(async () => {
    await loadDictionary();
  });

  $effect(() => {
    // Re-filter whenever searchQuery changes
    searchQuery;
    filterEntries();
  });

  async function loadDictionary() {
    try {
      const dict = await invoke<{ entries: DictEntry[] }>('get_dictionary');
      entries = dict.entries || [];
      filterEntries();
    } catch (err) {
      error = `Failed to load dictionary: ${err}`;
      console.error(error);
    }
  }

  function filterEntries() {
    if (!searchQuery.trim()) {
      filteredEntries = entries;
      return;
    }

    const query = searchQuery.toLowerCase();
    filteredEntries = entries.filter((entry: DictEntry) => {
      return entry.term.toLowerCase().includes(query) ||
             entry.aliases.some((a: string) => a.toLowerCase().includes(query)) ||
             (entry.description && entry.description.toLowerCase().includes(query));
    });
  }

  function openAddModal() {
    formData = { term: '', aliases: '', description: '' };
    currentEntry = null;
    showAddModal = true;
    error = '';
  }

  function openEditModal(entry: DictEntry) {
    currentEntry = entry;
    formData = {
      term: entry.term,
      aliases: entry.aliases.join(', '),
      description: entry.description || ''
    };
    showEditModal = true;
    error = '';
  }

  function openDeleteModal(entry: DictEntry) {
    currentEntry = entry;
    showDeleteModal = true;
    error = '';
  }

  function closeModals() {
    showAddModal = false;
    showEditModal = false;
    showDeleteModal = false;
    currentEntry = null;
    error = '';
  }

  async function handleAdd() {
    if (!formData.term.trim()) {
      error = 'Term cannot be empty';
      return;
    }

    try {
      loading = true;
      error = '';
      success = '';

      const aliases = formData.aliases
        .split(',')
        .map((a: string) => a.trim())
        .filter((a: string) => a.length > 0);

      await invoke('add_dictionary_entry', {
        params: {
          term: formData.term.trim(),
          aliases,
          description: formData.description.trim() || null
        }
      });

      success = `Added "${formData.term}"`;
      await loadDictionary();
      closeModals();
      setTimeout(() => { success = ''; }, 3000);
    } catch (err) {
      error = `Failed to add entry: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  async function handleEdit() {
    if (!formData.term.trim()) {
      error = 'Term cannot be empty';
      return;
    }

    if (!currentEntry) return;

    try {
      loading = true;
      error = '';
      success = '';

      const aliases = formData.aliases
        .split(',')
        .map((a: string) => a.trim())
        .filter((a: string) => a.length > 0);

      await invoke('update_dictionary_entry', {
        params: {
          old_term: currentEntry.term,
          term: formData.term.trim(),
          aliases,
          description: formData.description.trim() || null
        }
      });

      success = `Updated "${formData.term}"`;
      await loadDictionary();
      closeModals();
      setTimeout(() => { success = ''; }, 3000);
    } catch (err) {
      error = `Failed to update entry: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  async function handleDelete() {
    if (!currentEntry) return;

    try {
      loading = true;
      error = '';
      success = '';

      await invoke('delete_dictionary_entry', {
        term: currentEntry.term
      });

      success = `Deleted "${currentEntry.term}"`;
      await loadDictionary();
      closeModals();
      setTimeout(() => { success = ''; }, 3000);
    } catch (err) {
      error = `Failed to delete entry: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }
</script>

<div class="page">
  <PageHeader title="Dictionary" description="Manage custom words and phrase corrections" />

  {#if error && !showAddModal && !showEditModal && !showDeleteModal}
    <div class="alert alert-error">{error}</div>
  {/if}
  {#if success}
    <div class="alert alert-success">{success}</div>
  {/if}

  <!-- SEARCH -->
  <div class="search-row">
    <input
      type="text"
      bind:value={searchQuery}
      placeholder="Search dictionary..."
      class="search-input"
    />
  </div>

  <!-- ENTRIES -->
  <div class="section">
    <SectionHeader label="ENTRIES ({filteredEntries.length})" />
    <div class="entries-list">
      {#if filteredEntries.length === 0}
        <div class="empty-state">
          {#if entries.length === 0}
            <p>No dictionary entries yet.</p>
            <p class="hint">Add custom terms to improve transcription accuracy.</p>
          {:else}
            <p>No entries match your search.</p>
          {/if}
        </div>
      {:else}
        {#each filteredEntries as entry}
          <div class="entry-row">
            <div class="entry-info">
              <span class="entry-term">{entry.term}</span>
              {#if entry.aliases.length > 0}
                <span class="entry-aliases">{entry.aliases.join(', ')}</span>
              {/if}
            </div>
            <div class="entry-actions">
              <button class="icon-btn" onclick={() => openEditModal(entry)} title="Edit">✎</button>
              <button class="icon-btn danger" onclick={() => openDeleteModal(entry)} title="Delete">✕</button>
            </div>
          </div>
        {/each}
      {/if}
    </div>
  </div>

  <!-- ADD ENTRY -->
  <div class="section">
    <SectionHeader label="ADD ENTRY" />
    <ActionRow label="Add new word or correction" icon={BookPlus} onclick={openAddModal} />
  </div>
</div>

<!-- Add Modal -->
{#if showAddModal}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-overlay" onclick={closeModals} onkeydown={(e) => e.key === 'Escape' && closeModals()} role="presentation">
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" tabindex="-1">
      <h3>Add Dictionary Entry</h3>

      <div class="form-group">
        <label for="term">Term *</label>
        <input id="term" type="text" bind:value={formData.term} placeholder="e.g., Murmur" />
      </div>

      <div class="form-group">
        <label for="aliases">Aliases (comma-separated)</label>
        <input id="aliases" type="text" bind:value={formData.aliases} placeholder="e.g., local type, local-type" />
      </div>

      <div class="form-group">
        <label for="description">Description (optional)</label>
        <textarea id="description" bind:value={formData.description} placeholder="Optional notes about this term" rows="3"></textarea>
      </div>

      {#if error}
        <div class="alert alert-error">{error}</div>
      {/if}

      <div class="modal-actions">
        <button class="btn-secondary" onclick={closeModals}>Cancel</button>
        <button class="btn-primary" onclick={handleAdd} disabled={loading}>
          {loading ? 'Adding...' : 'Add Entry'}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Edit Modal -->
{#if showEditModal}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-overlay" onclick={closeModals} onkeydown={(e) => e.key === 'Escape' && closeModals()} role="presentation">
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog" tabindex="-1">
      <h3>Edit Dictionary Entry</h3>

      <div class="form-group">
        <label for="edit-term">Term *</label>
        <input id="edit-term" type="text" bind:value={formData.term} placeholder="e.g., Murmur" />
      </div>

      <div class="form-group">
        <label for="edit-aliases">Aliases (comma-separated)</label>
        <input id="edit-aliases" type="text" bind:value={formData.aliases} placeholder="e.g., local type, local-type" />
      </div>

      <div class="form-group">
        <label for="edit-description">Description (optional)</label>
        <textarea id="edit-description" bind:value={formData.description} placeholder="Optional notes about this term" rows="3"></textarea>
      </div>

      {#if error}
        <div class="alert alert-error">{error}</div>
      {/if}

      <div class="modal-actions">
        <button class="btn-secondary" onclick={closeModals}>Cancel</button>
        <button class="btn-primary" onclick={handleEdit} disabled={loading}>
          {loading ? 'Updating...' : 'Update Entry'}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Delete Confirmation Modal -->
{#if showDeleteModal}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-overlay" onclick={closeModals} onkeydown={(e) => e.key === 'Escape' && closeModals()} role="presentation">
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="modal modal-small" onclick={(e) => e.stopPropagation()} role="dialog" tabindex="-1">
      <h3>Delete Entry</h3>
      <p>Are you sure you want to delete "{currentEntry?.term}"?</p>

      {#if error}
        <div class="alert alert-error">{error}</div>
      {/if}

      <div class="modal-actions">
        <button class="btn-secondary" onclick={closeModals}>Cancel</button>
        <button class="btn-danger" onclick={handleDelete} disabled={loading}>
          {loading ? 'Deleting...' : 'Delete'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .alert {
    padding: 10px 14px;
    border-radius: 8px;
    font-size: 12px;
  }

  .alert-error {
    background: rgba(239, 68, 68, 0.15);
    border: 1px solid rgba(239, 68, 68, 0.4);
    color: #fca5a5;
  }

  .alert-success {
    background: rgba(34, 197, 94, 0.15);
    border: 1px solid rgba(34, 197, 94, 0.4);
    color: #86efac;
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 100%;
  }

  /* Search */
  .search-row {
    width: 100%;
  }

  .search-input {
    width: 100%;
    padding: 8px 12px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--bg-card);
    color: var(--text-primary);
    font-size: 12px;
    outline: none;
    transition: border-color 0.15s ease;
  }

  .search-input:focus {
    border-color: rgba(168, 85, 247, 0.6);
  }

  .search-input::placeholder {
    color: var(--text-placeholder);
  }

  /* Entries */
  .entries-list {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .entry-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    background: var(--bg-card);
    border-radius: 8px;
    min-height: 38px;
    transition: background 0.15s ease;
  }

  .entry-row:hover {
    background: #1a1a2e;
  }

  .entry-info {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
    min-width: 0;
  }

  .entry-term {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
    white-space: nowrap;
  }

  .entry-aliases {
    font-size: 11px;
    color: var(--accent);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .entry-actions {
    display: flex;
    gap: 4px;
    margin-left: 10px;
  }

  .icon-btn {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid var(--border);
    color: var(--text-muted);
    width: 26px;
    height: 26px;
    border-radius: 6px;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.15s ease;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
  }

  .icon-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-primary);
  }

  .icon-btn.danger:hover {
    background: rgba(239, 68, 68, 0.2);
    border-color: rgba(239, 68, 68, 0.5);
    color: #fca5a5;
  }

  .empty-state {
    text-align: center;
    padding: 32px 16px;
    color: var(--text-muted);
    font-size: 13px;
  }

  .empty-state p {
    margin: 4px 0;
  }

  .empty-state .hint {
    font-size: 12px;
    color: var(--text-placeholder);
  }

  /* Modal shared */
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
    background: var(--bg-card);
    padding: 24px;
    border-radius: 16px;
    max-width: 480px;
    width: 90%;
    border: 1px solid var(--border);
    max-height: 90vh;
    overflow-y: auto;
  }

  .modal-small {
    max-width: 380px;
  }

  .modal h3 {
    margin: 0 0 16px;
    font-size: 16px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .modal p {
    margin: 0 0 16px;
    color: var(--text-muted);
    font-size: 13px;
  }

  .form-group {
    margin-bottom: 12px;
  }

  .form-group label {
    display: block;
    margin-bottom: 4px;
    color: var(--text-secondary);
    font-size: 12px;
    font-weight: 500;
  }

  .form-group input,
  .form-group textarea {
    width: 100%;
    padding: 8px 12px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--bg-primary);
    color: var(--text-primary);
    font-size: 13px;
    font-family: inherit;
    outline: none;
    transition: border-color 0.15s ease;
  }

  .form-group input:focus,
  .form-group textarea:focus {
    border-color: rgba(168, 85, 247, 0.6);
  }

  .form-group textarea {
    resize: vertical;
  }

  .modal-actions {
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    margin-top: 16px;
  }

  .btn-primary, .btn-secondary, .btn-danger {
    padding: 8px 16px;
    border-radius: 8px;
    border: none;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-primary {
    background: var(--accent);
    color: var(--text-primary);
  }

  .btn-primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-secondary {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-primary);
  }

  .btn-secondary:hover {
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

  .btn-danger:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
