<script lang="ts">
  let { title, description }: { title: string; description: string } = $props();
</script>

<div class="page-header">
  <h2>{title}</h2>
  <p>{description}</p>
</div>

<style>
  .page-header {
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: 100%;
  }

  h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
    color: var(--text-primary);
  }

  p {
    margin: 0;
    font-size: 12px;
    font-weight: normal;
    color: var(--text-muted);
  }
</style>
