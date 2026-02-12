<script lang="ts">
  interface Props {
    partialText: string;
    committedText: string;
  }

  let { partialText, committedText }: Props = $props();
</script>

<div class="transcription-container">
  {#if committedText}
    <div class="committed-text">{committedText}</div>
  {/if}

  {#if partialText}
    <div class="partial-text">{partialText}</div>
  {/if}

  {#if !committedText && !partialText}
    <div class="placeholder">Listening...</div>
  {/if}
</div>

<style>
  .transcription-container {
    min-height: 60px;
    max-height: 200px;
    overflow-y: auto;
    padding: 12px;
    background: rgba(0, 0, 0, 0.3);
    border-radius: 8px;
    margin: 16px 0;
    font-family: 'SF Pro Text', -apple-system, system-ui, sans-serif;
  }

  .committed-text {
    color: rgba(255, 255, 255, 0.95);
    font-size: 14px;
    line-height: 1.6;
    margin-bottom: 8px;
    animation: fadeIn 0.3s ease-in-out;
  }

  .partial-text {
    color: rgba(255, 255, 255, 0.5);
    font-size: 14px;
    line-height: 1.6;
    font-style: italic;
    animation: pulse 1.5s ease-in-out infinite;
  }

  .placeholder {
    color: rgba(255, 255, 255, 0.4);
    font-size: 13px;
    font-style: italic;
    text-align: center;
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @keyframes pulse {
    0%, 100% {
      opacity: 0.5;
    }
    50% {
      opacity: 0.8;
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
