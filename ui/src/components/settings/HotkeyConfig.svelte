<script lang="ts">
  import Alert from './ui/Alert.svelte';
  import { useLifecycle } from '../../lib/lifecycle';
  import { createStatus } from '../../lib/status.svelte';
  import { safeInvoke as invoke } from '../../lib/tauri';
  import { onMount } from 'svelte';
  import PageHeader from './ui/PageHeader.svelte';
  import SectionHeader from './ui/SectionHeader.svelte';

  const lifecycle = useLifecycle();
  const status = createStatus(lifecycle);

  let currentHotkey = $state('Ctrl+`');
  let isRecording = $state(false);
  let recordedKeys = $state<string[]>([]);

  onMount(async () => {
    await loadConfig();
  });

  async function loadConfig(): Promise<void> {
    await status.run('Failed to load config', async () => {
      const config = await invoke<{ hotkey: string }>('get_config');
      currentHotkey = config.hotkey;
    });
  }

  function startRecording(): void {
    isRecording = true;
    recordedKeys = [];
    status.reset();
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
      status.fail('Hotkey must include at least one modifier key (Cmd, Ctrl, Alt, or Shift)', 4000);
      isRecording = false;
      recordedKeys = [];
      return;
    }

    await status.run('Failed to set hotkey', async () => {
      await invoke('set_hotkey', { hotkey });
      currentHotkey = hotkey;
      status.confirm(`Hotkey updated to: ${hotkey}`);
    });
    // Whether it took or not, the capture is over.
    isRecording = false;
    recordedKeys = [];
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

  <Alert error={status.error} success={status.success} />

  <SectionHeader label="CURRENT SHORTCUT" />
  <div class="hotkey-display">{currentHotkey}</div>

  <SectionHeader label="RECORD NEW" />
  {#if isRecording}
    <div class="recording-box">
      <span class="recording-dot"></span>
      <span class="recording-text">{getDisplayKeys()}</span>
    </div>
    <button class="btn btn-fixed btn-secondary" onclick={cancelRecording}>
      Cancel
    </button>
  {:else}
    <button class="btn btn-fixed btn-primary" onclick={startRecording}>
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

  {#if status.busy}
    <div class="loading">Updating hotkey...</div>
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 12px;
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
      border-color: color-mix(in srgb, var(--status-red) 40%, transparent);
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
    color: var(--status-red-text);
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
