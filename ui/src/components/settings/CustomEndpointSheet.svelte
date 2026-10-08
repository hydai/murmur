<script lang="ts" module>
  /** What the sheet hands over, as typed. Model and language are empty where the sheet does not ask for them. */
  export interface EndpointDraft {
    baseUrl: string;
    apiKey: string;
    model: string;
    language: string;
    displayName: string;
  }
</script>

<script lang="ts">
  import { untrack } from 'svelte';
  import Sheet from '../ui/Sheet.svelte';

  /**
   * The sheet the Transcription and AI Processing panes use to set up a custom
   * endpoint. It owns the fields and what is typed into them; the page owns
   * what happens to them: checking, saving, closing the sheet by no longer
   * rendering it, and any toast.
   */
  let {
    idPrefix,
    saved,
    baseUrlPlaceholder,
    modelPlaceholder,
    languageHint,
    displayNamePlaceholder,
    busy = false,
    onsave,
    onclose,
  }: {
    /** Starts the id of every field, such as `custom-stt`, so each pane's fields keep ids of their own. */
    idPrefix: string;
    /** What is saved. The fields start from it, all but the API key, which is never shown again. */
    saved: { baseUrl: string; displayName: string; model?: string; language?: string };
    baseUrlPlaceholder: string;
    /** Asks for a model as well, with this placeholder. */
    modelPlaceholder?: string;
    /** Asks for a language as well, with this hint under the field. */
    languageHint?: string;
    displayNamePlaceholder: string;
    /** The page is saving: the button says so. */
    busy?: boolean;
    /** Gets the fields as typed, not trimmed. A rejection is not caught here, so the page reports a failed save itself. */
    onsave: (draft: EndpointDraft) => void | Promise<unknown>;
    /** Cancel and Escape. */
    onclose: () => void;
  } = $props();

  // What is typed stays in the sheet, the key included, so it is gone when the
  // sheet is, and typing changes nothing on the page behind it.
  let draft = $state<EndpointDraft>(
    untrack(() => ({
      baseUrl: saved.baseUrl,
      apiKey: '',
      model: saved.model ?? '',
      language: saved.language ?? '',
      displayName: saved.displayName,
    })),
  );
</script>

<Sheet title="Custom Endpoint" {onclose} onsubmit={() => onsave({ ...draft })}>
  <label for="{idPrefix}-base-url">Base URL</label>
  <input
    id="{idPrefix}-base-url"
    type="text"
    bind:value={draft.baseUrl}
    placeholder={baseUrlPlaceholder}
    aria-required="true"
    autocomplete="off"
    autocapitalize="off"
    spellcheck="false"
  />
  <label for="{idPrefix}-api-key">API Key</label>
  <input
    id="{idPrefix}-api-key"
    type="password"
    bind:value={draft.apiKey}
    placeholder="Only if the server needs one"
    autocomplete="off"
    autocapitalize="off"
    spellcheck="false"
  />
  {#if modelPlaceholder !== undefined}
    <label for="{idPrefix}-model">Model</label>
    <input
      id="{idPrefix}-model"
      type="text"
      bind:value={draft.model}
      placeholder={modelPlaceholder}
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
    />
  {/if}
  {#if languageHint !== undefined}
    <label for="{idPrefix}-language">Language</label>
    <input
      id="{idPrefix}-language"
      type="text"
      bind:value={draft.language}
      placeholder="Automatic"
      aria-describedby="{idPrefix}-language-hint"
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
    />
    <p id="{idPrefix}-language-hint" class="hint">{languageHint}</p>
  {/if}
  <label for="{idPrefix}-display-name">Display Name</label>
  <input
    id="{idPrefix}-display-name"
    type="text"
    bind:value={draft.displayName}
    placeholder={displayNamePlaceholder}
    autocomplete="off"
  />
  {#snippet actions()}
    <button type="button" class="btn" onclick={() => onclose()}>Cancel</button>
    <button type="submit" class="btn btn-primary" disabled={busy || !draft.baseUrl.trim()}>
      {busy ? 'Saving…' : 'Save & Use'}
    </button>
  {/snippet}
</Sheet>

<style>
  .hint {
    font-size: 11.5px;
    color: var(--text-secondary);
  }
</style>
