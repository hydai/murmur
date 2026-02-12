<script lang="ts">
  import { onMount } from 'svelte';

  interface Props {
    rms: number;
    voiceActive: boolean;
  }

  let { rms, voiceActive }: Props = $props();

  // Number of bars in the waveform
  const barCount = 24;

  // Smoothed RMS value for smoother animations
  let smoothedRms = $state(0);
  let previousBarHeights = $state<number[]>(Array(barCount).fill(5));

  // Smooth RMS changes with exponential moving average
  $effect(() => {
    const alpha = 0.3; // Smoothing factor (0-1, lower = smoother)
    smoothedRms = alpha * rms + (1 - alpha) * smoothedRms;
  });

  // Generate bar heights based on smoothed RMS level
  let barHeights = $derived(
    Array.from({ length: barCount }, (_, i) => {
      // Create a wave pattern with some randomness
      const baseHeight = smoothedRms * 100; // Scale RMS (0-1) to 0-100

      // Create a more organic wave pattern
      const wavePhase = (i / barCount) * Math.PI * 2;
      const wave1 = Math.sin(wavePhase) * 0.3;
      const wave2 = Math.sin(wavePhase * 1.5 + Date.now() * 0.001) * 0.2;
      const offset = wave1 + wave2;

      // Add controlled randomness
      const randomness = (Math.random() - 0.5) * 0.15;

      // Calculate new height
      const targetHeight = Math.max(8, Math.min(100, baseHeight * (1 + offset + randomness)));

      // Smooth transition from previous height
      const previousHeight = previousBarHeights[i] || targetHeight;
      const smoothedHeight = previousHeight * 0.7 + targetHeight * 0.3;

      // Update previous heights
      previousBarHeights[i] = smoothedHeight;

      return smoothedHeight;
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
