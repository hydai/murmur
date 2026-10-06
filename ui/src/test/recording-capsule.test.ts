import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync } from 'svelte';
import RecordingCapsule from '../components/capsule/RecordingCapsule.svelte';
import { barHeights, smoothLevel, type CapsuleContext } from '../components/capsule/capsuleState';
import { render, settle, unmountAll } from './helpers';

// The capsule calls no commands, so the event bridge is the only thing to fake.
const mocks = vi.hoisted(() => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }));

/** The one question the capsule asks the system; any other query reads as "no preference". */
const REDUCED_MOTION = '(prefers-reduced-motion: reduce)';

/** What Rust sends the capsule window when a recording starts. */
const CONTEXT: CapsuleContext = { shortcut: 'Ctrl+`', output_mode: 'clipboard', save_history: true };

let listeners: Map<string, (event: { payload: unknown }) => void>;
/** One unlisten function for each subscription, so a test can tell which were let go. */
let unlisteners: ReturnType<typeof vi.fn>[];
let motion: { matches: boolean; addEventListener: ReturnType<typeof vi.fn>; removeEventListener: ReturnType<typeof vi.fn> };

/** jsdom has no matchMedia, so answer for the reduced-motion query. */
function systemMotion(reduced: boolean) {
  motion = { matches: reduced, addEventListener: vi.fn(), removeEventListener: vi.fn() };
  const noPreference = { matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() };
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    value: vi.fn((query: string) => (query === REDUCED_MOTION ? motion : noPreference)),
  });
}

beforeEach(() => {
  listeners = new Map();
  unlisteners = [];
  mocks.listen.mockReset().mockImplementation(async (name: string, callback: (event: { payload: unknown }) => void) => {
    listeners.set(name, callback);
    const unlisten = vi.fn(() => { listeners.delete(name); });
    unlisteners.push(unlisten);
    return unlisten;
  });
  systemMotion(false);
});

afterEach(async () => {
  await unmountAll();
  vi.useRealTimers();
});

function emit(name: string, payload: unknown) {
  const listener = listeners.get(name);
  expect(listener, `${name} listener`).toBeDefined();
  listener!({ payload });
  flushSync();
}

// One helper for each event the capsule hears, named after what happened.
const pipelineState = (state: string) => emit('pipeline-state', { state, timestamp_ms: 0 });
const microphone = (open: boolean) => emit('recording-state', { is_recording: open });
const commandDetected = (name: string | null) => emit('command-detected', { command_name: name, timestamp_ms: 0 });
const pipelineResult = () => emit('pipeline-result', { text: 'Hello', processing_time_ms: 5 });
const pipelineError = (message: string) => emit('pipeline-error', { message, recoverable: true });
const capsuleContext = (context: CapsuleContext = CONTEXT) => emit('capsule-context', context);
const audioLevel = (rms: number) => emit('audio-level', { rms, voice_active: true, timestamp_ms: 0 });

/** Move the clock on and let Svelte draw what the timers changed. */
function advance(ms: number) {
  vi.advanceTimersByTime(ms);
  flushSync();
}

/** Mount the capsule and let it subscribe. */
async function mountCapsule() {
  const { target } = render(RecordingCapsule, {});
  await settle();
  return target;
}

const text = (target: Element, selector: string) => target.querySelector(selector)?.textContent;
const fading = (target: Element) => target.querySelector('.capsule')!.classList.contains('fading');
const barHeightsShown = (target: Element) =>
  [...target.querySelectorAll<HTMLElement>('.bar')].map(bar => Number.parseFloat(bar.style.height));

/** The bars are the heights `barHeights` gives for this level. */
function expectBarsAt(target: Element, level: number) {
  const expected = barHeights(level);
  const shown = barHeightsShown(target);
  expect(shown).toHaveLength(expected.length);
  shown.forEach((height, i) => expect(height).toBeCloseTo(expected[i], 3));
}

