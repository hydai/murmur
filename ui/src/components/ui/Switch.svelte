<script lang="ts">
  let {
    checked,
    label,
    disabled = false,
    onchange,
  }: {
    checked: boolean;
    label: string;
    disabled?: boolean;
    onchange: (checked: boolean) => void;
  } = $props();
</script>

<button
  type="button"
  role="switch"
  class="switch"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  onclick={() => onchange(!checked)}
>
  <span class="knob"></span>
</button>

<style>
  .switch {
    position: relative;
    flex: none;
    width: 32px;
    height: 19px;
    padding: 0;
    border: 0;
    border-radius: 9.5px;
    background: var(--switch-off);
    transition: background-color 0.15s ease;
  }

  .switch[aria-checked='true'] {
    background: var(--accent-fill);
  }

  .switch:disabled {
    opacity: 0.45;
  }

  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 15px;
    height: 15px;
    border-radius: 50%;
    background: #FFFFFF;
    transition: transform 0.15s ease;
  }

  .switch[aria-checked='true'] .knob {
    transform: translateX(13px);
  }

  @media (prefers-reduced-motion: reduce) {
    .switch,
    .knob {
      transition: none;
    }
  }
</style>
