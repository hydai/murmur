<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { fade, fly, slide } from 'svelte/transition';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import WaveformIndicator from './WaveformIndicator.svelte';
  import TranscriptionView from './TranscriptionView.svelte';
  import SettingsPanel from '../settings/SettingsPanel.svelte';

  interface Props {
    status: string;
  }

  let { status }: Props = $props();

  // State
  let isRecording = $state(false);
  let audioLevel = $state({ rms: 0, voiceActive: false, timestamp_ms: 0 });
  let errorMessage = $state<string | null>(null);
  let partialText = $state('');
  let committedText = $state('');
  let isProcessing = $state(false);
  let processedText = $state('');
  let pipelineState = $state('idle');
  let showCopiedIndicator = $state(false);
  let showSettings = $state(false);
  let detectedCommand = $state<string | null>(null);

  let isDragging = $state(false);
  let dragStartX = $state(0);
  let dragStartY = $state(0);
  let overlayVisible = $state(true);

  // Event listeners cleanup
  let unlistenAudioLevel: UnlistenFn | null = null;
  let unlistenRecordingState: UnlistenFn | null = null;
  let unlistenAudioError: UnlistenFn | null = null;
  let unlistenTranscriptionPartial: UnlistenFn | null = null;
  let unlistenTranscriptionCommitted: UnlistenFn | null = null;
  let unlistenTranscriptionError: UnlistenFn | null = null;
  let unlistenProcessingStatus: UnlistenFn | null = null;
  let unlistenTranscriptionProcessed: UnlistenFn | null = null;
  let unlistenPipelineState: UnlistenFn | null = null;
  let unlistenPipelineResult: UnlistenFn | null = null;
  let unlistenPipelineError: UnlistenFn | null = null;
  let unlistenCommandDetected: UnlistenFn | null = null;

  async function handleMouseDown(e: MouseEvent) {
    isDragging = true;
    const appWindow = getCurrentWindow();

    const position = await appWindow.outerPosition();
    dragStartX = e.clientX - position.x;
    dragStartY = e.clientY - position.y;
  }

  async function handleMouseMove(e: MouseEvent) {
    if (!isDragging) return;

    const appWindow = getCurrentWindow();
    await appWindow.setPosition({
      x: e.screenX - dragStartX,
      y: e.screenY - dragStartY,
    });
  }

  function handleMouseUp() {
    isDragging = false;
  }

  async function toggleRecording() {
    try {
      if (isRecording) {
        await invoke('stop_pipeline');
      } else {
        errorMessage = null;
        partialText = '';
        committedText = '';
        processedText = '';
        isProcessing = false;
        showCopiedIndicator = false;
        detectedCommand = null;
        overlayVisible = true;
        await invoke('start_pipeline');
      }
    } catch (error) {
      console.error('Failed to toggle recording:', error);
      errorMessage = String(error);
    }
  }

  function dismissOverlay() {
    overlayVisible = false;
    setTimeout(() => {
      committedText = '';
      processedText = '';
      partialText = '';
      errorMessage = null;
      overlayVisible = true;
    }, 300);
  }

  function openSettings() {
    showSettings = true;
  }

  function closeSettings() {
    showSettings = false;
  }

  onMount(async () => {
    // Listen for audio level events
    unlistenAudioLevel = await listen('audio-level', (event) => {
      audioLevel = event.payload as { rms: number; voice_active: boolean; timestamp_ms: number };
    });

    // Listen for recording state changes
    unlistenRecordingState = await listen('recording-state', (event) => {
      const payload = event.payload as { is_recording: boolean };
      isRecording = payload.is_recording;
    });

    // Listen for audio errors
    unlistenAudioError = await listen('audio-error', (event) => {
      const payload = event.payload as { message: string };
      errorMessage = payload.message;
      isRecording = false;
    });

    // Listen for transcription events
    unlistenTranscriptionPartial = await listen('transcription-partial', (event) => {
      const payload = event.payload as { text: string };
      partialText = payload.text;
    });

    unlistenTranscriptionCommitted = await listen('transcription-committed', (event) => {
      const payload = event.payload as { text: string };
      // Append to committed text
      if (committedText) {
        committedText += ' ' + payload.text;
      } else {
        committedText = payload.text;
      }
      // Clear partial text when committed
      partialText = '';
    });

    unlistenTranscriptionError = await listen('transcription-error', (event) => {
      const payload = event.payload as { message: string };
      errorMessage = payload.message;
    });

    // Listen for processing status events
    unlistenProcessingStatus = await listen('processing-status', (event) => {
      const payload = event.payload as { status: string };
      isProcessing = payload.status === 'processing';
    });

    // Listen for processed transcription
    unlistenTranscriptionProcessed = await listen('transcription-processed', (event) => {
      const payload = event.payload as { text: string; processing_time_ms: number };
      processedText = payload.text;
      committedText = payload.text; // Update committed text with processed version
      isProcessing = false;
    });

    // Listen for pipeline state changes
    unlistenPipelineState = await listen('pipeline-state', (event) => {
      const payload = event.payload as { state: string; timestamp_ms: number };
      pipelineState = payload.state;

      // Update processing flag based on pipeline state
      isProcessing = payload.state === 'processing';

      console.log('Pipeline state:', payload.state);
    });

    // Listen for pipeline result (final text with clipboard copy)
    unlistenPipelineResult = await listen('pipeline-result', (event) => {
      const payload = event.payload as { text: string; processing_time_ms: number };
      processedText = payload.text;
      committedText = payload.text;

      // Show "Copied!" indicator
      showCopiedIndicator = true;

      // Hide after 2 seconds
      setTimeout(() => {
        showCopiedIndicator = false;
      }, 2000);

      console.log('Pipeline completed:', payload.text.length, 'chars in', payload.processing_time_ms, 'ms');
    });

    // Listen for pipeline errors
    unlistenPipelineError = await listen('pipeline-error', (event) => {
      const payload = event.payload as { message: string; recoverable: boolean };
      errorMessage = payload.message;
      if (!payload.recoverable) {
        isRecording = false;
      }
    });

    // Listen for command detection
    unlistenCommandDetected = await listen('command-detected', (event) => {
      const payload = event.payload as { command_name: string | null; timestamp_ms: number };
      detectedCommand = payload.command_name;
      console.log('Command detected:', payload.command_name);
    });
  });

  onDestroy(() => {
    if (unlistenAudioLevel) unlistenAudioLevel();
    if (unlistenRecordingState) unlistenRecordingState();
    if (unlistenAudioError) unlistenAudioError();
    if (unlistenTranscriptionPartial) unlistenTranscriptionPartial();
    if (unlistenTranscriptionCommitted) unlistenTranscriptionCommitted();
    if (unlistenTranscriptionError) unlistenTranscriptionError();
    if (unlistenProcessingStatus) unlistenProcessingStatus();
    if (unlistenTranscriptionProcessed) unlistenTranscriptionProcessed();
    if (unlistenPipelineState) unlistenPipelineState();
    if (unlistenPipelineResult) unlistenPipelineResult();
    if (unlistenPipelineError) unlistenPipelineError();
    if (unlistenCommandDetected) unlistenCommandDetected();
  });

  // Compute display state based on pipeline state
  $effect(() => {
    let displayState = pipelineState;

    // Override with more specific states
    if (isProcessing) {
      displayState = 'processing';
    } else if (isRecording && (partialText || committedText)) {
      displayState = 'transcribing';
    } else if (isRecording) {
      displayState = 'recording';
    } else if (committedText && !showCopiedIndicator) {
      displayState = 'done';
    }
  });
