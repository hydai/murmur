<script lang="ts">
  import { Eye, EyeOff } from 'lucide-svelte';
  import Sheet from '../ui/Sheet.svelte';

  /**
   * The sheet every settings page uses to take an API key. It owns how the key
   * is entered; the page owns what happens to it: saving, closing the sheet by
   * no longer rendering it, and any toast.
   */
  let {
    providerName,
    mode,
    busy = false,
    error = '',
    onsave,
    onclose,
  }: {
    /** The service the key is for; the title names it. */
    providerName: string;
    /** `add` for a service that has no key yet, `change` for one that has. */
    mode: 'add' | 'change';
    /** The page is saving: the button says so, and a second submit is ignored. */
    busy?: boolean;
    /** What the page has to say about the last save, shown under the field. */
    error?: string;
    /**
     * Gets the key as it was typed, not trimmed. A rejection is not caught
     * here, so the page reports a failed save itself.
     */
    onsave: (key: string) => void | Promise<unknown>;
    /** Cancel and Escape. */
    onclose: () => void;
  } = $props();

  const fieldId = $props.id();

  // The key stays in the sheet, so it is gone when the sheet is.
  let key = $state('');
  let visible = $state(false);
  // Why the last submit went nowhere; it clears as soon as the key is edited.
  let missing = $state('');

  function save(): void | Promise<unknown> {
    if (busy) return;
    if (!key.trim()) {
      missing = 'API key cannot be empty';
      return;
    }
    return onsave(key);
  }
</script>

<Sheet
  title="{mode === 'change' ? 'Change' : 'Add'} API Key for {providerName}"
  {onclose}
  onsubmit={save}
  error={missing || error}
>
  <label for={fieldId}>API Key</label>
  <div class="key-field">
    <input
      id={fieldId}
      type={visible ? 'text' : 'password'}
      bind:value={key}
      oninput={() => (missing = '')}
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
    />
    <button
      type="button"
      class="btn-icon"
      aria-label={visible ? 'Hide API key' : 'Show API key'}
      onclick={() => (visible = !visible)}
    >
      {#if visible}
        <EyeOff size={15} aria-hidden="true" />
      {:else}
        <Eye size={15} aria-hidden="true" />
      {/if}
    </button>
  </div>
  {#snippet actions()}
    <button type="button" class="btn" onclick={() => onclose()}>Cancel</button>
    <button type="submit" class="btn btn-primary" disabled={busy}>
      {busy ? 'Saving…' : mode === 'change' ? 'Save' : 'Save & Use'}
    </button>
  {/snippet}
</Sheet>

<style>
  /* The key field and its show/hide button share the row the sheet gives the field. */
  .key-field {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .key-field input {
    flex: 1;
    min-width: 0;
  }
</style>
