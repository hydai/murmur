<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { Plus } from 'lucide-svelte';
  import { useLifecycle } from '../../lib/lifecycle';
  import { createStatus } from '../../lib/status.svelte';
  import { safeInvoke as invoke } from '../../lib/tauri';
  import Group from '../ui/Group.svelte';
  import Pane from '../ui/Pane.svelte';
  import Row from '../ui/Row.svelte';
  import SearchField from '../ui/SearchField.svelte';
  import Sheet from '../ui/Sheet.svelte';

  const lifecycle = useLifecycle();
  const status = createStatus(lifecycle);

  // What ends an alias: the ASCII comma, the fullwidth comma (U+FF0C) and the ideographic comma (U+3001) an input method types.
  const ALIAS_SEPARATOR = /[,\uFF0C\u3001]/;

  interface DictEntry {
    term: string;
    aliases: string[];
    description: string | null;
  }

  /** The one sheet that is open: a word to add, or a word being edited or deleted. */
  type OpenSheet =
    | { kind: 'add' }
    | { kind: 'edit'; entry: DictEntry }
    | { kind: 'delete'; entry: DictEntry };

  let entries = $state<DictEntry[]>([]);
  let filteredEntries = $state<DictEntry[]>([]);
  let searchQuery = $state('');
  // What is searched for: spaces around it are not part of the word.
  let searchTerm = $derived(searchQuery.trim());
  // False until the backend has answered once, so "No words yet" is only said of a list that is known to be empty.
  let loaded = $state(false);
  let sheet = $state<OpenSheet | null>(null);
  let formData = $state({
    term: '',
    aliases: '',
    description: ''
  });
  // Why the last submit went nowhere. It shows in the sheet, and goes as soon as the word is edited.
  let refusal = $state('');

  /** `WORD_TAKEN` in main.rs, which refuses the same word if this check is ever bypassed. */
  const WORD_TAKEN = 'That word is already in your dictionary.';

  /** Another entry already has `term`, ignoring case. The word being renamed does not count against itself. */
  function wordTaken(term: string, oldTerm: string | null): boolean {
    // An edit that keeps its word adds no copy of it, even where an older dictionary already holds one.
    if (oldTerm === term) return false;
    const renamed = oldTerm === null ? -1 : entries.findIndex((entry) => entry.term === oldTerm);
    const word = term.toLowerCase();
    return entries.some((entry, index) => index !== renamed && entry.term.toLowerCase() === word);
  }
  // The toolbar's Add Word button, for putting the focus back when what opened a sheet is gone.
  let addButton = $state<HTMLButtonElement>();

  onMount(async () => {
    await loadDictionary();
  });

  // filterEntries reads searchTerm and entries, so $effect tracks both.
  $effect(filterEntries);

  async function loadDictionary() {
    await status.run('Failed to load dictionary', async () => {
      const dict = await invoke<{ entries: DictEntry[] }>('get_dictionary');
      entries = dict.entries || [];
      filterEntries();
      loaded = true;
    });
  }

  function filterEntries() {
    if (!searchTerm) {
      filteredEntries = entries;
      return;
    }

    const query = searchTerm.toLowerCase();
    filteredEntries = entries.filter((entry: DictEntry) => {
      return entry.term.toLowerCase().includes(query) ||
             entry.aliases.some((a: string) => a.toLowerCase().includes(query)) ||
             (entry.description && entry.description.toLowerCase().includes(query));
    });
  }

  /** The line under a word: who it is also heard as, or else its note. */
  function entryDetail(entry: DictEntry): string | undefined {
    if (entry.aliases.length > 0) return `Also heard as: ${entry.aliases.join(', ')}`;
    return entry.description || undefined;
  }

  function openAddSheet() {
    formData = { term: '', aliases: '', description: '' };
    refusal = '';
    sheet = { kind: 'add' };
    status.reset();
  }

  function openEditSheet(entry: DictEntry) {
    formData = {
      term: entry.term,
      aliases: entry.aliases.join(', '),
      description: entry.description || ''
    };
    refusal = '';
    sheet = { kind: 'edit', entry };
    status.reset();
  }

  /** From the edit sheet: the confirmation takes its place, so only one sheet is ever open. */
  function openDeleteSheet() {
    if (sheet?.kind !== 'edit') return;
    sheet = { kind: 'delete', entry: sheet.entry };
    status.reset();
  }

  /** Close the sheet. The next one to open starts from nothing typed. */
  function dismissSheet() {
    sheet = null;
    void keepFocus();
  }

  /**
   * A closing sheet gives the focus back to what opened it. A deleted word's row
   * is gone by then, and the focus would fall to the page, so it goes to the
   * toolbar instead, where the keyboard can carry on.
   */
  async function keepFocus() {
    await tick();
    if (!document.activeElement || document.activeElement === document.body) addButton?.focus();
  }

  /** Cancel: the sheet goes, and so does a failure it may have caused. */
  function closeSheet() {
    dismissSheet();
    status.reset();
  }

  /** The shape both add and update send. */
  function entryParams() {
    return {
      term: formData.term.trim(),
      aliases: formData.aliases
        .split(ALIAS_SEPARATOR)
        .map((alias: string) => alias.trim())
        .filter((alias: string) => alias.length > 0),
      description: formData.description.trim() || null,
    };
  }

  async function handleAdd() {
    if (status.busy) return;
    if (!formData.term.trim()) {
      refusal = 'Enter a word.';
      return;
    }
    if (wordTaken(formData.term.trim(), null)) {
      refusal = WORD_TAKEN;
      return;
    }

    await status.run('Failed to add word', async () => {
      await invoke('add_dictionary_entry', { params: entryParams() });
      // A search the new word does not match would leave the list as it was, and the list changing is all the confirmation an add gets.
      searchQuery = '';
      await loadDictionary();
      dismissSheet();
    });
  }

  async function handleEdit() {
    if (status.busy) return;
    if (!formData.term.trim()) {
      refusal = 'Enter a word.';
      return;
    }
    if (sheet?.kind !== 'edit') return;

    const { term: oldTerm } = sheet.entry;
    if (wordTaken(formData.term.trim(), oldTerm)) {
      refusal = WORD_TAKEN;
      return;
    }
    await status.run('Failed to save word', async () => {
      await invoke('update_dictionary_entry', {
        params: { old_term: oldTerm, ...entryParams() },
      });
      await loadDictionary();
      dismissSheet();
    });
  }

  async function handleDelete() {
    if (status.busy) return;
    if (sheet?.kind !== 'delete') return;

    const { term } = sheet.entry;
    await status.run('Failed to delete word', async () => {
      await invoke('delete_dictionary_entry', { term });
      await loadDictionary();
      dismissSheet();
      status.confirm(`Deleted “${term}”`);
    });
  }
