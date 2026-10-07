/**
 * What the recording capsule shows, as a pure function of the pipeline events
 * it hears. The component feeds each event to `reduce` and draws the result;
 * nothing here touches the DOM, a timer or IPC, so every transition is a unit
 * test.
 *
 * Recording and Transcribing cannot be told apart by `pipeline-state` alone. A
 * streaming provider reports `transcribing` while the microphone is still
 * open, and a batch provider reports `recording` until it has the audio, even
 * though Stop has closed the microphone. The capsule therefore also follows
 * `recording-state` (`capture` below) and says Recording for exactly as long
 * as the microphone is open.
 */

import { formatShortcut } from '../../lib/shortcut';

export type CapsulePhase =
  | 'hidden' | 'recording' | 'transcribing' | 'processing'
  | 'done' | 'output-failed' | 'error' | 'cancelled';

/** Mirrors `CapsuleContext` in crates/lt-tauri/src/capsule.rs. */
export interface CapsuleContext {
  shortcut: string;
  output_mode: 'clipboard' | 'keyboard' | 'both';
  save_history: boolean;
}

export interface CapsuleState {
  phase: CapsulePhase;
  /** Whether the microphone is open (`recording-state`), which the pipeline state alone cannot say. */
  capturing: boolean;
  /** When the current recording began, for its timer; `null` until one has. */
  startedAt: number | null;
  /** The voice command `command-detected` named for this recording; `null` for none. */
  command: string | null;
  /** An `Output failed` error arrived for this recording, so its result is not a success. */
  outputFailed: boolean;
  /** Which destinations that error named as failed; neither when it named none. */
  failedOutputs: FailedOutputs;
  /** The recording ended with nothing transcribed (`nothing-heard`), so the Idle that follows is not a cancel. */
  nothingHeard: boolean;
  /**
   * The session is over, so what the capsule shows is its ending, and it is on its way out. An error does not end a
   * session that is under way: the pipeline goes on past it with the text it already has, and delivers that seconds
   * later. The error shows at once all the same, and it is the result, or the pipeline's last state, that ends it.
   */
  ended: boolean;
  /** The error to show, as one line. */
  message: string;
  /** What the hint and the Done wording depend on; `null` until Rust has sent it. */
  context: CapsuleContext | null;
}

export interface FailedOutputs {
  clipboard: boolean;
  keyboard: boolean;
}

export type CapsuleEvent =
  | { type: 'state'; state: string; at: number }
  | { type: 'capture'; capturing: boolean }
  | { type: 'command'; name: string | null }
  | { type: 'result' }
  | { type: 'error'; message: string }
  | { type: 'context'; context: CapsuleContext }
  | { type: 'nothing-heard' };

/** Nothing has happened yet, so nothing shows. A new recording starts from this too. */
export const INITIAL_CAPSULE: CapsuleState = {
  phase: 'hidden',
  capturing: false,
  startedAt: null,
  command: null,
  outputFailed: false,
  failedOutputs: { clipboard: false, keyboard: false },
  nothingHeard: false,
  ended: false,
  message: '',
  context: null,
};

/**
 * How the error starts that the pipeline sends when it cannot deliver the text
 * (`format!("Output failed: {error}")` in lt-pipeline's orchestrator). It comes
 * before the result, so it is remembered rather than shown: the result is what
 * ends the session, as `output-failed` instead of `done`.
 */
const OUTPUT_FAILED = 'Output failed';

/**
 * The destinations named in that error. lt-output's `CombinedOutput` tries each
 * one on its own and names every one that failed (`clipboard: …`, `keyboard: …`,
 * joined by `; `), so in Both mode one can fail while the other delivers.
 */
const failedOutputsIn = (message: string): FailedOutputs => ({
  clipboard: message.includes('clipboard: '),
  keyboard: message.includes('keyboard: '),
});

/** A recording is under way: the capsule is up for it, and it can still be cancelled. */
const isInSession = (phase: CapsulePhase) =>
  phase === 'recording' || phase === 'transcribing' || phase === 'processing';

