import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  BAR_COUNT,
  HIDE_AFTER_MS,
  INITIAL_CAPSULE,
  barHeights,
  formatElapsed,
  hintText,
  reduce,
  smoothLevel,
  statusText,
  type CapsuleContext,
  type CapsuleEvent,
  type CapsulePhase,
  type CapsuleState,
} from '../components/capsule/capsuleState';
import { unmountAll } from './helpers';

afterEach(async () => {
  await unmountAll();
  vi.useRealTimers();
});

const ctx: CapsuleContext = { shortcut: 'Ctrl+`', output_mode: 'clipboard', save_history: true };
const other: CapsuleContext = { shortcut: 'Cmd+Shift+Space', output_mode: 'both', save_history: false };

// One constructor per event the component feeds the reducer, named after the Tauri event it comes from.
const pipelineState = (state: string, at = 0): CapsuleEvent => ({ type: 'state', state, at });
const capture = (capturing: boolean): CapsuleEvent => ({ type: 'capture', capturing });
const commandDetected = (name: string | null): CapsuleEvent => ({ type: 'command', name });
const pipelineResult: CapsuleEvent = { type: 'result' };
const pipelineError = (message: string): CapsuleEvent => ({ type: 'error', message });
const capsuleContext = (context: CapsuleContext): CapsuleEvent => ({ type: 'context', context });

/** Feed events to the reducer one after another, as the component does. */
const run = (events: CapsuleEvent[], from: CapsuleState = INITIAL_CAPSULE) =>
  events.reduce((current, event) => reduce(current, event), from);

// The sessions below follow the order the pipeline reports things in (crates/lt-pipeline/src/orchestrator.rs):
// a command is detected before Processing, an error is sent before the Error state, and a failed output is
// reported before the result.

/**
 * A recording that Stop ended before the first partial transcript: the microphone is closed, and the pipeline
 * reports Transcribing afterwards.
 */
const transcribing = (context: CapsuleContext = ctx) =>
  run([
    capsuleContext(context), pipelineState('recording', 1000), capture(true),
    capture(false), pipelineState('transcribing'),
  ]);

/** That recording while its text is processed. Its command was reported first, while it was still Transcribing. */
const processing = (context: CapsuleContext = ctx, command: string | null = null) =>
  run([commandDetected(command), pipelineState('processing')], transcribing(context));

const PHASES: CapsulePhase[] = [
  'hidden', 'recording', 'transcribing', 'processing', 'done', 'output-failed', 'error', 'cancelled',
];

/** One session in every phase, reached by the events that reach it in production. */
function sessions(): Record<CapsulePhase, CapsuleState> {
  const all = {
    hidden: INITIAL_CAPSULE,
    recording: run([capsuleContext(ctx), pipelineState('recording', 1000), capture(true)]),
    transcribing: transcribing(),
    processing: processing(),
    done: run([pipelineResult, pipelineState('done')], processing()),
    'output-failed': run(
      [pipelineError('Output failed: no access'), pipelineResult, pipelineState('done')],
      processing(),
    ),
    error: run([pipelineError('Boom'), pipelineState('error')], processing()),
    cancelled: run([pipelineState('idle')], processing()),
  };
  // A session that is not in the phase it is named after would make every table below wrong.
  for (const phase of PHASES) expect(all[phase].phase, `the ${phase} session`).toBe(phase);
  return all;
}

/**
 * Whether the session in each phase of `sessions()` has ended: the finished and the abandoned ones have, and so has
 * the one that failed. The ones under way have not, and a capsule that has seen nothing has no session to end.
 */
const ENDED: Record<CapsulePhase, boolean> = {
  hidden: false, recording: false, transcribing: false, processing: false,
  done: true, 'output-failed': true, error: true, cancelled: true,
};

/** A table in which every phase stays where it is; the tests name the phases that move. */
const unchanged = Object.fromEntries(PHASES.map(phase => [phase, phase])) as Record<CapsulePhase, CapsulePhase>;

/** A table in which every phase ends up in `target`; the tests name the exceptions. */
const everyPhaseTo = (target: CapsulePhase) =>
  Object.fromEntries(PHASES.map(phase => [phase, target])) as Record<CapsulePhase, CapsulePhase>;

/**
 * Check where `event` takes a session in each phase, once `before` has happened to it. Every phase is in the
 * table, so a phase that the event leaves alone is stated too.
 */
