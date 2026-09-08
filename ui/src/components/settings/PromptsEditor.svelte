<script lang="ts">
  import Alert from './ui/Alert.svelte';
  import { useLifecycle } from '../../lib/lifecycle';
  import { createStatus } from '../../lib/status.svelte';
  import { onMount } from 'svelte';
  import { safeInvoke as invoke } from '../../lib/tauri';
  import PageHeader from './ui/PageHeader.svelte';
  import SectionHeader from './ui/SectionHeader.svelte';

  const lifecycle = useLifecycle();
  const status = createStatus(lifecycle);

  interface PromptInfo {
    name: string;
    title: string;
    description: string;
    required_placeholders: string[];
    task_variant: string;
    content: string;
    is_override: boolean;
    default_content: string;
  }

  let prompts = $state<PromptInfo[]>([]);
  let selectedName = $state<string>('post_process');
  let editorContent = $state<string>('');
  // Unsaved edits, kept per prompt so switching the selector never loses work.
  let drafts = $state<Record<string, string>>({});

  let current = $derived<PromptInfo | undefined>(prompts.find((p) => p.name === selectedName));
  let missingPlaceholders = $derived<string[]>(
    current ? current.required_placeholders.filter((ph) => !editorContent.includes(ph)) : []
  );
  let isDirty = $derived<boolean>(current ? editorContent !== current.content : false);
  let isEmpty = $derived<boolean>(editorContent.trim().length === 0);

  onMount(loadPrompts);

  async function loadPrompts() {
    await status.run('Failed to load prompts', async () => {
      prompts = await invoke<PromptInfo[]>('get_prompts');
      syncEditor();
    });
  }

  function syncEditor() {
    editorContent = drafts[selectedName] ?? current?.content ?? '';
  }

  function selectPrompt(next: string) {
    // Switching away used to overwrite the editor from the newly selected
    // prompt, discarding unsaved work with no warning. Park the edit instead,
    // so every prompt keeps its own draft until it is saved or reset.
    if (isDirty) {
      drafts[selectedName] = editorContent;
    } else {
      delete drafts[selectedName];
    }
    selectedName = next;
    // Reloading after a save must keep the banner, so the reset lives with the
    // selection change rather than inside syncEditor.
    status.reset();
    syncEditor();
  }

  async function save() {
    if (isEmpty) {
      status.fail('Prompt cannot be empty. Type something or click "Reset to default".');
      return;
    }
    await status.run('Failed to save', async () => {
      await invoke('set_prompt', {
        params: { name: selectedName, content: editorContent },
      });
      delete drafts[selectedName];
      await loadPrompts();
      status.confirm(`Saved "${current?.title ?? selectedName}"`);
    });
  }

  async function reset() {
    await status.run('Failed to reset', async () => {
      await invoke('reset_prompt', { params: { name: selectedName } });
      delete drafts[selectedName];
      await loadPrompts();
      status.confirm(`Reset "${current?.title ?? selectedName}" to default`);
    });
  }
</script>

<div class="page">
  <PageHeader
    title="Prompt Templates"
    description="Edit the Markdown prompts sent to the LLM. Changes take effect on the next recording."
  />

  <Alert error={status.error} success={status.success} />

  <div class="section">
    <SectionHeader label="PROMPT" />
    <select
      class="prompt-select"
      value={selectedName}
      onchange={(event) => selectPrompt(event.currentTarget.value)}
    >
      {#each prompts as p (p.name)}
        <option value={p.name}>{p.title}{p.is_override ? ' *' : ''}</option>
      {/each}
    </select>
  </div>

  {#if current}
    <p class="prompt-desc">{current.description}</p>

    <div class="meta-row">
      <span class="task-chip">Task: {current.task_variant}</span>
      {#each current.required_placeholders as ph (ph)}
        <span class="ph-chip" class:missing={missingPlaceholders.includes(ph)}>{ph}</span>
      {/each}
    </div>

    <Alert
      warning={missingPlaceholders.length > 0
        ? `Missing required placeholder(s): ${missingPlaceholders.join(', ')}. Saving is allowed but the LLM call may produce incorrect output because the input text will not be substituted into the prompt.`
        : ''}
    />

    <textarea
      class="prompt-textarea"
      bind:value={editorContent}
      rows="22"
      spellcheck="false"
      placeholder="Type your prompt here..."
    ></textarea>

    <div class="actions">
      <button
        class="btn btn-md btn-secondary"
        onclick={reset}
        disabled={status.busy || !current.is_override}
        title={current.is_override ? 'Delete the override and revert to the built-in default' : 'No override to reset'}
      >
        Reset to default
      </button>
      <span class="spacer"></span>
      <button
        class="btn btn-md btn-primary"
        onclick={save}
        disabled={status.busy || !isDirty || isEmpty}
      >
        {status.busy ? 'Saving...' : 'Save'}
      </button>
    </div>
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }





  .section {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 100%;
  }

  .prompt-select {
    width: 100%;
    padding: 8px 12px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--bg-card);
    color: var(--text-primary);
    font-size: 13px;
    font-family: inherit;
    outline: none;
    cursor: pointer;
    transition: border-color 0.15s ease;
  }

  .prompt-select:focus {
    border-color: rgba(168, 85, 247, 0.6);
  }

  .prompt-desc {
    margin: 0;
    color: var(--text-secondary);
    font-size: 12px;
    line-height: 1.4;
  }

  .meta-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }

  .task-chip,
  .ph-chip {
    display: inline-flex;
    align-items: center;
    padding: 3px 8px;
    border-radius: 6px;
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.4;
    border: 1px solid var(--border);
    background: var(--bg-card);
    color: var(--text-secondary);
  }

  .task-chip {
    color: var(--accent);
    border-color: rgba(168, 85, 247, 0.4);
  }

  .ph-chip.missing {
    color: var(--status-red-text);
    border-color: color-mix(in srgb, var(--status-red) 50%, transparent);
    background: color-mix(in srgb, var(--status-red) 10%, transparent);
  }

  .prompt-textarea {
    width: 100%;
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--bg-primary);
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.5;
    outline: none;
    resize: vertical;
    min-height: 320px;
    transition: border-color 0.15s ease;
  }

  .prompt-textarea:focus {
    border-color: rgba(168, 85, 247, 0.6);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 4px;
  }

  .spacer {
    flex: 1;
  }







</style>
