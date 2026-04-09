<script lang="ts">
  import { safeInvoke as invoke } from '../../lib/tauri';
  import { onMount } from 'svelte';
  import PageHeader from './ui/PageHeader.svelte';
  import SectionHeader from './ui/SectionHeader.svelte';

  let currentHotkey = $state('Ctrl+`');
  let isRecording = $state(false);
  let recordedKeys = $state<string[]>([]);
  let loading = $state(false);
  let error = $state('');
  let success = $state('');

  onMount(async () => {
    await loadConfig();
  });

  async function loadConfig(): Promise<void> {
    try {
      const config = await invoke<{ hotkey: string }>('get_config');
      currentHotkey = config.hotkey;
    } catch (err: unknown) {
      error = `Failed to load config: ${err}`;
      console.error(error);
    }
  }

  function startRecording(): void {
    isRecording = true;
    recordedKeys = [];
    error = '';
    success = '';
  }

  function cancelRecording(): void {
    isRecording = false;
    recordedKeys = [];
  }

  function handleKeyDown(event: KeyboardEvent): void {
    if (!isRecording) return;

    event.preventDefault();
    event.stopPropagation();

    const modifiers: string[] = [];
    if (event.metaKey) modifiers.push('Cmd');
    if (event.ctrlKey) modifiers.push('Ctrl');
    if (event.altKey) modifiers.push('Alt');
    if (event.shiftKey) modifiers.push('Shift');

    // Get the key (excluding modifiers)
    let key = event.key;
    if (key === 'Meta' || key === 'Control' || key === 'Alt' || key === 'Shift') {
      return; // Ignore modifier-only presses
    }

    // Normalize key name
    if (key === ' ') {
      key = 'Space';
    } else if (key.length === 1) {
      key = key.toUpperCase();
    }

    // Build hotkey string
    const hotkeyParts = [...modifiers, key];
    const hotkey = hotkeyParts.join('+');

    recordedKeys = hotkeyParts;

    // Auto-save after capturing
    saveHotkey(hotkey);
  }

  async function saveHotkey(hotkey: string): Promise<void> {
    // Validate: must have at least one modifier
    const hasModifier = hotkey.includes('Cmd') || hotkey.includes('Ctrl') ||
                       hotkey.includes('Alt') || hotkey.includes('Shift');

    if (!hasModifier) {
      error = 'Hotkey must include at least one modifier key (Cmd, Ctrl, Alt, or Shift)';
      isRecording = false;
      recordedKeys = [];
      setTimeout(() => { error = ''; }, 4000);
      return;
    }

    try {
      loading = true;
      error = '';

      await invoke('set_hotkey', { hotkey });
      currentHotkey = hotkey;
      isRecording = false;
      recordedKeys = [];
      success = `Hotkey updated to: ${hotkey}`;
      setTimeout(() => { success = ''; }, 3000);
    } catch (err: unknown) {
      error = `Failed to set hotkey: ${err}`;
      console.error(error);
      isRecording = false;
      recordedKeys = [];
    } finally {
      loading = false;
    }
  }

  function getDisplayKeys(): string {
    if (recordedKeys.length > 0) {
      return recordedKeys.join(' + ');
    }
    return 'Press keys...';
  }
</script>

<svelte:window onkeydown={handleKeyDown} />

<div class="page">
  <PageHeader title="Hotkey" description="Configure keyboard shortcuts for recording" />

  {#if error}
    <div class="alert alert-error">{error}</div>
  {/if}

  {#if success}
    <div class="alert alert-success">{success}</div>
  {/if}

  <SectionHeader label="CURRENT SHORTCUT" />
  <div class="hotkey-display">{currentHotkey}</div>

  <SectionHeader label="RECORD NEW" />
  {#if isRecording}
    <div class="recording-box">
      <span class="recording-dot"></span>
      <span class="recording-text">{getDisplayKeys()}</span>
    </div>
    <button class="secondary-btn" onclick={cancelRecording}>
      Cancel
    </button>
  {:else}
    <button class="primary-btn" onclick={startRecording}>
      Record New Hotkey
    </button>
  {/if}

  <div class="separator"></div>

  <SectionHeader label="TIPS" />
  <div class="hint-card">
    <ul>
      <li>Click "Record New Hotkey" then press your desired key combination</li>
      <li>Must include at least one modifier (Cmd, Ctrl, Alt, Shift)</li>
      <li>Common examples: Cmd+Shift+Space, Ctrl+Alt+V, Cmd+Ctrl+M</li>
      <li>Changes take effect immediately after recording</li>
    </ul>
  </div>

  {#if loading}
    <div class="loading">Updating hotkey...</div>
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
    color: #fca5a5;
  }

  .alert-success {
    background: rgba(34, 197, 94, 0.15);
    color: #86efac;
  }

  .hotkey-display {
    padding: 12px 14px;
    background: var(--bg-card);
    border: 1px solid var(--accent);
    border-radius: 8px;
    font-family: var(--font-mono);
    font-size: 14px;
    font-weight: 600;
    color: var(--accent);
    text-align: center;
  }

  .recording-box {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 12px 14px;
    background: var(--bg-card);
    border: 1px solid var(--status-red);
    border-radius: 8px;
    min-height: 42px;
    animation: pulse 1.5s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% {
      border-color: var(--status-red);
    }
    50% {
      border-color: rgba(239, 68, 68, 0.4);
    }
  }

  .recording-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--status-red);
    animation: blink 1s ease-in-out infinite;
  }

  @keyframes blink {
    0%, 100% {
      opacity: 1;
    }
    50% {
      opacity: 0.3;
    }
  }

  .recording-text {
    font-family: var(--font-mono);
    font-size: 14px;
    font-weight: 600;
    color: #fca5a5;
  }

  .primary-btn {
    height: 34px;
    padding: 0 16px;
    background: var(--accent);
    color: var(--text-primary);
    border: none;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .primary-btn:hover {
    background: var(--accent-hover);
  }

  .secondary-btn {
    height: 34px;
    padding: 0 16px;
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-primary);
    border: none;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .secondary-btn:hover {
    background: rgba(255, 255, 255, 0.15);
  }

  .separator {
    height: 1px;
    background: var(--border);
    width: 100%;
  }

  .hint-card {
    padding: 12px 14px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 8px;
  }

  .hint-card ul {
    margin: 0;
    padding-left: 18px;
  }

  .hint-card li {
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.7;
  }

  .loading {
    text-align: center;
    padding: 8px;
    color: var(--text-muted);
    font-size: 12px;
  }
</style>
