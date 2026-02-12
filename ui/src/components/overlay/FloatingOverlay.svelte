<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import WaveformIndicator from './WaveformIndicator.svelte';
  import TranscriptionView from './TranscriptionView.svelte';

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

  let isDragging = $state(false);
  let dragStartX = $state(0);
  let dragStartY = $state(0);

  // Event listeners cleanup
  let unlistenAudioLevel: UnlistenFn | null = null;
  let unlistenRecordingState: UnlistenFn | null = null;
  let unlistenAudioError: UnlistenFn | null = null;
  let unlistenTranscriptionPartial: UnlistenFn | null = null;
  let unlistenTranscriptionCommitted: UnlistenFn | null = null;
  let unlistenTranscriptionError: UnlistenFn | null = null;

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
        await invoke('stop_recording');
      } else {
        errorMessage = null;
        partialText = '';
        committedText = '';
        await invoke('start_recording');
      }
    } catch (error) {
      console.error('Failed to toggle recording:', error);
      errorMessage = String(error);
    }
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
  });

  onDestroy(() => {
    if (unlistenAudioLevel) unlistenAudioLevel();
    if (unlistenRecordingState) unlistenRecordingState();
    if (unlistenAudioError) unlistenAudioError();
    if (unlistenTranscriptionPartial) unlistenTranscriptionPartial();
    if (unlistenTranscriptionCommitted) unlistenTranscriptionCommitted();
    if (unlistenTranscriptionError) unlistenTranscriptionError();
  });
</script>

<svelte:window
  onmousemove={handleMouseMove}
  onmouseup={handleMouseUp}
/>

<div class="overlay-container">
  <div
    class="overlay-window"
    onmousedown={handleMouseDown}
    role="button"
    tabindex="0"
  >
    <div class="status-indicator">
      <div class="status-dot {isRecording ? (partialText || committedText ? 'transcribing' : 'recording') : (committedText ? 'done' : status.toLowerCase())}"></div>
      <span class="status-text">
        {#if isRecording}
          {partialText || committedText ? 'Transcribing' : 'Recording'}
        {:else if committedText}
          Done
        {:else}
          {status}
        {/if}
      </span>
    </div>

    <div class="app-title">Localtype</div>

    {#if errorMessage}
      <div class="error-message">{errorMessage}</div>
    {/if}

    {#if isRecording}
      <WaveformIndicator rms={audioLevel.rms} voiceActive={audioLevel.voiceActive} />
      <TranscriptionView partialText={partialText} committedText={committedText} />
    {:else if committedText}
      <TranscriptionView partialText={partialText} committedText={committedText} />
    {:else}
      <div class="hint-text">Press Cmd+Shift+Space to start</div>
    {/if}

    <button class="record-button" onclick={toggleRecording}>
      {isRecording ? 'Stop Recording' : 'Start Recording'}
    </button>
  </div>
</div>

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
    background: rgba(30, 30, 30, 0.95);
    backdrop-filter: blur(10px);
    border-radius: 16px;
    padding: 24px 32px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    border: 1px solid rgba(255, 255, 255, 0.1);
    cursor: move;
    min-width: 450px;
    text-align: center;
  }

  .status-indicator {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    margin-bottom: 16px;
  }

  .status-dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    animation: pulse 2s ease-in-out infinite;
  }

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
    color: rgba(255, 255, 255, 0.9);
    font-size: 14px;
    font-weight: 500;
  }

  .app-title {
    font-size: 24px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.95);
    margin-bottom: 16px;
  }

  .hint-text {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.6);
    margin-bottom: 16px;
  }

  .error-message {
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: 8px;
    padding: 12px;
    margin-bottom: 16px;
    color: rgba(239, 68, 68, 0.9);
    font-size: 13px;
    line-height: 1.5;
  }

  .record-button {
    background: rgba(74, 222, 128, 0.2);
    border: 1px solid rgba(74, 222, 128, 0.4);
    color: rgba(74, 222, 128, 0.9);
    padding: 10px 24px;
    border-radius: 8px;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .record-button:hover {
    background: rgba(74, 222, 128, 0.3);
    border-color: rgba(74, 222, 128, 0.6);
    transform: translateY(-1px);
  }

  .record-button:active {
    transform: translateY(0);
  }
</style>