</script>

<svelte:window
  onmousemove={handleMouseMove}
  onmouseup={handleMouseUp}
/>

<div class="overlay-container">
  {#if overlayVisible}
    <div
      class="overlay-window"
      class:compact={!isRecording && !committedText}
      class:expanded={isRecording || committedText}
      onmousedown={handleMouseDown}
      role="button"
      tabindex="0"
      transition:fade={{ duration: 200 }}
    >
      <div class="header-row">
        <div class="status-indicator">
          {#key pipelineState + isRecording + isProcessing}
            <div
              class="status-dot {isProcessing ? 'processing' : (isRecording ? (partialText || committedText ? 'transcribing' : 'recording') : (committedText ? 'done' : pipelineState))}"
              transition:fade={{ duration: 150 }}
            ></div>
          {/key}
          <span class="status-text">
            {#if isProcessing}
              {#if detectedCommand === 'shorten'}
                Shortening...
              {:else if detectedCommand === 'formalize'}
                Formalizing...
              {:else if detectedCommand === 'casualize'}
                Casualizing...
              {:else if detectedCommand === 'reply'}
                Generating reply...
              {:else if detectedCommand && detectedCommand.startsWith('translate to')}
                {detectedCommand.charAt(0).toUpperCase() + detectedCommand.slice(1)}...
              {:else}
                Processing...
              {/if}
            {:else if isRecording}
              {partialText || committedText ? 'Transcribing' : 'Recording'}
            {:else if committedText}
              Done
            {:else}
              {pipelineState === 'idle' ? 'Ready' : pipelineState.charAt(0).toUpperCase() + pipelineState.slice(1)}
            {/if}
          </span>
        </div>
        <div class="control-buttons">
          <button class="settings-button" onclick={openSettings} title="Settings">
            ⚙
          </button>
          {#if committedText}
            <button class="close-button" onclick={dismissOverlay} title="Dismiss" transition:fly={{ x: 10, duration: 200 }}>
              ✕
            </button>
          {/if}
        </div>
      </div>

      <div class="app-title">Localtype</div>

      {#if showCopiedIndicator}
        <div class="copied-indicator" transition:fly={{ y: -10, duration: 300 }}>
          ✓ Copied to clipboard!
        </div>
      {/if}

      {#if errorMessage}
        <div class="error-message" transition:slide={{ duration: 250 }}>{errorMessage}</div>
      {/if}

      {#if isRecording}
        <div transition:fade={{ duration: 200 }}>
          <WaveformIndicator rms={audioLevel.rms} voiceActive={audioLevel.voiceActive} />
        </div>
        <TranscriptionView partialText={partialText} committedText={committedText} />
      {:else if committedText}
        <div transition:fade={{ duration: 300 }}>
          <TranscriptionView partialText={partialText} committedText={committedText} />
        </div>
      {:else}
        <div class="hint-text" transition:fade={{ duration: 200 }}>Press Cmd+Shift+Space to start</div>
      {/if}

      <button class="record-button" class:recording={isRecording} onclick={toggleRecording}>
        {isRecording ? '⏸ Stop Recording' : '⏺ Start Recording'}
      </button>
    </div>
  {/if}
</div>

<SettingsPanel visible={showSettings} onClose={closeSettings} />

<style>
  .overlay-container {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 20px;
  }

  .overlay-window {
    background: rgba(28, 28, 30, 0.88);
    backdrop-filter: blur(40px) saturate(180%);
    -webkit-backdrop-filter: blur(40px) saturate(180%);
    border-radius: 16px;
    padding: 20px 28px;
    box-shadow:
      0 8px 32px rgba(0, 0, 0, 0.6),
      0 2px 8px rgba(0, 0, 0, 0.4),
      inset 0 1px 0 rgba(255, 255, 255, 0.1);
    border: 1px solid rgba(255, 255, 255, 0.12);
    cursor: move;
    min-width: 400px;
    max-width: 600px;
    text-align: center;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .overlay-window.compact {
    padding: 18px 24px;
    min-width: 380px;
  }

  .overlay-window.expanded {
    padding: 24px 32px;
    min-width: 500px;
  }

  .overlay-window:hover {
    border-color: rgba(255, 255, 255, 0.18);
    box-shadow:
      0 12px 40px rgba(0, 0, 0, 0.7),
      0 4px 12px rgba(0, 0, 0, 0.5),
      inset 0 1px 0 rgba(255, 255, 255, 0.15);
  }

  .header-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
    gap: 12px;
  }

  .status-indicator {
    display: flex;
    align-items: center;
    justify-content: flex-start;
    gap: 10px;
    flex: 1;
  }

  .control-buttons {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .settings-button,
  .close-button {
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: rgba(255, 255, 255, 0.65);
    width: 32px;
    height: 32px;
    border-radius: 8px;
    font-size: 15px;
    cursor: pointer;
    transition: all 0.15s cubic-bezier(0.4, 0, 0.2, 1);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    flex-shrink: 0;
  }

  .settings-button:hover,
  .close-button:hover {
    background: rgba(255, 255, 255, 0.12);
    border-color: rgba(255, 255, 255, 0.22);
    color: rgba(255, 255, 255, 0.95);
    transform: scale(1.05);
  }

  .settings-button:active,
  .close-button:active {
    transform: scale(0.95);
  }

  .close-button {
    color: rgba(255, 99, 99, 0.75);
  }

  .close-button:hover {
    background: rgba(255, 99, 99, 0.15);
    border-color: rgba(255, 99, 99, 0.3);
    color: rgba(255, 99, 99, 1);
  }

  .status-dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    animation: pulse 2s ease-in-out infinite;
  }

  .status-dot.idle,
  .status-dot.ready {
    background: #4ade80;
    box-shadow: 0 0 8px rgba(74, 222, 128, 0.6);
  }

  .status-dot.recording {
    background: #ef4444;
    box-shadow: 0 0 8px rgba(239, 68, 68, 0.8);
    animation: pulse 1s ease-in-out infinite;
  }

  .status-dot.initializing {
    background: #facc15;
    box-shadow: 0 0 8px rgba(250, 204, 21, 0.6);
  }

  .status-dot.transcribing {
    background: #3b82f6;
    box-shadow: 0 0 8px rgba(59, 130, 246, 0.8);
    animation: pulse 1.2s ease-in-out infinite;
  }

  .status-dot.processing {
    background: #8b5cf6;
    box-shadow: 0 0 8px rgba(139, 92, 246, 0.8);
    animation: pulse 1.5s ease-in-out infinite;
  }

  .status-dot.done {
    background: #10b981;
    box-shadow: 0 0 8px rgba(16, 185, 129, 0.6);
  }

  .status-dot.error {
    background: #ef4444;
    box-shadow: 0 0 8px rgba(239, 68, 68, 0.6);
  }

  @keyframes pulse {
    0%, 100% {
      opacity: 1;
    }
    50% {
      opacity: 0.5;
    }
  }

  .status-text {
    color: rgba(255, 255, 255, 0.92);
    font-size: 13px;
    font-weight: 500;
    letter-spacing: 0.01em;
    transition: color 0.2s ease;
  }

  .app-title {
    font-size: 22px;
    font-weight: 600;
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.95) 0%, rgba(255, 255, 255, 0.75) 100%);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
    margin-bottom: 14px;
    letter-spacing: -0.02em;
  }

  .copied-indicator {
    background: rgba(52, 211, 153, 0.18);
    border: 1px solid rgba(52, 211, 153, 0.35);
    border-radius: 10px;
    padding: 10px 16px;
    margin-bottom: 14px;
    color: rgba(52, 211, 153, 1);
    font-size: 13px;
    font-weight: 500;
    box-shadow: 0 4px 12px rgba(52, 211, 153, 0.15);
  }

  .hint-text {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
    margin-bottom: 14px;
    font-weight: 400;
    letter-spacing: 0.01em;
  }

  .error-message {
    background: rgba(248, 113, 113, 0.12);
    border: 1px solid rgba(248, 113, 113, 0.3);
    border-radius: 10px;
    padding: 10px 14px;
    margin-bottom: 14px;
    color: rgba(248, 113, 113, 1);
    font-size: 12px;
    line-height: 1.5;
    box-shadow: 0 2px 8px rgba(248, 113, 113, 0.08);
  }

  .record-button {
    background: rgba(52, 211, 153, 0.15);
    border: 1px solid rgba(52, 211, 153, 0.35);
    color: rgba(52, 211, 153, 1);
    padding: 10px 20px;
    border-radius: 10px;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    box-shadow: 0 2px 8px rgba(52, 211, 153, 0.1);
    min-width: 160px;
  }

  .record-button:hover {
    background: rgba(52, 211, 153, 0.22);
    border-color: rgba(52, 211, 153, 0.5);
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(52, 211, 153, 0.2);
  }

  .record-button:active {
    transform: translateY(0);
  }

  .record-button.recording {
    background: rgba(248, 113, 113, 0.15);
    border-color: rgba(248, 113, 113, 0.35);
    color: rgba(248, 113, 113, 1);
    box-shadow: 0 2px 8px rgba(248, 113, 113, 0.1);
  }

  .record-button.recording:hover {
    background: rgba(248, 113, 113, 0.22);
    border-color: rgba(248, 113, 113, 0.5);
    box-shadow: 0 4px 12px rgba(248, 113, 113, 0.2);
  }
</style>
