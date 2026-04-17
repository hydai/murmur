<script lang="ts">
  import { onMount } from 'svelte';
  import { safeInvoke as invoke } from '../../lib/tauri';
  import PageHeader from './ui/PageHeader.svelte';
  import SectionHeader from './ui/SectionHeader.svelte';

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
  let loading = $state(false);
  let error = $state('');
  let success = $state('');

  let current = $derived<PromptInfo | undefined>(prompts.find((p) => p.name === selectedName));
  let missingPlaceholders = $derived<string[]>(
    current ? current.required_placeholders.filter((ph) => !editorContent.includes(ph)) : []
  );
  let isDirty = $derived<boolean>(current ? editorContent !== current.content : false);
  let isEmpty = $derived<boolean>(editorContent.trim().length === 0);

  onMount(loadPrompts);

  async function loadPrompts() {
    try {
      prompts = await invoke<PromptInfo[]>('get_prompts');
      syncEditor();
    } catch (err) {
      error = `Failed to load prompts: ${err}`;
      console.error(error);
    }
  }

  function syncEditor() {
    editorContent = current?.content ?? '';
    error = '';
    success = '';
  }

  function onSelectChange() {
    syncEditor();
  }

  async function save() {
    if (isEmpty) {
      error = 'Prompt cannot be empty. Type something or click "Reset to default".';
      return;
    }
    try {
      loading = true;
      error = '';
      success = '';
      await invoke('set_prompt', {
        params: { name: selectedName, content: editorContent },
      });
      success = `Saved "${current?.title ?? selectedName}"`;
      await loadPrompts();
      setTimeout(() => {
        success = '';
      }, 3000);
    } catch (err) {
      error = `Failed to save: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }

  async function reset() {
    try {
      loading = true;
      error = '';
      success = '';
      await invoke('reset_prompt', { params: { name: selectedName } });
      success = `Reset "${current?.title ?? selectedName}" to default`;
      await loadPrompts();
      setTimeout(() => {
        success = '';
      }, 3000);
    } catch (err) {
      error = `Failed to reset: ${err}`;
      console.error(error);
    } finally {
      loading = false;
    }
  }
</script>

<div class="page">
  <PageHeader
    title="Prompt Templates"
    description="Edit the Markdown prompts sent to the LLM. Changes take effect on the next recording."
  />

  {#if error}
    <div class="alert alert-error">{error}</div>
  {/if}
  {#if success}
    <div class="alert alert-success">{success}</div>
  {/if}

  <div class="section">
    <SectionHeader label="PROMPT" />
    <select class="prompt-select" bind:value={selectedName} onchange={onSelectChange}>
      {#each prompts as p}
        <option value={p.name}>{p.title}{p.is_override ? ' *' : ''}</option>
      {/each}
    </select>
  </div>

  {#if current}
    <p class="prompt-desc">{current.description}</p>

    <div class="meta-row">
      <span class="task-chip">Task: {current.task_variant}</span>
      {#each current.required_placeholders as ph}
        <span class="ph-chip" class:missing={missingPlaceholders.includes(ph)}>{ph}</span>
      {/each}
    </div>

    {#if missingPlaceholders.length > 0}
      <div class="alert alert-warning">
        Missing required placeholder(s): {missingPlaceholders.join(', ')}. Saving is allowed but
        the LLM call may produce incorrect output because the input text will not be substituted
        into the prompt.
      </div>
    {/if}

    <textarea
      class="prompt-textarea"
      bind:value={editorContent}
      rows="22"
      spellcheck="false"
      placeholder="Type your prompt here..."
    ></textarea>

    <div class="actions">
      <button
        class="btn-secondary"
        onclick={reset}
        disabled={loading || !current.is_override}
        title={current.is_override ? 'Delete the override and revert to the built-in default' : 'No override to reset'}
      >
        Reset to default
      </button>
      <span class="spacer"></span>
      <button
        class="btn-primary"
        onclick={save}
        disabled={loading || !isDirty || isEmpty}
      >
        {loading ? 'Saving...' : 'Save'}
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

  .alert-warning {
    background: rgba(234, 179, 8, 0.12);
    border: 1px solid rgba(234, 179, 8, 0.4);
    color: #fde68a;
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
    color: #fca5a5;
    border-color: rgba(239, 68, 68, 0.5);
    background: rgba(239, 68, 68, 0.1);
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

  .btn-primary,
  .btn-secondary {
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

  .btn-secondary:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.15);
  }

  .btn-secondary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
