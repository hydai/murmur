<script lang="ts">
  import { Search } from 'lucide-svelte';

  let {
    value = $bindable(''),
    label,
    placeholder,
    disabled = false,
    oninput,
  }: {
    value?: string;
    /** The accessible name. The placeholder is only a hint and goes away as soon as the user types. */
    label: string;
    placeholder?: string;
    disabled?: boolean;
    oninput?: (value: string) => void;
  } = $props();

  function report(event: Event & { currentTarget: HTMLInputElement }) {
    value = event.currentTarget.value;
    oninput?.(value);
  }
</script>

<div class="search">
  <span class="search-icon"><Search size={13} aria-hidden="true" /></span>
  <input type="search" aria-label={label} {placeholder} {disabled} {value} oninput={report} />
</div>

<style>
  .search {
    position: relative;
    flex: none;
    width: 170px;
    max-width: 100%;
  }

  .search-icon {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 8px;
    display: inline-flex;
    align-items: center;
    color: var(--text-secondary);
    pointer-events: none;
  }

  input {
    width: 100%;
    height: 26px;
    padding: 0 8px 0 27px;
    border: 0;
    border-radius: 6px;
    background: var(--fill-selected);
    color: var(--text-primary);
    font: inherit;
    font-size: 13px;
    /* Keep the engine's own search-field chrome out of the way of the rounded field above. */
    appearance: none;
  }

  input::placeholder {
    color: var(--text-secondary);
  }

  input:disabled {
    opacity: 0.45;
  }
</style>
