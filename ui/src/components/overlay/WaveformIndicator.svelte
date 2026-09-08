<script lang="ts">
  interface Props {
    rms: number;
    voiceActive: boolean;
  }

  let { rms, voiceActive }: Props = $props();

  // Number of bars in the waveform
  const barCount = 24;

  // Derivations stay pure; CSS transitions smooth changes between audio samples.
  let barHeights = $derived(
    Array.from({ length: barCount }, (_, i) => {
      const baseHeight = rms * 100;
      const wavePhase = (i / barCount) * Math.PI * 2;
      const wave1 = Math.sin(wavePhase) * 0.3;
      const wave2 = Math.sin(wavePhase * 1.5) * 0.2;
      return Math.max(8, Math.min(100, baseHeight * (1 + wave1 + wave2)));
    })
  );
</script>

<div class="waveform-container">
  <div class="waveform">
    <!-- Bars are positional, so the index is the key. -->
    {#each barHeights as height, i (i)}
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
    gap: 10px;
    width: 100%;
    padding: 12px 0;
  }

  .waveform {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 3px;
    height: 70px;
    width: 100%;
    padding: 0 24px;
  }

  .bar {
    flex: 1;
    min-width: 3px;
    max-width: 6px;
    background: rgba(120, 120, 128, 0.35);
    border-radius: 3px;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    will-change: height, background, box-shadow;
  }

  .bar.active {
    background: linear-gradient(
      to top,
      rgba(52, 211, 153, 0.9),
      rgba(34, 197, 94, 0.7),
      rgba(52, 211, 153, 0.6)
    );
    box-shadow:
      0 0 10px rgba(52, 211, 153, 0.5),
      0 0 20px rgba(52, 211, 153, 0.2);
  }

  @media (prefers-reduced-motion: no-preference) {
    .bar {
      transition: all 0.15s ease-out;
    }
  }

  .status-text {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.65);
    font-weight: 500;
    letter-spacing: 0.01em;
  }
</style>
