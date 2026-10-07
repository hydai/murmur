<script lang="ts">
  import { onMount } from 'svelte';
  import { Check, CircleAlert, LoaderCircle } from 'lucide-svelte';
  import type { AppEventName, AppEvents } from '../../lib/events';
  import { useLifecycle } from '../../lib/lifecycle';
  import {
    HIDE_AFTER_MS,
    INITIAL_CAPSULE,
    barHeights,
    formatElapsed,
    hintText,
    reduce,
    smoothLevel,
    statusText,
    type CapsuleEvent,
    type CapsulePhase,
  } from './capsuleState';

  // The capsule only shows what the pipeline reports, and it calls no commands: Rust shows and hides its window,
  // and this draws what is in it.

  /** The fade-out takes this long, and ends as Rust hides the window. */
  const FADE_MS = 200;

  /** How long each ending stays up. A phase that is not listed stays until something else happens. */
  const HIDE_AFTER: Partial<Record<CapsulePhase, number>> = HIDE_AFTER_MS;

  const lifecycle = useLifecycle();

  // Only ever replaced, never edited: INITIAL_CAPSULE is shared with every new recording, and an event that
  // changes nothing gives the same object back.
  let capsule = $state.raw(INITIAL_CAPSULE);
  // The microphone level, smoothed over the samples. The capsule never sees what was said.
  let level = $state(0);
  // The clock behind the timer, brought up to date on each second of a recording.
  let now = $state(Date.now());
  let fading = $state(false);
  let reducedMotion = $state(false);
  // How many errors have come. The phase stays 'error' when one follows another, so this is what tells the fade
  // below that the display has started over.
  let errors = $state(0);

  // Derived, so that what follows the phase runs when the phase or the end of the session changes and not on every
  // event.
  const phase = $derived(capsule.phase);
  const ended = $derived(capsule.ended);
  const startedAt = $derived(capsule.startedAt);
  const status = $derived(statusText(capsule));
  const hint = $derived(hintText(capsule));
  const bars = $derived(barHeights(level));

  function apply(event: CapsuleEvent) {
    capsule = reduce(capsule, event);
  }

  /** One failed subscription is reported and leaves the others in place. */
  function listen<K extends AppEventName>(name: K, handler: (payload: AppEvents[K]) => void) {
    void lifecycle.listen(name, ({ payload }) => handler(payload))
      .catch((error) => console.warn(`Failed to listen for ${name}:`, error));
  }

  onMount(() => {
    const motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)');
    reducedMotion = motionQuery.matches;
    const onMotionChange = (event: MediaQueryListEvent) => {
      reducedMotion = event.matches;
    };
    motionQuery.addEventListener('change', onMotionChange);
    lifecycle.onCleanup(() => motionQuery.removeEventListener('change', onMotionChange));

    listen('pipeline-state', ({ state }) => {
      // Each recording starts quiet, not at the level the last one ended on.
      if (state === 'recording') level = 0;
      apply({ type: 'state', state, at: Date.now() });
    });
    listen('recording-state', ({ is_recording }) => apply({ type: 'capture', capturing: is_recording }));
    listen('command-detected', ({ command_name }) => apply({ type: 'command', name: command_name }));
    listen('pipeline-result', () => apply({ type: 'result' }));
    listen('pipeline-error', ({ message }) => {
      apply({ type: 'error', message });
      // Counts each error that leaves the capsule showing one. `Output failed` only waits for the result, so it
      // does not, unless an error from the same recording is up already.
      if (capsule.phase === 'error') errors += 1;
    });
    listen('capsule-context', (context) => apply({ type: 'context', context }));
    listen('nothing-heard', () => apply({ type: 'nothing-heard' }));
    listen('audio-level', ({ rms }) => {
      level = smoothLevel(level, rms);
    });
  });

  // The timer moves on each whole second of the recording, so it never skips a number or shows one twice. It has
  // nothing to count from when the window loaded in the middle of a recording, so it waits for the next one.
  $effect(() => {
    if (phase !== 'recording' || startedAt === null) return;
    const began = startedAt;
    const tick = () => {
      // Read the clock once, and not through `now`, which would make this effect depend on its own ticks.
      const current = Date.now();
      now = current;
      lifecycle.timeout(tick, 1000 - ((current - began) % 1000), 'tick');
    };
    tick();
    return () => lifecycle.cancelTimeout('tick');
  });

  // An ending fades out just before Rust hides the window. Anything else, a new recording above all, stops the
  // fade and brings the capsule back at once. So does another error, such as a second failed start: it comes while
  // the phase is still 'error', and Rust shows the window again for as long as the first.
  //
  // The countdown starts when the session has ended, not when the error appears. An error in the middle of a session
  // is on screen at once, but the pipeline goes on past it with the text it has, which takes seconds, and Rust keeps
  // the window up until the session's last state. Fading earlier would leave the window up and the capsule gone.
  $effect(() => {
    // Read only so that another error runs this again.
    void errors;
    fading = false;
    const hideAfter = ended ? HIDE_AFTER[phase] : undefined;
    if (hideAfter === undefined) {
      lifecycle.cancelTimeout('fade');
    } else {
      lifecycle.timeout(() => {
        fading = true;
      }, hideAfter - FADE_MS, 'fade');
    }
  });