function expectPhases(event: CapsuleEvent, expected: Record<CapsulePhase, CapsulePhase>, before: CapsuleEvent[] = []) {
  const all = sessions();
  for (const phase of PHASES) {
    const next = run([...before, event], all[phase]);
    expect(next.phase, `${JSON.stringify([...before, event])} in ${phase}`).toBe(expected[phase]);
  }
}

/** A table in which every phase has ended, or none has. */
const everyEnded = (ended: boolean) =>
  Object.fromEntries(PHASES.map(phase => [phase, ended])) as Record<CapsulePhase, boolean>;

/** Like `expectPhases`, for whether the session in each phase has ended. */
function expectEnded(event: CapsuleEvent, expected: Record<CapsulePhase, boolean>, before: CapsuleEvent[] = []) {
  const all = sessions();
  for (const phase of PHASES) {
    const next = run([...before, event], all[phase]);
    expect(next.ended, `${JSON.stringify([...before, event])} in ${phase}`).toBe(expected[phase]);
  }
}

/**
 * Check that `event` changes exactly the fields in `change` of a session in any phase. `change` is a function of
 * the session when the change depends on it.
 */
function expectChange(
  event: CapsuleEvent,
  change: Partial<CapsuleState> | ((session: CapsuleState) => Partial<CapsuleState>),
) {
  for (const [phase, session] of Object.entries(sessions())) {
    const changed = typeof change === 'function' ? change(session) : change;
    expect(reduce(session, event), `${JSON.stringify(event)} in ${phase}`).toEqual({ ...session, ...changed });
  }
}

/** Freeze a value and everything it holds, so that any write to it throws in strict mode. */
function deepFreeze<T>(value: T): T {
  if (typeof value === 'object' && value !== null && !Object.isFrozen(value)) {
    Object.freeze(value);
    for (const child of Object.values(value)) deepFreeze(child);
  }
  return value;
}

