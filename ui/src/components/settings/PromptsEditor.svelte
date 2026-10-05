<script lang="ts">
  import { onMount } from 'svelte';
  import { useLifecycle } from '../../lib/lifecycle';
  import { createStatus } from '../../lib/status.svelte';
  import { safeInvoke as invoke } from '../../lib/tauri';
  import { voiceCommand, type PromptName } from '../../lib/voiceCommands';
  import Pane from '../ui/Pane.svelte';
  import { clearDraft, getDraft, setDraft } from './promptDrafts.svelte';

  /**
   * The editor of one voice command's prompt, a page below AI Processing. It
   * has a status of its own, so its toasts end with it and the next prompt that
   * is opened starts clean.
   */
  let { name, onback }: { name: PromptName; onback: () => void } = $props();

  const lifecycle = useLifecycle();
  const status = createStatus(lifecycle);
  const introId = $props.id();

  /** The part of what `get_prompts` lists that the editor uses. */
  interface PromptInfo {
    name: PromptName;
    description: string;
    required_placeholders: string[];
    content: string;
    is_override: boolean;
  }

  /**
   * The placeholders that stand for what was said. A prompt without one never
   * sees the transcription.
   */
  const TRANSCRIPTION_PLACEHOLDERS = ['{raw_text}', '{text}', '{context}'];

  /**
   * What the model goes without when one of the other placeholders is missing.
   * Each prompt has at most one of them.
   */
  const LEFT_OUT: Record<string, string> = {
    '{dictionary_terms}': "Your dictionary terms won't be sent to the model.",
    '{tone}': "The tone you ask for won't be sent to the model.",
    '{language}': "The language you ask for won't be sent to the model.",
  };

  let prompts = $state<PromptInfo[]>([]);

  let command = $derived(voiceCommand(name));
  let current = $derived(prompts.find((prompt) => prompt.name === name));
  // What the editor shows: the unsaved draft if there is one, otherwise what is stored.
  let content = $derived(getDraft(name) ?? current?.content ?? '');
  let missing = $derived(
    current ? current.required_placeholders.filter((placeholder) => !content.includes(placeholder)) : [],
  );
  // The backend does not refuse a prompt without a placeholder, it leaves that value out,
  // so the warning says which value the model will not get.
  let missingNote = $derived(describeMissing(missing));
  let isDirty = $derived(current ? content !== current.content : false);
  let isEmpty = $derived(content.trim().length === 0);

  /**
   * The line under the editor for the placeholders the text lacks, in the order the prompt requires them,
   * and then everything the model goes without because of them; empty when none are missing.
   */
  function describeMissing(names: string[]): string {
    if (names.length === 0) return '';
    const consequences: string[] = [];
    // Said once, however many of the transcription's placeholders are missing.
    if (names.some((name) => TRANSCRIPTION_PLACEHOLDERS.includes(name))) {
      consequences.push("Your transcription won't be inserted into the prompt.");
    }
    for (const name of names) {
      if (TRANSCRIPTION_PLACEHOLDERS.includes(name)) continue;
      // A placeholder this build has no wording for would otherwise print "undefined".
      consequences.push(LEFT_OUT[name] ?? "It won't be sent to the model.");
    }
    return `Missing ${names.join(', ')}. ${consequences.join(' ')}`;
  }

  onMount(() => {
    void load();
  });

  async function refresh() {
    prompts = await invoke<PromptInfo[]>('get_prompts');
  }

  async function load() {
    await status.run('Failed to load prompts', refresh);
  }

  /** A draft exists exactly while the text differs from what is stored, so "Unsaved" never lingers on a reverted edit. */
  function edit(text: string) {
    if (current && text !== current.content) setDraft(name, text);
    else clearDraft(name);
  }

  // After a change the prompts are read back before the draft goes, so the editor
  // never shows the old stored text in between.
  async function save() {
    if (!current || status.busy) return;
    // The button is off for a blank prompt; this only guards a click that gets past it.
    if (isEmpty) {
      status.fail("Prompt can't be empty. Type something or restore the default.");
      return;
    }
    const text = content;
    await status.run('Failed to save', async () => {
      await invoke('set_prompt', { params: { name, content: text } });
      await refresh();
      // Typing goes on while the save runs. What was typed after it is not saved, so it stays a draft.
      if (getDraft(name) === text) clearDraft(name);
      status.confirm('Saved');
    });
  }

  async function restore() {
    if (!current || status.busy) return;
    const before = getDraft(name);
    await status.run('Failed to restore', async () => {
      await invoke('reset_prompt', { params: { name } });
      await refresh();
      // Typing goes on while the restore runs. The restore replaces what was there when it started, so text typed
      // after that stays a draft, unless it is the restored prompt itself and there is nothing left to save.
      const after = getDraft(name);
      if (after === before || after === current?.content) clearDraft(name);
      status.confirm('Restored the default prompt');
    });
  }
