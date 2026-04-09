<script lang="ts">
  let {
    placeholder = '',
    value = $bindable(''),
    onsubmit,
  }: {
    placeholder?: string;
    value?: string;
    onsubmit?: (value: string) => void;
  } = $props();

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && onsubmit) {
      onsubmit(value);
    }
  }
</script>

<div class="settings-input">
  <input
    type="text"
    bind:value
    {placeholder}
    onkeydown={handleKeydown}
  />
</div>

<style>
  .settings-input {
    width: 100%;
  }

  input {
    width: 100%;
    height: 36px;
    padding: 0 12px;
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 8px;
    color: var(--text-primary);
    font-size: 12px;
    outline: none;
    transition: border-color 0.15s ease;
  }

  input::placeholder {
    color: var(--text-placeholder);
  }

  input:focus {
    border-color: rgba(168, 85, 247, 0.6);
  }
</style>