/**
 * A session is still going: a recording is under way, or an error came in the middle of one that has not ended
 * since. The pipeline goes on past an error, so a capsule that shows one with `ended` still false has not seen the
 * last of its session.
 */
const isLive = (state: CapsuleState) => isInSession(state.phase) || (state.phase === 'error' && !state.ended);

/** `state` with its session over. A session that was over already comes back as the same object. */
const endSession = (state: CapsuleState): CapsuleState => (state.ended ? state : { ...state, ended: true });

/** Where a session ends once its text has been handed over, or has failed to be. */
const endingPhase = (state: CapsuleState): CapsulePhase => (state.outputFailed ? 'output-failed' : 'done');

function reducePipelineState(state: CapsuleState, name: string, at: number): CapsuleState {
  const { phase } = state;
  switch (name) {
    case 'recording':
      // A new recording replaces whatever is on screen, a result that is still fading included.
      return { ...INITIAL_CAPSULE, phase: 'recording', capturing: true, startedAt: at, context: state.context };
    case 'transcribing':
      // A streaming provider reports this while the microphone is still open, which is still Recording.
      if (phase === 'hidden' || phase === 'recording' || phase === 'transcribing') {
        return { ...state, phase: state.capturing ? 'recording' : 'transcribing' };
      }
      return state;
    case 'processing':
      // An error stays on screen, as it does for the result: only a new recording replaces it.
      return phase === 'error' ? state : { ...state, phase: 'processing' };
    case 'done':
      // The result has usually finished the session before this arrives.
      return endSession(phase === 'hidden' || isInSession(phase) ? { ...state, phase: endingPhase(state) } : state);
    case 'error':
      return endSession({ ...state, phase: 'error' });
    case 'idle':
      // Back to idle without a result: the session was abandoned.
      return endSession(isInSession(phase) ? { ...state, phase: 'cancelled' } : state);
    default:
      return state;
  }
}

/** The next state for an event. The input is never changed. */
export function reduce(state: CapsuleState, event: CapsuleEvent): CapsuleState {
  switch (event.type) {
    case 'state':
      return reducePipelineState(state, event.state, event.at);
    case 'capture':
      if (event.capturing) return { ...state, capturing: true };
      // Stop closed the microphone, so a recording still waiting on the pipeline is Transcribing now.
      return { ...state, capturing: false, phase: state.phase === 'recording' ? 'transcribing' : state.phase };
    case 'command':
      return { ...state, command: event.name };
    case 'result':
      // An error stays on screen, but the result is still the end of the session it came in.
      return endSession(state.phase === 'error' ? state : { ...state, phase: endingPhase(state) });
    case 'error':
      if (event.message.startsWith(OUTPUT_FAILED)) {
        return { ...state, outputFailed: true, failedOutputs: failedOutputsIn(event.message) };
      }
      // With no session under way the error is all there is to it, so it ends the session. With one under way, it
      // goes on: the pipeline processes the text it has, and the result or the Error state is what ends it.
      return {
        ...state,
        phase: 'error',
        message: event.message.trim().replace(/\s+/g, ' '),
        ended: !isLive(state),
      };
    case 'context':
      return { ...state, context: event.context };
    case 'nothing-heard':
      return { ...state, nothingHeard: true };
  }
}

/** The `translate to <Language>` voice command, as lt-pipeline's `detect_command` names it. */
const TRANSLATE_PREFIX = 'translate to ';

function processingText(command: string | null): string {
  switch (command) {
    case null: return 'Polishing…';
    case 'shorten': return 'Shortening…';
    case 'formalize': return 'Making it formal…';
    case 'casualize': return 'Making it casual…';
    case 'reply': return 'Writing a reply…';
  }
  return command.startsWith(TRANSLATE_PREFIX)
    ? `Translating to ${command.slice(TRANSLATE_PREFIX.length)}…`
    : 'Processing…';
}