</script>

<Pane title={command.title} {onback} backLabel="AI Processing" {status}>
  {#if current}
    <div class="editor">
      <p class="editor-intro" id={introId}>{current.description} {command.usage}</p>
      <textarea
        value={content}
        oninput={(event) => edit(event.currentTarget.value)}
        aria-label="{command.title} prompt"
        aria-describedby={introId}
        spellcheck="false"
        placeholder="Type your prompt here…"
      ></textarea>
      <div class="editor-footer">
        <div class="editor-notes">
          {#if current.required_placeholders.length > 0}
            <p class="editor-required">
              <span>Required:</span>
              {#each current.required_placeholders as placeholder (placeholder)}
                <code class:is-missing={missing.includes(placeholder)}>{placeholder}</code>
              {/each}
            </p>
          {/if}
          <!-- Always there, so that the text appearing in it is announced. -->
          <p class="editor-missing" role="status">{missingNote}</p>
        </div>
        <div class="editor-actions">
          <button
            type="button"
            class="btn"
            disabled={status.busy || !current.is_override}
            onclick={restore}
          >
            Restore Default
          </button>
          <button
            type="button"
            class="btn btn-primary"
            disabled={status.busy || !isDirty || isEmpty}
            onclick={save}
          >
            Save
          </button>
        </div>
      </div>
    </div>
  {/if}
</Pane>

<style>
  /* Fills what the pane leaves, and never shrinks below its content: the pane's body scrolls instead. */
  .editor {
    display: flex;
    flex: 1 0 auto;
    flex-direction: column;
    gap: 12px;
  }

  .editor-intro {
    font-size: 11.5px;
    line-height: 1.4;
    color: var(--text-secondary);
  }

  textarea {
    flex: 1 1 auto;
    width: 100%;
    min-height: 140px;
    padding: 10px 12px;
    border: 1px solid var(--control-border);
    border-radius: 6px;
    background: var(--field-bg);
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.5;
    resize: none;
    transition: border-color 0.15s ease;
  }

  textarea::placeholder {
    color: var(--text-secondary);
  }

  /* A soft ring in place of the page-wide outline, like the sheet's fields. */
  textarea:focus-visible {
    border-color: var(--accent);
    outline: 3px solid color-mix(in srgb, var(--accent) 30%, transparent);
    outline-offset: 0;
  }

  .editor-footer {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: 8px 12px;
  }

  .editor-notes {
    flex: 1 1 200px;
    min-width: 0;
  }

  /* The label and each placeholder are separate items, 6px apart, however many there are. */
  .editor-required {
    display: flex;
    flex-wrap: wrap;
    gap: 0 6px;
    font-size: 12px;
    line-height: 1.4;
    color: var(--text-secondary);
  }

  code {
    font-family: var(--font-mono);
    font-size: inherit;
  }

  .is-missing {
    color: var(--danger);
  }

  .editor-missing {
    font-size: 11.5px;
    line-height: 1.4;
    color: var(--danger);
  }

  .editor-missing:not(:empty) {
    margin-top: 4px;
  }

  .editor-actions {
    display: flex;
    flex: none;
    gap: 8px;
    margin-left: auto;
  }

  @media (prefers-reduced-motion: reduce) {
    textarea {
      transition: none;
    }
  }
</style>