</script>

{#if phase !== 'hidden'}
  <div class="capsule" class:recording={phase === 'recording'} class:fading role="status">
    {#if phase === 'recording'}
      <span class="dot" aria-hidden="true"></span>
      {#if startedAt !== null}
        <!-- A timer is not announced as it ticks, which a status region would do every second. -->
        <span class="timer" role="timer">{formatElapsed(now - startedAt)}</span>
      {/if}
      {#if reducedMotion}
        <span class="track" aria-hidden="true">
          <span class="level" style:width="{level * 100}%"></span>
        </span>
      {:else}
        <span class="bars" aria-hidden="true">
          <!-- Bars are positional, so the index is the key. -->
          {#each bars as height, i (i)}
            <span class="bar" style:height="{height}%"></span>
          {/each}
        </span>
      {/if}
    {:else if phase === 'transcribing' || phase === 'processing'}
      <span class="spinner"><LoaderCircle size={16} aria-hidden="true" /></span>
      <span class="status">{status}</span>
    {:else if phase === 'done'}
      <span class="icon done"><Check size={16} aria-hidden="true" /></span>
      <span class="status">{status}</span>
    {:else if phase === 'output-failed' || phase === 'error'}
      <span class="icon alert"><CircleAlert size={16} aria-hidden="true" /></span>
      <span class="status">{status}</span>
    {:else}
      <span class="status">{status}</span>
    {/if}
    {#if hint}
      <span class="hint">{hint}</span>
    {/if}
  </div>
{/if}

<style>
  /* Dark whatever the system appearance is, so that it reads on any content behind it. */
  .capsule {
    --capsule-text: #FFFFFF;
    --capsule-secondary: rgba(255, 255, 255, 0.55);
    --capsule-red: #FF453A;
    --capsule-green: #32D74B;
    --capsule-orange: #FF9F0A;

    display: flex;
    align-items: center;
    gap: 8px;
    max-width: 400px;
    height: 40px;
    padding: 0 16px;
    border-radius: 20px;
    background: rgba(22, 22, 24, 0.92);
    box-shadow:
      0 2px 10px rgba(0, 0, 0, 0.35),
      0 0 0 1px rgba(255, 255, 255, 0.08);
    color: var(--capsule-text);
    font-size: 13px;
    font-weight: 500;
    white-space: nowrap;
  }

  /* The capsule comes in as a recording starts, including over an ending that is still fading out. */
  .recording {
    animation: capsule-in 150ms ease-out;
  }

  @keyframes capsule-in {
    from {
      opacity: 0;
    }

    to {
      opacity: 1;
    }
  }

  /* The transition is declared here, so that taking .fading off again brings the capsule back at once. */
  .fading {
    opacity: 0;
    transition: opacity 200ms ease;
  }

  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--capsule-red);
  }

  .timer {
    font-variant-numeric: tabular-nums;
  }

  /* The bars grow from the middle of a fixed-height strip, so a loud signal cannot overflow the capsule. */
  .bars {
    display: flex;
    flex: none;
    align-items: center;
    gap: 2px;
    height: 16px;
  }

  .bar {
    width: 2px;
    border-radius: 1px;
    background: var(--capsule-text);
    transition: height 100ms ease-out;
  }

  /* As wide as the bars, so the capsule does not change size when the setting does. */
  .track {
    flex: none;
    width: 58px;
    height: 4px;
    overflow: hidden;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.2);
  }

  .level {
    display: block;
    height: 100%;
    background: var(--capsule-text);
  }

  .spinner {
    display: inline-flex;
    flex: none;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .icon {
    display: inline-flex;
    flex: none;
  }

  .done {
    color: var(--capsule-green);
  }

  .alert {
    color: var(--capsule-orange);
  }

  /* A long message is cut short, and the shortcut beside it is not. */
  .status {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .hint {
    flex-shrink: 0;
    color: var(--capsule-secondary);
  }

  @media (prefers-reduced-motion: reduce) {
    .recording {
      animation: none;
    }

    .fading {
      transition: none;
    }

    .spinner {
      animation: none;
    }
  }
</style>