describe('recording capsule', () => {
  it('renders nothing until a recording starts', async () => {
    const target = await mountCapsule();
    expect(target.querySelector('.capsule')).toBeNull();
    expect(target.textContent?.trim()).toBe('');

    // What reaches it before a recording does not show it either.
    capsuleContext();
    audioLevel(0.5);
    microphone(true);
    expect(target.querySelector('.capsule')).toBeNull();

    pipelineState('recording');
    expect(target.querySelector('.capsule')?.getAttribute('role')).toBe('status');
  });

  it('shows the timer, 15 bars and the stop hint while recording', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(0);
    const target = await mountCapsule();
    capsuleContext();
    pipelineState('recording');
    microphone(true);
    expect(text(target, '.timer')).toBe('00:00');
    expect(target.querySelectorAll('.bar')).toHaveLength(15);
    expect(text(target, '.hint')).toBe('⌃` to stop');
    expect(target.querySelector('.dot')).not.toBeNull();
    // The timer stands in for words while the microphone is open.
    expect(target.querySelector('.status')).toBeNull();
    // The fade-in is hung on this class.
    expect(target.querySelector('.capsule')!.classList.contains('recording')).toBe(true);

    advance(999);
    expect(text(target, '.timer')).toBe('00:00');
    advance(1);
    expect(text(target, '.timer')).toBe('00:01');
    advance(64000);
    expect(text(target, '.timer')).toBe('01:05');

    // A streaming provider reports Transcribing while the person is still talking. The microphone is open, so it
    // is still a recording, and the timer keeps the start it had.
    pipelineState('transcribing');
    advance(5000);
    expect(text(target, '.timer')).toBe('01:10');
    expect(text(target, '.hint')).toBe('⌃` to stop');
  });

  it('leaves the timer out when it joins a recording that is already under way', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(5000);
    const target = await mountCapsule();
    capsuleContext();
    // The window loaded after the recording began: the microphone is open, but nothing said when it started.
    microphone(true);
    pipelineState('transcribing');
    expect(target.querySelector('.dot')).not.toBeNull();
    expect(target.querySelectorAll('.bar')).toHaveLength(15);
    expect(text(target, '.hint')).toBe('⌃` to stop');
    expect(target.querySelector('.timer')).toBeNull();
    // With nothing to count from there is nothing to keep ticking.
    expect(vi.getTimerCount()).toBe(0);

    // The next recording knows when it started, and its timer comes in with it.
    pipelineState('recording');
    expect(text(target, '.timer')).toBe('00:00');
    expect(vi.getTimerCount()).toBeGreaterThan(0);
    advance(3000);
    expect(text(target, '.timer')).toBe('00:03');
  });

  it('keeps the timer on whole seconds when a tick comes late', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(0);
    const target = await mountCapsule();
    pipelineState('recording');

    // The machine was busy or asleep: the tick due at one second comes 400 ms late.
    vi.setSystemTime(400);
    advance(1000);
    expect(text(target, '.timer')).toBe('00:01');
    // The next one is not pushed back with it: it still comes at the second.
    advance(600);
    expect(text(target, '.timer')).toBe('00:02');
    advance(1000);
    expect(text(target, '.timer')).toBe('00:03');
  });

  it('moves the bars with the audio level and starts each recording quiet', async () => {
    const target = await mountCapsule();
    pipelineState('recording');
    expectBarsAt(target, 0);

    audioLevel(1);
    expectBarsAt(target, smoothLevel(0, 1));
    audioLevel(1);
    expectBarsAt(target, smoothLevel(smoothLevel(0, 1), 1));

    // However loud it gets, every bar stays inside the capsule and keeps a stub.
    for (let i = 0; i < 30; i++) audioLevel(1);
    expect(barHeightsShown(target).every(height => height >= 8 && height <= 100)).toBe(true);

    // The next recording does not start where this one left off.
    pipelineState('processing');
    pipelineResult();
    pipelineState('recording');
    expectBarsAt(target, 0);
  });

  it('swaps the bars for one level bar when motion is reduced', async () => {
    systemMotion(true);
    const target = await mountCapsule();
    pipelineState('recording');
    expect(window.matchMedia).toHaveBeenCalledWith(REDUCED_MOTION);
    expect(target.querySelectorAll('.bar')).toHaveLength(0);
    expect(target.querySelectorAll('.level')).toHaveLength(1);

    const level = target.querySelector<HTMLElement>('.level')!;
    expect(Number.parseFloat(level.style.width)).toBe(0);
    audioLevel(1);
    expect(Number.parseFloat(level.style.width)).toBeCloseTo(smoothLevel(0, 1) * 100);
  });

  it('follows the system motion setting while it is shown, and lets go of it when unmounted', async () => {
    const target = await mountCapsule();
    pipelineState('recording');
    expect(target.querySelectorAll('.bar')).toHaveLength(15);

    const changes = motion.addEventListener.mock.calls.filter(([type]) => type === 'change');
    expect(changes).toHaveLength(1);
    const onChange = changes[0][1] as (event: { matches: boolean }) => void;

    // The setting changes under the capsule: the query and the event it fires say so together.
    motion.matches = true;
    onChange({ matches: true });
    flushSync();
    expect(target.querySelectorAll('.bar')).toHaveLength(0);
    expect(target.querySelectorAll('.level')).toHaveLength(1);

    motion.matches = false;
    onChange({ matches: false });
    flushSync();
    expect(target.querySelectorAll('.bar')).toHaveLength(15);
    expect(target.querySelector('.level')).toBeNull();

    expect(motion.removeEventListener).not.toHaveBeenCalled();
    await unmountAll();
    expect(motion.removeEventListener).toHaveBeenCalledWith('change', onChange);
  });

  it('shows Transcribing with the cancel hint after the microphone closes', async () => {
    const target = await mountCapsule();
    capsuleContext();
    pipelineState('recording');
    microphone(true);
    microphone(false);
    expect(text(target, '.status')).toBe('Transcribing…');
    expect(text(target, '.hint')).toBe('⌃` to cancel');
    // Words and a spinner take the place of the timer and the bars.
    expect(target.querySelector('.spinner svg.lucide-loader-circle')).not.toBeNull();
    expect(target.querySelector('.timer')).toBeNull();
    expect(target.querySelector('.bar')).toBeNull();
    expect(target.querySelector('.dot')).toBeNull();
    expect(target.querySelector('.capsule')!.classList.contains('recording')).toBe(false);
  });

  it('names the voice command while it works, and forgets it for the next recording', async () => {
    const target = await mountCapsule();
    capsuleContext();
    pipelineState('recording');
    microphone(true);
    microphone(false);
    // Rust reports the command before it reports Processing.
    commandDetected('translate to Japanese');
    pipelineState('processing');
    expect(text(target, '.status')).toBe('Translating to Japanese…');
    expect(text(target, '.hint')).toBe('⌃` to cancel');
    expect(target.querySelector('.spinner svg.lucide-loader-circle')).not.toBeNull();

    pipelineResult();
    pipelineState('recording');
    microphone(true);
    microphone(false);
    pipelineState('processing');
    expect(text(target, '.status')).toBe('Polishing…');
  });

  it('shows Copied for clipboard output and fades before it is hidden', async () => {
    vi.useFakeTimers();
    const target = await mountCapsule();
    capsuleContext();
    pipelineState('processing');
    // The result alone ends the session, with no terminal pipeline-state after it.
    pipelineResult();
    expect(text(target, '.status')).toBe('Copied · ⌘V to paste');
    expect(target.querySelector('svg.lucide-check')).not.toBeNull();
    expect(target.querySelector('.spinner')).toBeNull();
    expect(target.querySelector('.hint')).toBeNull();

    // Rust hides the window 1.5 s after the result; the fade takes the last 200 ms of that.
    expect(fading(target)).toBe(false);
    advance(1299);
    expect(fading(target)).toBe(false);
    advance(1);
    expect(fading(target)).toBe(true);
  });

  it('shows the error from a failed start', async () => {
    const target = await mountCapsule();
    capsuleContext();
    pipelineError('No API key is set for OpenAI');
    expect(text(target, '.status')).toBe('No API key is set for OpenAI');
    expect(target.querySelector('svg.lucide-circle-alert')).not.toBeNull();
    expect(target.querySelector('.hint')).toBeNull();
  });

  it('restarts the error display for a second failed start', async () => {
    vi.useFakeTimers();
    const target = await mountCapsule();
    capsuleContext();
    pipelineError('No API key is set for OpenAI');
    advance(3800);
    expect(fading(target)).toBe(true);

    // Rust hid the window at 4 s. The person tries again and it fails the same way: the capsule is in its error
    // phase already, and still has to come back and stay up for the full time.
    advance(200);
    pipelineError('No API key is set for OpenAI');
    expect(fading(target)).toBe(false);
    advance(3799);
    expect(fading(target)).toBe(false);
    advance(1);
    expect(fading(target)).toBe(true);
  });

  it('fades the error from a failed start 3.8 s after it, with no session to wait for', async () => {
    vi.useFakeTimers();
    const target = await mountCapsule();
    capsuleContext();
    pipelineError('No API key is set for OpenAI');
    advance(3799);
    expect(fading(target)).toBe(false);
    advance(1);
    expect(fading(target)).toBe(true);
  });

  it('keeps an error up while the session it came in goes on', async () => {
    vi.useFakeTimers();
    const target = await mountCapsule();
    capsuleContext();
    pipelineState('recording');
    microphone(true);
    microphone(false);
    pipelineState('transcribing');
    // The provider fails partway through with text already in hand, so the pipeline goes on to process it. Rust
    // keeps the window up until the session's last state, so the capsule shows the error in the meantime.
    pipelineError('HTTP STT error: 429 Too Many Requests');
    commandDetected(null);
    pipelineState('processing');
    expect(text(target, '.status')).toBe('HTTP STT error: 429 Too Many Requests');

    // The language model takes seconds, and the error does not fade while it does.
    advance(3800);
    expect(fading(target)).toBe(false);
    advance(4200);
    expect(fading(target)).toBe(false);

    // The result ends the session, and the Error state after it changes nothing: the fade counts from here.
    pipelineResult();
    pipelineState('error');
    expect(text(target, '.status')).toBe('HTTP STT error: 429 Too Many Requests');
    advance(3799);
    expect(fading(target)).toBe(false);
    advance(1);
    expect(fading(target)).toBe(true);
  });

  it('keeps waiting for the session to end when a second error comes before it', async () => {
    vi.useFakeTimers();
    const target = await mountCapsule();
    capsuleContext();
    pipelineState('recording');
    microphone(true);
    microphone(false);
    pipelineError('HTTP STT error: 429 Too Many Requests');
    commandDetected(null);
    pipelineState('processing');
    advance(5000);

    // The language model fails as well. Its error replaces the first on screen, and the session goes on to deliver
    // the raw text, so there is still nothing to fade for.
    pipelineError('LLM processing failed: timed out. Using raw transcription.');
    expect(text(target, '.status')).toBe('LLM processing failed: timed out. Using raw transcription.');
    advance(3800);
    expect(fading(target)).toBe(false);
    advance(10000);
    expect(fading(target)).toBe(false);

    pipelineResult();
    pipelineState('error');
    advance(3799);
    expect(fading(target)).toBe(false);
    advance(1);
    expect(fading(target)).toBe(true);
  });

  it('counts the fade from the end of the session, not from the reports that follow it', async () => {
    vi.useFakeTimers();
    const target = await mountCapsule();
    capsuleContext();
    pipelineState('processing');
    pipelineResult();
    // The session settles with a few more reports. They replace the capsule's state without changing what it shows,
    // so the countdown that the result began goes on.
    advance(1000);
    microphone(false);
    pipelineState('done');
    advance(299);
    expect(fading(target)).toBe(false);
    advance(1);
    expect(fading(target)).toBe(true);
  });

  // Each of these ends a session in a different way; Rust hides the window after 4, 4 and 1 s.
  it.each([
    {
      ending: 'a failed output',
      status: "Couldn't copy the text · saved in History",
      icon: 'lucide-circle-alert',
      fadesAt: 3800,
      reach() {
        pipelineState('processing');
        pipelineError('Output failed: no access');
        pipelineResult();
      },
    },
    {
      ending: 'an error',
      status: 'Microphone unavailable',
      icon: 'lucide-circle-alert',
      fadesAt: 3800,
      reach() {
        pipelineState('recording');
        pipelineError('Microphone unavailable');
        pipelineState('error');
      },
    },
    {
      ending: 'a cancelled recording',
      status: 'Cancelled',
      icon: null,
      fadesAt: 800,
      reach() {
        pipelineState('processing');
        pipelineState('idle');
      },
    },
  ])('shows $ending, and fades it $fadesAt ms in', async ({ status, icon, fadesAt, reach }) => {
    vi.useFakeTimers();
    const target = await mountCapsule();
    capsuleContext();
    reach();
    expect(text(target, '.status')).toBe(status);
    if (icon) expect(target.querySelector(`svg.${icon}`)).not.toBeNull();
    else expect(target.querySelector('svg')).toBeNull();
    expect(target.querySelector('.hint')).toBeNull();

    advance(fadesAt - 1);
    expect(fading(target)).toBe(false);
    advance(1);
    expect(fading(target)).toBe(true);
  });

  it('replaces a fading result when the next recording starts, and clears what it left', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(0);
    const target = await mountCapsule();
    capsuleContext();
    commandDetected('shorten');
    pipelineState('processing');
    pipelineError('Output failed: no access');
    pipelineResult();
    expect(text(target, '.status')).toBe("Couldn't copy the text · saved in History");
    advance(3800);
    expect(fading(target)).toBe(true);

    // Started while the last one fades out: the fade stops, and nothing of the last one is left.
    advance(100);
    pipelineState('recording');
    microphone(true);
    expect(fading(target)).toBe(false);
    expect(target.querySelector('.status')).toBeNull();
    expect(text(target, '.timer')).toBe('00:00');
    expect(text(target, '.hint')).toBe('⌃` to stop');

    // Started before the last one began to fade: the fade it had scheduled must not take this one.
    microphone(false);
    pipelineState('processing');
    expect(text(target, '.status')).toBe('Polishing…');
    pipelineResult();
    advance(1000);
    pipelineState('recording');
    microphone(true);
    advance(5000);
    expect(fading(target)).toBe(false);
    expect(text(target, '.timer')).toBe('00:05');
  });

  it('listens only to events the backend emits', async () => {
    await mountCapsule();
    const subscribed = mocks.listen.mock.calls.map(([name]) => name as string).sort();
    expect(subscribed).toEqual([
      'audio-level', 'capsule-context', 'command-detected', 'pipeline-error',
      'pipeline-result', 'pipeline-state', 'recording-state',
    ]);
    for (const name of [
      'audio-error', 'transcription-partial', 'transcription-committed',
      'processing-status', 'transcription-error', 'transcription-processed',
    ]) {
      expect(subscribed, `${name} is not an event the capsule needs`).not.toContain(name);
    }
  });

  it('stops listening when unmounted', async () => {
    await mountCapsule();
    expect(unlisteners).toHaveLength(7);
    for (const unlisten of unlisteners) expect(unlisten).not.toHaveBeenCalled();
    await unmountAll();
    for (const unlisten of unlisteners) expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('keeps no timer running while nothing needs one', async () => {
    vi.useFakeTimers();
    const target = await mountCapsule();
    expect(vi.getTimerCount()).toBe(0);

    // The recording's clock needs one, and nothing else does.
    pipelineState('recording');
    microphone(true);
    expect(vi.getTimerCount()).toBeGreaterThan(0);
    microphone(false);
    expect(target.querySelector('.timer')).toBeNull();
    expect(vi.getTimerCount()).toBe(0);

    pipelineState('recording');
    microphone(true);
    expect(vi.getTimerCount()).toBeGreaterThan(0);
    await unmountAll();
    expect(vi.getTimerCount()).toBe(0);
  });
});
