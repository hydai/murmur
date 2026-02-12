<script lang="ts">
  interface Props {
    rms: number;
    voiceActive: boolean;
  }

  let { rms, voiceActive }: Props = $props();

  // Number of bars in the waveform
  const barCount = 20;

  // Generate bar heights based on RMS level
  let barHeights = $derived(
    Array.from({ length: barCount }, (_, i) => {
      // Create a wave pattern with some randomness
      const baseHeight = rms * 100; // Scale RMS (0-1) to 0-100
      const offset = Math.sin((i / barCount) * Math.PI * 2) * 0.3;
      const randomness = Math.random() * 0.2;
      return Math.max(5, baseHeight * (1 + offset + randomness));
    })
  );
</script>

<div class="waveform-container">
  <div class="waveform">
    {#each barHeights as height, i}
      <div
        class="bar"
        class:active={voiceActive}
        style="height: {height}%"
      ></div>
    {/each}
  </div>
  {#if voiceActive}
    <div class="status-text">Listening...</div>
  {:else if rms > 0.001}
    <div class="status-text">Speak louder...</div>
  {:else}
    <div class="status-text">Waiting for audio...</div>
  {/if}
</div>

<style>
  .waveform-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 16px 0;
  }

  .waveform {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    height: 80px;
    width: 100%;
    padding: 0 20px;
  }

  .bar {
    flex: 1;
    min-width: 3px;
    max-width: 8px;
    background: rgba(100, 100, 100, 0.4);
    border-radius: 2px;
    transition: all 0.1s ease-out;
  }

  .bar.active {
    background: linear-gradient(
      to top,
      rgba(74, 222, 128, 0.8),
      rgba(34, 197, 94, 0.6)
    );
    box-shadow: 0 0 8px rgba(74, 222, 128, 0.4);
  }

  .status-text {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.7);
    font-weight: 500;
  }
</style>
