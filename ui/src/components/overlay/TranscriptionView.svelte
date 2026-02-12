<script lang="ts">
  import { fade, fly } from 'svelte/transition';

  interface Props {
    partialText: string;
    committedText: string;
  }

  let { partialText, committedText }: Props = $props();

  // Calculate auto-sizing based on text length
  let containerHeight = $derived(() => {
    const totalLength = committedText.length + partialText.length;
    if (totalLength === 0) return 60;
    if (totalLength < 50) return 70;
    if (totalLength < 150) return 100;
    if (totalLength < 300) return 150;
    return 200;
  });
</script>

<div class="transcription-container" style="min-height: {containerHeight()}px; max-height: {Math.max(containerHeight(), 200)}px;">
  {#if committedText}
    <div class="committed-text" transition:fly={{ y: 5, duration: 300 }}>{committedText}</div>
  {/if}

  {#if partialText}
    <div class="partial-text" transition:fade={{ duration: 200 }}>{partialText}</div>
  {/if}

  {#if !committedText && !partialText}
    <div class="placeholder" transition:fade={{ duration: 200 }}>Listening...</div>
  {/if}
</div>

<style>
  .transcription-container {
    overflow-y: auto;
    padding: 14px 16px;
    background: rgba(0, 0, 0, 0.25);
    border-radius: 10px;
    margin: 12px 0;
    font-family: 'SF Pro Text', -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .committed-text {
    color: rgba(255, 255, 255, 0.95);
    font-size: 14px;
    line-height: 1.65;
    margin-bottom: 6px;
    font-weight: 400;
    letter-spacing: 0.01em;
  }

  .partial-text {
    color: rgba(255, 255, 255, 0.55);
    font-size: 14px;
    line-height: 1.65;
    font-style: italic;
    font-weight: 400;
    animation: pulse 2s ease-in-out infinite;
    letter-spacing: 0.01em;
  }

  .placeholder {
    color: rgba(255, 255, 255, 0.45);
    font-size: 13px;
    font-style: italic;
    text-align: center;
    font-weight: 400;
  }

  @keyframes pulse {
    0%, 100% {
      opacity: 0.55;
    }
    50% {
      opacity: 0.75;
    }
  }

  /* Scrollbar styling */
  .transcription-container::-webkit-scrollbar {
    width: 6px;
  }

  .transcription-container::-webkit-scrollbar-track {
    background: rgba(0, 0, 0, 0.2);
    border-radius: 3px;
  }

  .transcription-container::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.3);
    border-radius: 3px;
  }

  .transcription-container::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.5);
  }
</style>
