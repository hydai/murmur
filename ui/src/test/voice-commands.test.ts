import { afterEach, describe, expect, it, vi } from 'vitest';
import { VOICE_COMMANDS, voiceCommand } from '../lib/voiceCommands';
import { unmountAll } from './helpers';

afterEach(async () => {
  await unmountAll();
  vi.useRealTimers();
});

describe('voice commands', () => {
  it('lists the five commands in the order the pane shows them', () => {
    expect(VOICE_COMMANDS.map((command) => command.prompt))
      .toEqual(['post_process', 'shorten', 'change_tone', 'generate_reply', 'translate']);
    expect(VOICE_COMMANDS.map((command) => command.title))
      .toEqual(['Clean Up', 'Shorten', 'Change Tone', 'Reply', 'Translate']);
  });

  it('words each command the way the spec does, curly quotes and all', () => {
    expect(VOICE_COMMANDS.map(({ title, detail, usage }) => [title, detail, usage])).toEqual([
      ['Clean Up', 'Every transcription without a command', 'Used for every transcription without a command.'],
      ['Shorten', 'Say “shorten: …”', 'Used when you start with “shorten:”.'],
      [
        'Change Tone',
        'Say “make it formal: …” or “make it casual: …”',
        'Used when you start with “make it formal:” or “make it casual:”.',
      ],
      ['Reply', 'Say “reply to: …”', 'Used when you start with “reply to:”.'],
      ['Translate', 'Say “translate to Japanese: …”', 'Used when you start with “translate to <language>:”.'],
    ]);
  });

  it('finds the command of each prompt', () => {
    for (const command of VOICE_COMMANDS) {
      expect(voiceCommand(command.prompt)).toBe(command);
    }
    expect(voiceCommand('generate_reply').title).toBe('Reply');
  });
});
