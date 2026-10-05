<script lang="ts">
  import type { Component, ComponentType, Snippet, SvelteComponent } from 'svelte';
  import { Check } from 'lucide-svelte';
  import type { IconProps } from 'lucide-svelte';

  /**
   * lucide-svelte icons are class components, which the `Component` type
   * alone rejects, so accept both kinds.
   */
  type IconComponent = Component<IconProps> | ComponentType<SvelteComponent<IconProps>>;

  let {
    label,
    detail,
    icon,
    current = false,
    disabled = false,
    onclick,
    accessory,
    trailing,
    children,
  }: {
    label: string;
    detail?: string;
    icon?: IconComponent;
    /** The option in use; it gets a check mark and is announced as such. */
    current?: boolean;
    disabled?: boolean;
    /** Makes the main area a button. */
    onclick?: () => void;
    /**
     * Content at the right end of the main area, inside the button when there is
     * an `onclick`, so a click on it is a click on the row: a status, a chevron
     * that says the row leads to a page. It is read as part of the row's name.
     * Nothing that can be pressed belongs here, and a button cannot hold a
     * button, so only phrasing content; controls go in `trailing`.
     */
    accessory?: Snippet;
    /** Actions beside the main area. They are siblings of it, never inside the button. */
    trailing?: Snippet;
    /** Extra content under the main area, such as a progress bar. */
    children?: Snippet;
  } = $props();

  const Icon = $derived(icon);

  // Call without the click event, so a handler with optional parameters never receives it.
  function activate() {
    onclick?.();
  }
</script>

{#snippet content()}
  {#if Icon}
    <span class="row-icon"><Icon size={15} strokeWidth={1.75} aria-hidden="true" /></span>
  {/if}
  <span class="row-text">
    <span class="row-label">{label}</span>
    {#if detail}<span class="row-detail" title={detail}>{detail}</span>{/if}
  </span>
  <!-- Before the check, which stays at the far end of every row that has one. -->
  {#if accessory}<span class="row-accessory">{@render accessory()}</span>{/if}
  {#if current}
    <span class="row-check"><Check size={17} aria-hidden="true" /></span>
    <span class="sr-only">In use</span>
  {/if}
{/snippet}

<div class="row" class:has-icon={!!Icon}>
  {#if onclick}
    <button
      type="button"
      class="row-main"
      class:has-detail={!!detail}
      class:disabled
      aria-current={current ? 'true' : undefined}
      {disabled}
      onclick={activate}
    >
      {@render content()}
    </button>
  {:else}
    <div
      class="row-main"
      class:has-detail={!!detail}
      class:disabled
      aria-current={current ? 'true' : undefined}
    >
      {@render content()}
    </div>
  {/if}
  {#if trailing}
    <div class="row-trailing">{@render trailing()}</div>
  {/if}
  {#if children}
    <div class="row-extra">{@render children()}</div>
  {/if}
</div>

<style>
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
  }

  .row-main {
    position: relative;
    display: flex;
    flex: 1 1 0;
    align-items: center;
    gap: 10px;
    min-width: 0;
    min-height: 40px;
    padding: 6px 12px;
    border: 0;
    background: transparent;
    color: var(--text-primary);
    font: inherit;
    text-align: left;
    transition: background-color 0.15s ease;
  }

  .row-main.has-detail {
    min-height: 44px;
  }

  button.row-main:hover:not(:disabled) {
    background: var(--fill-selected);
  }

  /* An outset ring would be clipped by the group's rounded edge. */
  .row-main:focus-visible {
    outline-offset: -2px;
  }

  .row-main.disabled {
    opacity: 0.45;
  }

  .row-icon {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border-radius: 7px;
    background: var(--fill-selected);
    color: var(--text-secondary);
  }

  .row-text {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    min-width: 0;
    line-height: 1.3;
  }

  .row-label {
    font-size: 13px;
    overflow-wrap: anywhere;
  }

  .row-detail {
    min-width: 0;
    overflow: hidden;
    font-size: 11.5px;
    color: var(--text-secondary);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-accessory {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 8px;
  }

  .row-check {
    display: inline-flex;
    flex: none;
    color: var(--accent);
  }

  .row-trailing {
    display: flex;
    flex: none;
    align-items: center;
    gap: 8px;
    padding-right: 12px;
  }

  .row-extra {
    flex: 0 0 100%;
    padding: 0 12px 10px;
  }

  /* Line the extra content up with the text, past the icon tile and its gap. */
  .row.has-icon .row-extra {
    padding-left: 48px;
  }

  @media (prefers-reduced-motion: reduce) {
    .row-main {
      transition: none;
    }
  }
</style>
