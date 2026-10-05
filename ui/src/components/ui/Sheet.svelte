<script lang="ts">
  import type { Snippet } from 'svelte';
  import { trapFocus } from '../../lib/focus';

  let {
    title,
    onclose,
    onsubmit,
    error,
    children,
    actions,
    leading,
  }: {
    title: string;
    /** Escape asks for this; the sheet's own Cancel button should call it too. */
    onclose: () => void;
    /** Enter in a field and the submit button both land here. */
    onsubmit?: () => void;
    /** A rejected entry, shown under the fields so it is read next to what caused it. */
    error?: string;
    /** The fields: each a label followed by its control. */
    children: Snippet;
    /** The buttons, with the default one `type="submit"` so Enter reaches it. */
    actions: Snippet;
    /** Left of the actions, for a button that is not one of the choices, such as Delete. */
    leading?: Snippet;
  } = $props();

  const titleId = $props.id();

  function onkeydown(event: KeyboardEvent) {
    if (event.key !== 'Escape') return;
    // An input method cancels its candidate list with Escape. That press is the
    // input method's own, and closing the sheet too would throw away what was typed.
    // keyCode 229 marks a key the input method handled; WebKit can send it after
    // the composition has already ended, with isComposing false.
    if (event.isComposing || event.keyCode === 229) return;
    // Escape belongs to the sheet; nothing behind it should react to it as well.
    event.stopPropagation();
    onclose();
  }

  function submit(event: SubmitEvent) {
    // Without this the webview would navigate to the form's action.
    event.preventDefault();
    onsubmit?.();
  }

  // Pressing a part of the page that cannot take focus moves focus to the body,
  // and from there Escape, Enter and the Tab trap no longer reach the sheet.
  // Only a press on the backdrop itself is cancelled; the sheet's own fields
  // keep their default behaviour.
  function keepFocus(event: MouseEvent) {
    if (event.target === event.currentTarget) event.preventDefault();
  }
</script>

<!-- Clicking the backdrop does nothing on purpose: closing there would throw away what was typed. -->
<div class="sheet-backdrop" role="presentation" onmousedown={keepFocus}>
  <div
    class="sheet"
    role="dialog"
    aria-modal="true"
    aria-labelledby={titleId}
    tabindex="-1"
    use:trapFocus
    {onkeydown}
  >
    <form onsubmit={submit}>
      <h2 id={titleId}>{title}</h2>
      <div class="sheet-fields">{@render children()}</div>
      {#if error}<p class="sheet-error" role="alert">{error}</p>{/if}
      <div class="sheet-footer">
        {#if leading}<div class="sheet-leading">{@render leading()}</div>{/if}
        <div class="sheet-actions">{@render actions()}</div>
      </div>
    </form>
  </div>
</div>

<style>
  /* Fixed rather than absolute: the sheet may be opened from inside a scrolling pane, and it dims the whole window. */
  .sheet-backdrop {
    position: fixed;
    inset: 0;
    z-index: 20;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding: 64px 22px 22px;
    background: var(--sheet-backdrop);
    animation: appear 0.15s ease;
  }

  .sheet {
    width: 360px;
    max-width: 100%;
    /* Scroll inside the sheet when the window is too short for it. */
    max-height: 100%;
    overflow-y: auto;
    padding: 18px;
    border-radius: 12px;
    background: var(--group-bg);
    box-shadow: 0 0 0 1px var(--separator), 0 12px 32px rgba(0, 0, 0, .28);
    color: var(--text-primary);
    font-size: 13px;
    line-height: 1.35;
  }

  h2 {
    margin: 0 0 12px;
    font-size: 13px;
    font-weight: 600;
    line-height: 1.3;
  }

  .sheet-fields {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  /*
   * The fields come from the caller's snippet, which this component's scoped
   * styles cannot name. A label sits 5px above its control, and every label
   * after the first starts a new field, 12px below the previous one.
   */
  .sheet-fields :global(label) {
    font-size: 12px;
    line-height: 1.2;
  }

  .sheet-fields :global(* + label) {
    margin-top: 7px;
  }

  .sheet-fields :global(input),
  .sheet-fields :global(textarea) {
    width: 100%;
    border: 1px solid var(--control-border);
    border-radius: 6px;
    background: var(--field-bg);
    color: var(--text-primary);
    font: inherit;
    font-size: 13px;
    transition: border-color 0.15s ease;
  }

  .sheet-fields :global(input) {
    height: 26px;
    padding: 0 8px;
  }

  .sheet-fields :global(textarea) {
    min-height: 48px;
    padding: 4px 8px;
    line-height: 1.35;
    resize: none;
  }

  .sheet-fields :global(input::placeholder),
  .sheet-fields :global(textarea::placeholder) {
    color: var(--text-secondary);
  }

  /* A soft ring in place of the page-wide outline, like the native text field. */
  .sheet-fields :global(input:focus-visible),
  .sheet-fields :global(textarea:focus-visible) {
    border-color: var(--accent);
    outline: 3px solid color-mix(in srgb, var(--accent) 30%, transparent);
    outline-offset: 0;
  }

  .sheet-error {
    margin-top: 10px;
    font-size: 11.5px;
    color: var(--danger);
  }

  .sheet-footer {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 16px;
  }

  .sheet-leading,
  .sheet-actions {
    display: flex;
    gap: 8px;
  }

  .sheet-actions {
    margin-left: auto;
  }

  @keyframes appear {
    from {
      opacity: 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .sheet-backdrop {
      animation: none;
    }

    .sheet-fields :global(input),
    .sheet-fields :global(textarea) {
      transition: none;
    }
  }
</style>
