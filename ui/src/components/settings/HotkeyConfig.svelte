<script>
  import { safeInvoke as invoke } from '../../lib/tauri';
  import { onMount } from 'svelte';

  let currentHotkey = 'Ctrl+`';
  let isRecording = false;
  let recordedKeys = [];
  let loading = false;
  let error = '';
  let success = '';

  onMount(async () => {
    await loadConfig();
  });

  async function loadConfig() {
    try {
      const config = await invoke('get_config');
      currentHotkey = config.hotkey;
    } catch (err) {
      error = `Failed to load config: ${err}`;
      console.error(error);
    }
  }

  function startRecording() {
    isRecording = true;
    recordedKeys = [];
    error = '';
    success = '';
  }

  function cancelRecording() {
    isRecording = false;
    recordedKeys = [];
  }

  function handleKeyDown(event) {
    if (!isRecording) return;

    event.preventDefault();
    event.stopPropagation();

    const modifiers = [];
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

  async function saveHotkey(hotkey) {
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
    } catch (err) {
      error = `Failed to set hotkey: ${err}`;
      console.error(error);
      isRecording = false;
      recordedKeys = [];
    } finally {
      loading = false;
    }
  }

  function getDisplayKeys() {
    if (recordedKeys.length > 0) {
      return recordedKeys.join(' + ');
    }
    return 'Press keys...';
  }
</script>

<svelte:window on:keydown={handleKeyDown} />

<div class="hotkey-config">
  <h2>Global Hotkey</h2>
  <p class="description">
    Set a global keyboard shortcut to start/stop voice transcription from anywhere.
    The hotkey must include at least one modifier key (Cmd, Ctrl, Alt, or Shift).
  </p>

  {#if error}
    <div class="alert alert-error">{error}</div>
  {/if}

  {#if success}
    <div class="alert alert-success">{success}</div>
  {/if}

  <div class="hotkey-section">
    <div class="current-hotkey">
      <label>Current Hotkey:</label>
      <div class="hotkey-display">{currentHotkey}</div>
    </div>

    <div class="hotkey-recorder">
      <label>Record New Hotkey:</label>

      {#if isRecording}
        <div class="recording-box active">
          <span class="recording-indicator">🔴</span>
          <span class="recording-text">{getDisplayKeys()}</span>
        </div>
        <button class="btn btn-secondary" on:click={cancelRecording}>
          Cancel
        </button>
      {:else}
        <button class="btn btn-primary" on:click={startRecording}>
          Record New Hotkey
        </button>
      {/if}
    </div>

    <div class="hotkey-hints">
      <h4>Tips:</h4>
      <ul>
        <li>Click "Record New Hotkey" then press your desired key combination</li>
        <li>Must include at least one modifier (Cmd, Ctrl, Alt, Shift)</li>
        <li>Common examples: Cmd+Shift+Space, Ctrl+Alt+V, Cmd+Ctrl+M</li>
        <li>Changes take effect immediately after recording</li>
      </ul>
    </div>
  </div>

  {#if loading}
    <div class="loading">Updating hotkey...</div>
  {/if}
</div>

<style>
  .hotkey-config {
    padding: 20px;
    max-width: 600px;
  }

  h2 {
    margin-bottom: 8px;
    font-size: 24px;
    font-weight: bold;
    color: #fff;
  }

  .description {
    margin-bottom: 24px;
    color: rgba(255, 255, 255, 0.7);
    font-size: 14px;
    line-height: 1.5;
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

  .hotkey-section {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .current-hotkey,
  .hotkey-recorder {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  label {
    font-size: 14px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.8);
  }

  .hotkey-display {
    padding: 12px 16px;
    background: rgba(59, 130, 246, 0.2);
    border: 2px solid rgba(59, 130, 246, 0.6);
    border-radius: 8px;
    font-family: 'SF Mono', 'Monaco', 'Courier New', monospace;
    font-size: 16px;
    font-weight: 600;
    color: #93c5fd;
    text-align: center;
  }

  .recording-box {
    padding: 12px 16px;
    background: rgba(255, 255, 255, 0.05);
    border: 2px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    min-height: 48px;
  }

  .recording-box.active {
    background: rgba(239, 68, 68, 0.1);
    border-color: rgba(239, 68, 68, 0.5);
    animation: pulse 1.5s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% {
      border-color: rgba(239, 68, 68, 0.5);
    }
    50% {
      border-color: rgba(239, 68, 68, 0.8);
    }
  }

  .recording-indicator {
    font-size: 12px;
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
    font-family: 'SF Mono', 'Monaco', 'Courier New', monospace;
    font-size: 16px;
    font-weight: 600;
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
    margin-top: 8px;
  }

  .btn-secondary:hover {
    background: rgba(255, 255, 255, 0.15);
  }

  .hotkey-hints {
    padding: 16px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .hotkey-hints h4 {
    margin: 0 0 12px 0;
    font-size: 14px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.9);
  }

  .hotkey-hints ul {
    margin: 0;
    padding-left: 20px;
  }

  .hotkey-hints li {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.6);
    line-height: 1.6;
    margin-bottom: 4px;
  }

  .loading {
    text-align: center;
    padding: 12px;
    color: rgba(255, 255, 255, 0.7);
    font-size: 14px;
  }
</style>