describe('capsule state', () => {
  it('starts a fresh session on recording', () => {
    const finished: CapsuleState = {
      phase: 'done', capturing: false, startedAt: 10, command: 'shorten', outputFailed: true,
      failedOutputs: { clipboard: true, keyboard: false }, ended: true, message: 'Earlier error', context: ctx,
    };

    expect(reduce(finished, pipelineState('recording', 1000))).toEqual({
      phase: 'recording', capturing: true, startedAt: 1000, command: null, outputFailed: false,
      failedOutputs: { clipboard: false, keyboard: false }, ended: false, message: '', context: ctx,
    });
  });

  it('stays in Recording while the microphone is open', () => {
    const recording = run([capsuleContext(ctx), pipelineState('recording', 1000), pipelineState('transcribing')]);

    expect(recording.phase).toBe('recording');
    expect(hintText(recording)).toBe('⌃` to stop');
    // The timer stands in for the status text, and a capsule that is not showing has none.
    expect(statusText(recording)).toBe('');
    expect(statusText(INITIAL_CAPSULE)).toBe('');

    // Rust sends the context when it places the capsule, so it can arrive after the recording has begun.
    expect(hintText(run([pipelineState('recording', 1000), capsuleContext(ctx)]))).toBe('⌃` to stop');
    expectChange(capsuleContext(other), { context: other });

    // Closing the microphone is what ends Recording, and opening it again is only a flag...
    const closed = reduce(recording, capture(false));
    expect(closed.capturing).toBe(false);
    const reopened = reduce(closed, capture(true));
    expect(reopened).toEqual({ ...closed, capturing: true });
    expectChange(capture(true), { capturing: true });
    // ...until the pipeline's next Transcribing report makes it Recording again. Later phases do not go back.
    expect(reduce(reopened, pipelineState('transcribing')).phase).toBe('recording');
    expectPhases(pipelineState('transcribing'), { ...unchanged, hidden: 'recording', transcribing: 'recording' }, [
      capture(true),
    ]);
  });

  it('turns into Transcribing when the microphone closes', () => {
    const recording = run([capsuleContext(ctx), pipelineState('recording', 1000), capture(true)]);

    // Stop meets the pipeline's own Transcribing report in either order, or never meets it.
    const stops: [provider: string, events: CapsuleEvent[]][] = [
      ['a batch provider, which stays in Recording', [capture(false)]],
      ['a streaming provider with a partial result before Stop', [pipelineState('transcribing'), capture(false)]],
      ['a streaming provider whose first partial result follows Stop', [capture(false), pipelineState('transcribing')]],
    ];
    for (const [provider, events] of stops) {
      const closed = run(events, recording);
      expect(closed.phase, provider).toBe('transcribing');
      expect(closed.capturing, provider).toBe(false);
      expect(statusText(closed), provider).toBe('Transcribing…');
      expect(hintText(closed), provider).toBe('⌃` to cancel');
    }

    // A capsule that missed the start of the recording is told what the pipeline is doing; later phases keep theirs.
    expectPhases(pipelineState('transcribing'), { ...unchanged, hidden: 'transcribing' });
    // Closing the microphone moves a recording only, and leaves it closed whatever the phase.
    expectPhases(capture(false), { ...unchanged, recording: 'transcribing' });
    for (const [phase, session] of Object.entries(sessions())) {
      expect(reduce(session, capture(false)).capturing, phase).toBe(false);
    }
  });

  it('names the voice command while processing', () => {
    const wording: [command: string | null, status: string][] = [
      [null, 'Polishing…'],
      ['shorten', 'Shortening…'],
      ['formalize', 'Making it formal…'],
      ['casualize', 'Making it casual…'],
      ['reply', 'Writing a reply…'],
      ['translate to Traditional Chinese', 'Translating to Traditional Chinese…'],
      ['translate to Traditional Chinese (Taiwan)', 'Translating to Traditional Chinese (Taiwan)…'],
      ['a command from a newer backend', 'Processing…'],
    ];

    for (const [name, status] of wording) {
      // The command is detected before Processing is reported, so it arrives while the capsule is Transcribing.
      const named = processing(ctx, name);
      expect(named.phase, String(name)).toBe('processing');
      expect(named.command, String(name)).toBe(name);
      expect(statusText(named), String(name)).toBe(status);
      expect(hintText(named), String(name)).toBe('⌃` to cancel');
    }

    // A later detection replaces the earlier one, "no command" included.
    const changed = run([commandDetected('shorten'), commandDetected(null)], processing());
    expect(changed.command).toBeNull();
    expect(statusText(changed)).toBe('Polishing…');

    // Whatever the phase, a detection sets the command and nothing else.
    expectChange(commandDetected('shorten'), { command: 'shorten' });
  });

  it('leaves processing on the result alone', () => {
    const delivered = reduce(processing(), pipelineResult);
    expect(delivered.phase).toBe('done');
    expect(statusText(delivered)).toBe('Copied · ⌘V to paste');
    expect(hintText(delivered)).toBe('');

    // The state report that follows the result finds the session finished already.
    expect(reduce(delivered, pipelineState('done'))).toEqual(delivered);

    // What Done says depends on how the text was delivered, and on whether that is known yet.
    for (const [output_mode, status] of [['keyboard', 'Typed'], ['both', 'Typed and copied']] as const) {
      expect(statusText(reduce(processing({ ...ctx, output_mode }), pipelineResult)), output_mode).toBe(status);
    }
    expect(statusText(reduce(INITIAL_CAPSULE, pipelineResult))).toBe('Done');

    // A result ends every phase but an error, even one that had been cancelled. A state report of Done only
    // ends a session that is still going; a finished or cancelled one keeps its phase.
    expectPhases(pipelineResult, { ...everyPhaseTo('done'), 'output-failed': 'output-failed', error: 'error' });
    expectPhases(pipelineState('done'), {
      ...everyPhaseTo('done'), 'output-failed': 'output-failed', error: 'error', cancelled: 'cancelled',
    });

    // With a failed output on record, both end it as a failed output instead.
    const outputFailed = [pipelineError('Output failed: x')];
    expectPhases(pipelineResult, { ...everyPhaseTo('output-failed'), error: 'error' }, outputFailed);
    expectPhases(pipelineState('done'), {
      ...everyPhaseTo('output-failed'), done: 'done', error: 'error', cancelled: 'cancelled',
    }, outputFailed);
  });

  it('reports a failed output instead of Done', () => {
    const keyboard: CapsuleContext = { ...ctx, output_mode: 'keyboard', save_history: true };
    const failing = reduce(processing(keyboard), pipelineError('Output failed: no access'));
    // The error only marks the session; it is the result that ends it.
    expect(failing).toEqual({ ...processing(keyboard), outputFailed: true });
    expectChange(pipelineError('Output failed: x'), { outputFailed: true });

    const failed = reduce(failing, pipelineResult);
    expect(failed.phase).toBe('output-failed');
    expect(statusText(failed)).toBe("Couldn't type the text · saved in History");
    expect(hintText(failed)).toBe('');
    // The state report that follows finds the session finished already.
    expect(reduce(failed, pipelineState('done')).phase).toBe('output-failed');

    const copy = { ...ctx, output_mode: 'clipboard', save_history: false } as const;
    expect(statusText(run([pipelineError('Output failed: no access'), pipelineResult], processing(copy))))
      .toBe("Couldn't copy the text");

    // Typing and copying at once fails as typing. Without a context the capsule cannot tell how the text was
    // meant to go.
    const both = { ...ctx, output_mode: 'both', save_history: false } as const;
    expect(statusText(run([pipelineError('Output failed: x'), pipelineResult], processing(both))))
      .toBe("Couldn't type the text");
    expect(statusText(run([pipelineError('Output failed: x'), pipelineResult])))
      .toBe("Couldn't deliver the text");
  });

  it('words a failed output by the destinations the error names', () => {
    // lt-output names each destination that failed, so in Both mode one can fail while the other delivers.
    const both = { ...ctx, output_mode: 'both', save_history: false } as const;
    const failedWith = (message: string) =>
      statusText(run([pipelineError(message), pipelineResult], processing(both)));
    expect(failedWith('Output failed: Output error: clipboard: pasteboard unavailable'))
      .toBe("Couldn't copy the text");
    expect(failedWith('Output failed: Output error: keyboard: not trusted')).toBe("Couldn't type the text");
    expect(failedWith('Output failed: Output error: clipboard: pasteboard unavailable; keyboard: not trusted'))
      .toBe("Couldn't copy or type the text");

    expect(reduce(processing(both), pipelineError('Output failed: Output error: clipboard: x')))
      .toMatchObject({ outputFailed: true, failedOutputs: { clipboard: true, keyboard: false } });

    // The destination decides over the output mode, and History still gets its mention.
    const keyboard = { ...ctx, output_mode: 'keyboard', save_history: true } as const;
    expect(statusText(run([pipelineError('Output failed: Output error: clipboard: x'), pipelineResult],
      processing(keyboard)))).toBe("Couldn't copy the text · saved in History");
  });

  it('shows an error that arrives before any recording', () => {
    const message = 'ElevenLabs API key not configured. Please add your API key in Settings';
    const failed = reduce(INITIAL_CAPSULE, pipelineError(message));

    expect(failed.phase).toBe('error');
    expect(statusText(failed)).toBe(message);
    expect(hintText(failed)).toBe('');
    // Nothing was under way, so this is all there is to the session: it has ended.
    expect(failed.ended).toBe(true);

    // One line: the ends are trimmed and every run of whitespace, line breaks included, becomes one space.
    expect(reduce(INITIAL_CAPSULE, pipelineError('  Could not start:\n\n  no   microphone \t')).message)
      .toBe('Could not start: no microphone');
    // An error ends any phase, and puts its message in the capsule, replacing the one an error session has. It ends
    // the session too, unless one is under way: that goes on past an error.
    expectChange(pipelineError('Another failure'), session => ({
      phase: 'error', message: 'Another failure', ended: session.phase === 'hidden' || session.ended,
    }));

    // The pipeline state alone says it failed; without a message the capsule still says something.
    const stateOnly = reduce(INITIAL_CAPSULE, pipelineState('error'));
    expect(stateOnly.phase).toBe('error');
    expect(statusText(stateOnly)).toBe('Something went wrong');
    // The Error state ends any phase too, and the session with it, and takes nothing else with it, a failed output
    // included.
    expectChange(pipelineState('error'), { phase: 'error', ended: true });
    expectPhases(pipelineState('error'), everyPhaseTo('error'), [pipelineError('Output failed: x')]);
    // A message that follows the state is shown all the same.
    expect(statusText(run([pipelineState('error'), pipelineError('Late message')]))).toBe('Late message');

    // Only an error that starts with "Output failed" is a failed output; one that merely says so is an error.
    expect(reduce(INITIAL_CAPSULE, pipelineError('The provider said: Output failed')))
      .toMatchObject({ phase: 'error', outputFailed: false });
  });

  it('keeps an error over a later result', () => {
    const failed = run([capsuleContext(ctx), pipelineState('recording', 1000), pipelineError('Microphone unavailable')]);
    expect(failed.phase).toBe('error');

    expect(reduce(failed, pipelineResult).phase).toBe('error');
    // The same goes for the pipeline states that can follow an error. The Error state comes after the message.
    for (const name of ['transcribing', 'processing', 'done', 'error']) {
      expect(reduce(failed, pipelineState(name)).phase, name).toBe('error');
    }
    expect(statusText(reduce(failed, pipelineResult))).toBe('Microphone unavailable');
    expect(statusText(reduce(failed, pipelineState('error')))).toBe('Microphone unavailable');

    // A failure the pipeline survives: the error, then what it made of the text it had, then the Error state.
    const survived = run([
      pipelineError('Microphone unavailable'), commandDetected(null), pipelineState('processing'),
      pipelineResult, pipelineState('error'),
    ], transcribing());
    expect(survived.phase).toBe('error');
    expect(statusText(survived)).toBe('Microphone unavailable');

    // An error is the only phase a Processing report leaves alone.
    expectPhases(pipelineState('processing'), { ...everyPhaseTo('processing'), error: 'error' });
  });

  it('knows when the session has ended', () => {
    for (const [phase, session] of Object.entries(sessions())) {
      expect(session.ended, `the ${phase} session`).toBe(ENDED[phase as CapsulePhase]);
    }

    // The result and the pipeline's last states end a session in any phase, and a new recording starts one that has
    // not ended, over whatever the capsule was showing.
    expectEnded(pipelineResult, everyEnded(true));
    for (const name of ['done', 'error', 'idle']) expectEnded(pipelineState(name), everyEnded(true));
    expectEnded(pipelineState('recording', 1000), everyEnded(false));

    // Nothing else moves it: not the microphone, a voice command or the context, and not a failed output, which
    // only marks the session and waits for the result.
    for (const event of [
      capture(true), capture(false), commandDetected('shorten'), capsuleContext(other),
      pipelineError('Output failed: no access'),
    ]) {
      expectEnded(event, ENDED);
    }

    // An error is all there is to a session when none was under way, so it ends it. One that was under way is still
    // going afterwards.
    expectEnded(pipelineError('Microphone unavailable'), { ...ENDED, hidden: true });
  });

  it('keeps a session up through an error that it survives', () => {
    // A provider can fail partway through with text already in hand. The pipeline then goes on to process it: the
    // error, then the voice command and Processing, and the result only when the language model is done.
    const failing = run([pipelineError('HTTP STT error: 429 Too Many Requests')], transcribing());
    expect(failing).toMatchObject({ phase: 'error', ended: false });
    expect(statusText(failing)).toBe('HTTP STT error: 429 Too Many Requests');

    const processed = run([commandDetected(null), pipelineState('processing')], failing);
    expect(processed).toMatchObject({ phase: 'error', ended: false });

    // A second error from the same session, such as the language model failing, shows in place of the first and
    // does not end it either. Nor does a failed output, which only marks the session.
    const failedAgain = reduce(processed, pipelineError('LLM processing failed: timed out. Using raw transcription.'));
    expect(failedAgain).toMatchObject({
      phase: 'error', ended: false, message: 'LLM processing failed: timed out. Using raw transcription.',
    });
    expect(reduce(failedAgain, pipelineError('Output failed: no access')))
      .toMatchObject({ phase: 'error', ended: false, outputFailed: true });

    // The result ends it, and the Error state that follows it finds the session over already.
    const delivered = reduce(failedAgain, pipelineResult);
    expect(delivered).toMatchObject({ phase: 'error', ended: true });
    expect(reduce(delivered, pipelineState('error'))).toEqual(delivered);
    // With no text there is nothing to process, and the Error state alone ends it.
    expect(reduce(failing, pipelineState('error'))).toMatchObject({ phase: 'error', ended: true });
  });

  it('says Cancelled when the session returns to idle unfinished', () => {
    const cancelled = reduce(processing(), pipelineState('idle'));
    expect(cancelled.phase).toBe('cancelled');
    expect(statusText(cancelled)).toBe('Cancelled');
    expect(hintText(cancelled)).toBe('');

    // Any stage of a session can be abandoned. One that had finished, or never began, has nothing to cancel.
    expectPhases(pipelineState('idle'), {
      ...unchanged, recording: 'cancelled', transcribing: 'cancelled', processing: 'cancelled',
    });
  });

  it('a new recording replaces a finished result', () => {
    const done = reduce(processing(), pipelineResult);
    expect(done.phase).toBe('done');

    const again = reduce(done, pipelineState('recording', 5000));
    expect(again.phase).toBe('recording');
    expect(again.startedAt).toBe(5000);

    // Whatever the capsule is showing gives way, a result that is fading included, and only the context carries over.
    for (const [phase, session] of Object.entries(sessions())) {
      expect(reduce(session, pipelineState('recording', 7000)), phase).toEqual({
        phase: 'recording', capturing: true, startedAt: 7000, command: null, outputFailed: false,
        failedOutputs: { clipboard: false, keyboard: false }, ended: false, message: '', context: session.context,
      });
    }
  });

  it('does not change its input', () => {
    const events: CapsuleEvent[] = [
      ...['recording', 'transcribing', 'processing', 'done', 'error', 'idle', 'a state from a newer backend']
        .map(name => pipelineState(name, 1000)),
      capture(true), capture(false),
      commandDetected('shorten'), commandDetected(null),
      pipelineResult,
      pipelineError('Output failed: no access'), pipelineError('Microphone unavailable'),
      capsuleContext(other),
    ].map(deepFreeze);

    for (const phase of PHASES) {
      for (const capturing of [true, false]) {
        for (const outputFailed of [true, false]) {
          for (const ended of [true, false]) {
            const frozen = deepFreeze<CapsuleState>({
              phase, capturing, outputFailed, ended, startedAt: 1000, command: 'translate to French',
              failedOutputs: { clipboard: outputFailed, keyboard: false }, message: 'Earlier error', context: { ...ctx },
            });
            for (const event of events) {
              expect(() => reduce(frozen, event), `${phase} + ${JSON.stringify(event)}`).not.toThrow();
            }
          }
        }
      }
    }

    // Every session starts from the same object, so it must come out of all of the above as it went in.
    expect(INITIAL_CAPSULE).toEqual({
      phase: 'hidden', capturing: false, startedAt: null, command: null, outputFailed: false,
      failedOutputs: { clipboard: false, keyboard: false }, ended: false, message: '', context: null,
    });

    // A pipeline state the capsule does not know leaves it as it is, whatever the phase.
    expectChange(pipelineState('a state from a newer backend'), {});
  });

  it('formats the elapsed time as mm:ss', () => {
    expect(formatElapsed(0)).toBe('00:00');
    expect(formatElapsed(65000)).toBe('01:05');
    expect(formatElapsed(600000)).toBe('10:00');

    // Whole seconds only; the minutes have no upper limit; and a clock that stepped back shows no negative time.
    expect(formatElapsed(999)).toBe('00:00');
    expect(formatElapsed(59999)).toBe('00:59');
    expect(formatElapsed(6_000_000)).toBe('100:00');
    expect(formatElapsed(-1500)).toBe('00:00');
  });

  it('keeps the waveform bounded and its derivation pure', () => {
    expect(BAR_COUNT).toBe(15);

    const bars = barHeights(0.5);
    expect(bars).toHaveLength(15);
    expect(bars.every(height => height >= 8 && height <= 100)).toBe(true);
    expect(new Set(bars).size).toBeGreaterThan(1);
    expect(barHeights(0.5)).toEqual(bars);

    // The shape is the old waveform's: the level, bent by two sine waves along the bars.
    bars.forEach((height, i) => {
      const phase = (i / 15) * 2 * Math.PI;
      expect(height, `bar ${i}`).toBeCloseTo(50 * (1 + 0.3 * Math.sin(phase) + 0.2 * Math.sin(1.5 * phase)), 10);
    });

    // Silence keeps a visible stub, and a level above the scale cannot overflow the capsule.
    expect(barHeights(0).every(height => height === 8)).toBe(true);
    expect(barHeights(5).every(height => height <= 100)).toBe(true);

    // The level approaches the signal by 30% of the gap, and the signal is clamped to 0..1 first.
    expect(smoothLevel(0, 1)).toBe(0.3);
    expect(smoothLevel(0, 5)).toBe(0.3);
    expect(smoothLevel(1, -3)).toBeCloseTo(0.7);
  });

  it("hides after the spec's delays", () => {
    expect(HIDE_AFTER_MS).toEqual({ done: 1500, 'output-failed': 4000, error: 4000, cancelled: 1000 });
  });
});