function doneText(context: CapsuleContext | null): string {
  switch (context?.output_mode) {
    case 'clipboard': return 'Copied · ⌘V to paste';
    case 'keyboard': return 'Typed';
    case 'both': return 'Typed and copied';
    default: return 'Done';
  }
}

/** What failed, from the destinations the error named, or else from how the text was meant to go. */
function failedOutputText(context: CapsuleContext | null, failed: FailedOutputs): string {
  if (failed.clipboard && failed.keyboard) return "Couldn't copy or type the text";
  if (failed.clipboard) return "Couldn't copy the text";
  if (failed.keyboard) return "Couldn't type the text";
  switch (context?.output_mode) {
    case 'clipboard': return "Couldn't copy the text";
    case 'keyboard':
    case 'both': return "Couldn't type the text";
    default: return "Couldn't deliver the text";
  }
}

function outputFailedText(context: CapsuleContext | null, failed: FailedOutputs): string {
  const text = failedOutputText(context, failed);
  // With history on, the text is not lost: the result is saved there either way.
  return context?.save_history ? `${text} · saved in History` : text;
}

/** The words beside the icon. Recording has none: its timer stands in for them. */
export function statusText(state: CapsuleState): string {
  switch (state.phase) {
    case 'hidden':
    case 'recording':
      return '';
    case 'transcribing':
      return 'Transcribing…';
    case 'processing':
      return processingText(state.command);
    case 'done':
      return doneText(state.context);
    case 'output-failed':
      return outputFailedText(state.context, state.failedOutputs);
    case 'error':
      return state.message || 'Something went wrong';
    case 'cancelled':
      return state.nothingHeard ? 'Nothing heard' : 'Cancelled';
  }
}

/** What the shortcut does right now, such as "⌃` to stop"; empty once there is nothing left to stop or cancel. */
export function hintText(state: CapsuleState): string {
  if (!state.context) return '';
  const keys = formatShortcut(state.context.shortcut).join('');
  switch (state.phase) {
    case 'recording':
      return `${keys} to stop`;
    case 'transcribing':
    case 'processing':
      return `${keys} to cancel`;
    default:
      return '';
  }
}

/** `65000` is `01:05`: whole seconds, with as many minute digits as the time needs. */
export function formatElapsed(ms: number): string {
  // The clock can step back during a recording, and the timer must not show it.
  const seconds = Math.floor(Math.max(0, ms) / 1000);
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${pad(Math.floor(seconds / 60))}:${pad(seconds % 60)}`;
}

/**
 * How long each ending stays on screen, from the moment the session ends
 * (`CapsuleState.ended`). Rust hides the window on the same schedule
 * (crates/lt-tauri/src/capsule.rs); keep the two together.
 */
export const HIDE_AFTER_MS = { done: 1500, 'output-failed': 4000, error: 4000, cancelled: 1000 } as const;

export const BAR_COUNT = 15;

/** How far the level moves toward each new sample: enough to follow the voice, too little to jitter. */
const SMOOTHING = 0.3;

/** A bar's height in percent: silence still leaves a stub, and a loud signal cannot overflow the capsule. */
const MIN_BAR = 8;
const MAX_BAR = 100;

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

/** The level after another `audio-level` sample, which is clamped to 0..1 first. */
export function smoothLevel(previous: number, rms: number): number {
  return previous + (clamp(rms, 0, 1) - previous) * SMOOTHING;
}

/** The height of every bar for a level: the old waveform's shape, two sine waves along the bars, on fewer bars. */
export function barHeights(level: number): number[] {
  return Array.from({ length: BAR_COUNT }, (_, i) => {
    const phase = (i / BAR_COUNT) * Math.PI * 2;
    const swell = 1 + Math.sin(phase) * 0.3 + Math.sin(phase * 1.5) * 0.2;
    return clamp(level * 100 * swell, MIN_BAR, MAX_BAR);
  });
}
