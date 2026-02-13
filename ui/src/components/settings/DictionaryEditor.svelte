<script>
  import { safeInvoke as invoke } from '../../lib/tauri';
  import { onMount } from 'svelte';

  let entries = [];
  let filteredEntries = [];
  let searchQuery = '';
  let showAddModal = false;
  let showEditModal = false;
  let showDeleteModal = false;
  let currentEntry = null;
  let formData = {
    term: '',
    aliases: '',
    description: ''
  };
  let loading = false;
  let error = '';
  let success = '';

  onMount(async () => {
    await loadDictionary();
  });

  async function loadDictionary() {
    try {
      const dict = await invoke('get_dictionary');
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
    filteredEntries = entries.filter(entry => {
      return entry.term.toLowerCase().includes(query) ||
             entry.aliases.some(a => a.toLowerCase().includes(query)) ||
             (entry.description && entry.description.toLowerCase().includes(query));
    });
  }

  $: {
    searchQuery;
    filterEntries();
  }

  function openAddModal() {
    formData = { term: '', aliases: '', description: '' };
    currentEntry = null;
    showAddModal = true;
    error = '';
  }

  function openEditModal(entry) {
    currentEntry = entry;
    formData = {
      term: entry.term,
      aliases: entry.aliases.join(', '),
      description: entry.description || ''
    };
    showEditModal = true;
    error = '';
  }

  function openDeleteModal(entry) {
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
        .map(a => a.trim())
        .filter(a => a.length > 0);

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

    try {
      loading = true;
      error = '';
      success = '';

      const aliases = formData.aliases
        .split(',')
        .map(a => a.trim())
        .filter(a => a.length > 0);

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

<div class="dictionary-editor">
  <div class="header">
    <h2>Personal Dictionary</h2>
    <button class="btn btn-primary" on:click={openAddModal}>
      + Add Entry
    </button>
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
      placeholder="Search dictionary..."
      class="search-input"
    />
  </div>

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
    <div class="entries-list">
      {#each filteredEntries as entry}
        <div class="entry-card">
          <div class="entry-content">
            <h3 class="entry-term">{entry.term}</h3>
            {#if entry.aliases.length > 0}
              <div class="entry-aliases">
                Aliases: {entry.aliases.join(', ')}
              </div>
            {/if}
            {#if entry.description}
              <div class="entry-description">{entry.description}</div>
            {/if}
          </div>
          <div class="entry-actions">
            <button class="btn-icon" on:click={() => openEditModal(entry)} title="Edit">
              ✎
            </button>
            <button class="btn-icon btn-danger" on:click={() => openDeleteModal(entry)} title="Delete">
              ✕
            </button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<!-- Add Modal -->
{#if showAddModal}
  <div class="modal-overlay" on:click={closeModals} on:keypress={(e) => e.key === 'Escape' && closeModals()} role="presentation">
    <div class="modal" on:click|stopPropagation role="dialog">
      <h3>Add Dictionary Entry</h3>

      <div class="form-group">
        <label for="term">Term *</label>
        <input
          id="term"
          type="text"
          bind:value={formData.term}
          placeholder="e.g., Localtype"
          class="form-input"
        />
      </div>

      <div class="form-group">
        <label for="aliases">Aliases (comma-separated)</label>
        <input
          id="aliases"
          type="text"
          bind:value={formData.aliases}
          placeholder="e.g., local type, local-type"
          class="form-input"
        />
      </div>

      <div class="form-group">
        <label for="description">Description (optional)</label>
        <textarea
          id="description"
          bind:value={formData.description}
          placeholder="Optional notes about this term"
          class="form-textarea"
          rows="3"
        ></textarea>
      </div>

      {#if error}
        <div class="alert alert-error">{error}</div>
      {/if}

      <div class="modal-actions">
        <button class="btn btn-secondary" on:click={closeModals}>Cancel</button>
        <button class="btn btn-primary" on:click={handleAdd} disabled={loading}>
          {loading ? 'Adding...' : 'Add Entry'}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Edit Modal -->
{#if showEditModal}
  <div class="modal-overlay" on:click={closeModals} on:keypress={(e) => e.key === 'Escape' && closeModals()} role="presentation">
    <div class="modal" on:click|stopPropagation role="dialog">
      <h3>Edit Dictionary Entry</h3>

      <div class="form-group">
        <label for="edit-term">Term *</label>
        <input
          id="edit-term"
          type="text"
          bind:value={formData.term}
          placeholder="e.g., Localtype"
          class="form-input"
        />
      </div>

      <div class="form-group">
        <label for="edit-aliases">Aliases (comma-separated)</label>
        <input
          id="edit-aliases"
          type="text"
          bind:value={formData.aliases}
          placeholder="e.g., local type, local-type"
          class="form-input"
        />
      </div>

      <div class="form-group">
        <label for="edit-description">Description (optional)</label>
        <textarea
          id="edit-description"
          bind:value={formData.description}
          placeholder="Optional notes about this term"
          class="form-textarea"
          rows="3"
        ></textarea>
      </div>

      {#if error}
        <div class="alert alert-error">{error}</div>
      {/if}

      <div class="modal-actions">
        <button class="btn btn-secondary" on:click={closeModals}>Cancel</button>
        <button class="btn btn-primary" on:click={handleEdit} disabled={loading}>
          {loading ? 'Updating...' : 'Update Entry'}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Delete Confirmation Modal -->
{#if showDeleteModal}
  <div class="modal-overlay" on:click={closeModals} on:keypress={(e) => e.key === 'Escape' && closeModals()} role="presentation">
    <div class="modal modal-small" on:click|stopPropagation role="dialog">
      <h3>Delete Entry</h3>
      <p>Are you sure you want to delete "{currentEntry?.term}"?</p>

      {#if error}
        <div class="alert alert-error">{error}</div>
      {/if}

      <div class="modal-actions">
        <button class="btn btn-secondary" on:click={closeModals}>Cancel</button>
        <button class="btn btn-danger" on:click={handleDelete} disabled={loading}>
          {loading ? 'Deleting...' : 'Delete'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .dictionary-editor {
    padding: 20px;
    max-width: 800px;
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
  }

  .entry-card {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    padding: 16px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    transition: all 0.2s ease;
  }

  .entry-card:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.2);
  }

  .entry-content {
    flex: 1;
  }

  .entry-term {
    margin: 0 0 8px 0;
    font-size: 18px;
    font-weight: 600;
    color: #fff;
  }

  .entry-aliases {
    font-size: 13px;
    color: rgba(59, 130, 246, 0.9);
    margin-bottom: 6px;
  }

  .entry-description {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.6);
    line-height: 1.5;
  }

  .entry-actions {
    display: flex;
    gap: 8px;
    margin-left: 16px;
  }

  .btn-icon {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.7);
    width: 32px;
    height: 32px;
    border-radius: 6px;
    font-size: 16px;
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

  .btn-primary {
    background: #3b82f6;
    color: #fff;
  }

  .btn-primary:hover:not(:disabled) {
    background: #2563eb;
  }

  .btn-secondary {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
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
    max-height: 90vh;
    overflow-y: auto;
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

  .form-group {
    margin-bottom: 16px;
  }

  .form-group label {
    display: block;
    margin-bottom: 6px;
    color: rgba(255, 255, 255, 0.9);
    font-size: 13px;
    font-weight: 500;
  }

  .form-input,
  .form-textarea {
    width: 100%;
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.2);
    background: rgba(0, 0, 0, 0.3);
    color: #fff;
    font-size: 14px;
    font-family: inherit;
  }

  .form-input:focus,
  .form-textarea:focus {
    outline: none;
    border-color: rgba(59, 130, 246, 0.6);
  }

  .form-textarea {
    resize: vertical;
  }

  .modal-actions {
    display: flex;
    gap: 12px;
    justify-content: flex-end;
    margin-top: 20px;
  }
</style>
