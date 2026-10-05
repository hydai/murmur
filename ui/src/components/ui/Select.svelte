<script lang="ts">
  let {
    value,
    options,
    label,
    disabled = false,
    onchange,
  }: {
    value: string;
    options: readonly { value: string; label: string }[];
    label: string;
    disabled?: boolean;
    onchange: (value: string) => void;
  } = $props();
</script>

<!-- The native element, so macOS draws its own pop-up menu. -->
<select aria-label={label} {value} {disabled} onchange={(event) => onchange(event.currentTarget.value)}>
  {#each options as option (option.value)}
    <option value={option.value}>{option.label}</option>
  {/each}
</select>

<style>
  select {
    font-family: inherit;
    font-size: 12px;
    color: var(--text-primary);
  }
</style>