</script>

<Pane title="Dictionary" {status}>
  {#snippet actions()}
    <SearchField bind:value={searchQuery} label="Search dictionary" placeholder="Search" />
    <button
      type="button"
      class="add-word"
      aria-label="Add Word"
      title="Add Word"
      bind:this={addButton}
      onclick={openAddSheet}
    >
      <Plus size={15} aria-hidden="true" />
    </button>
  {/snippet}

  {#if loaded}
    {#if entries.length === 0}
      <div class="empty">
        <p class="empty-title">No words yet</p>
        <p class="empty-hint">Add names, jargon, or product terms so Murmur spells them correctly.</p>
        <button type="button" class="btn empty-action" onclick={openAddSheet}>Add Word…</button>
      </div>
    {:else if filteredEntries.length === 0}
      <div class="empty">
        <p class="empty-hint">No words match “{searchTerm}”.</p>
      </div>
    {:else}
      <!--
        Keyed by position and word. The list is read again after every change, and a row that is
        still there must stay, or the focus a sheet gives back to it has nowhere to go. The word
        alone would do, but a dictionary written before duplicates were refused can still hold a
        second copy of one, and a key must not repeat.
      -->
      <Group>
        {#each filteredEntries as entry, index (`${index}:${entry.term}`)}
          <Row label={entry.term} detail={entryDetail(entry)} onclick={() => openEditSheet(entry)} />
        {/each}
      </Group>
    {/if}
  {/if}

  {#if sheet?.kind === 'add' || sheet?.kind === 'edit'}
    {@const editing = sheet.kind === 'edit'}
    {#snippet deleteAction()}
      <!-- Not a choice of the form, and it comes before Save, so it must not be the default button. -->
      <button type="button" class="btn" onclick={openDeleteSheet}>Delete…</button>
    {/snippet}
    <Sheet
      title={editing ? 'Edit Word' : 'Add Word'}
      onclose={closeSheet}
      onsubmit={editing ? handleEdit : handleAdd}
      error={refusal}
      leading={editing ? deleteAction : undefined}
    >
      <label for="term">Word</label>
      <input
        id="term"
        type="text"
        bind:value={formData.term}
        oninput={() => (refusal = '')}
        aria-required="true"
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
      />
      <label for="aliases">Also heard as</label>
      <input
        id="aliases"
        type="text"
        bind:value={formData.aliases}
        aria-describedby="aliases-hint"
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
      />
      <p id="aliases-hint" class="hint">Separate with commas</p>
      <label for="description">Note</label>
      <textarea id="description" bind:value={formData.description} placeholder="Optional" rows="2"></textarea>
      {#snippet actions()}
        <button type="button" class="btn" onclick={closeSheet}>Cancel</button>
        <button type="submit" class="btn btn-primary" disabled={status.busy}>
          {editing ? 'Save' : 'Add Word'}
        </button>
      {/snippet}
    </Sheet>
  {:else if sheet?.kind === 'delete'}
    <!-- The title asks the question, so the sheet has no fields. -->
    <Sheet title={`Delete “${sheet.entry.term}”?`} onclose={closeSheet} onsubmit={handleDelete}>
      {#snippet actions()}
        <button type="button" class="btn" onclick={closeSheet}>Cancel</button>
        <button type="submit" class="btn btn-destructive" disabled={status.busy}>Delete</button>
      {/snippet}
    </Sheet>
  {/if}
</Pane>

<style>
  /* The look of a pane button, as a square that holds only an icon, level with the search field beside it. */
  .add-word {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: 0;
    border-radius: 6px;
    background: var(--control-bg);
    box-shadow: 0 0 0 .5px var(--control-border), 0 .5px 1.5px rgba(0, 0, 0, .14);
    color: var(--text-secondary);
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

  .empty-action {
    margin-top: 8px;
  }

  .hint {
    font-size: 11.5px;
    color: var(--text-secondary);
  }
</style>
