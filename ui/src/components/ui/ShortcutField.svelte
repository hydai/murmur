<script lang="ts">
  import { buildHotkey, formatShortcut, hasModifier, MISSING_MODIFIER_MESSAGE } from '../../lib/shortcut';

  let {
    value,
    disabled = false,
    onchange,
    onerror,
  }: {
    /** The stored form, such as `Cmd+Shift+K`. */
    value: string;
    disabled?: boolean;
    /** The shortcut that was typed, in its stored form. */
    onchange: (hotkey: string) => void;
    /** Why a key press was not taken as a shortcut. */
    onerror: (message: string) => void;
  } = $props();

  let recording = $state(false);
  let field: HTMLButtonElement | undefined;

  const keys = $derived(formatShortcut(value));

  function start() {
    recording = true;
    // Safari does not focus a button when it is clicked, and losing focus is
    // what ends the recording, so the field has to take focus itself.
    field?.focus();
  }

  /** Every attempt ends in a result: a shortcut, a reason it was refused, or a cancel. */
  function onkeydown(event: KeyboardEvent) {
    if (!recording) return;
    event.preventDefault();
    event.stopPropagation();

    const hotkey = buildHotkey(event);
    // A modifier on its own: the user is still reaching for the key.
    if (hotkey === null) return;

    recording = false;
    if (hasModifier(hotkey)) onchange(hotkey);
    else if (event.key !== 'Escape') onerror(MISSING_MODIFIER_MESSAGE);
  }
</script>

<svelte:window {onkeydown} />

<button
  bind:this={field}
  type="button"
  class="shortcut"
  class:recording
  {disabled}
  onclick={start}
  onblur={() => (recording = false)}
>
  <!-- Always in the page, so that assistive technology notices the text arriving. -->
  <span class="shortcut-status" aria-live="polite">{#if recording}Type shortcut…{/if}</span>
  {#if !recording}
    {#each keys as key, index (index)}
      <kbd>{key}</kbd>
    {/each}
  {/if}
</button>

<style>
  .shortcut {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    gap: 4px;
    min-width: 84px;
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--control-border);
    border-radius: 6px;
    background: var(--field-bg);
    color: var(--text-primary);
    font: inherit;
    font-size: 12px;
    transition: border-color 0.15s ease;
  }

  .shortcut.recording {
    border-color: var(--accent);
  }

  .shortcut:disabled {
    opacity: 0.45;
  }

  .shortcut-status {
    color: var(--text-secondary);
  }

  /*
   * Empty, the region is rendered but out of the flex flow. Rendered, so that
   * assistive technology has it before text arrives in it; out of the flow,
   * because an empty flex item would still add a gap beside the keys.
   */
  .shortcut-status:empty {
    position: absolute;
  }

  kbd {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 22px;
    height: 18px;
    padding: 0 5px;
    border-radius: 5px;
    background: var(--fill-selected);
    box-shadow: 0 1px 0 rgba(0, 0, 0, .2);
    font: inherit;
    font-size: 12px;
    font-weight: 500;
    line-height: 1;
  }

  @media (prefers-reduced-motion: reduce) {
    .shortcut {
      transition: none;
    }
  }
</style>
