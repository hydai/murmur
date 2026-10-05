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
    /**
     * Called with the option the user picked. When the parent only learns later
     * whether it can accept the pick, such as after a backend call, it returns
     * a promise, and the element is lined up with `value` once that settles.
     * A rejection is not caught here.
     */
    onchange: (value: string) => void | Promise<unknown>;
  } = $props();

  /**
   * `value` is the parent's word: it moves only when the parent accepts a pick.
   * Until then the native element shows the pick, so once the parent has
   * answered, line the element up with `value` again. An accepted pick is
   * already there; a refused one goes back to what it was.
   */
  async function change(event: Event & { currentTarget: HTMLSelectElement }) {
    // Gone by the time the await returns, so take it now.
    const select = event.currentTarget;
    try {
      await onchange(select.value);
    } finally {
      select.value = value;
    }
  }
</script>

<!-- The native element, so macOS draws its own pop-up menu. -->
<select aria-label={label} {value} {disabled} onchange={change}>
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
