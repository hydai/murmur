<script lang="ts">
  let { label }: { label: string } = $props();
</script>

<div class="section-header">
  <span class="marker">+</span>
  <span class="label">{label}</span>
</div>

<style>
  .section-header {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .marker {
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 700;
    color: var(--accent);
  }

  .label {
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: normal;
    letter-spacing: 1.5px;
    color: var(--text-secondary);
    text-transform: uppercase;
  }
</style>
