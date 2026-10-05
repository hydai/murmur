/** The prompt templates the backend keeps, by the name `get_prompts` and `set_prompt` know them by. */
export type PromptName = 'post_process' | 'shorten' | 'change_tone' | 'generate_reply' | 'translate';

export interface VoiceCommand {
  prompt: PromptName;
  /** What the pane calls it, and the title of its editor. */
  title: string;
  /** What to say, under the title in the list. */
  detail: string;
  /** When the prompt is used; the editor puts it after the prompt's own description. */
  usage: string;
}

/**
 * The commands in the order the pane lists them. The prefixes are the ones
 * `detect_command` matches, which the user would otherwise have to find in the
 * documentation.
 */
export const VOICE_COMMANDS: readonly VoiceCommand[] = [
  {
    prompt: 'post_process',
    title: 'Clean Up',
    detail: 'Every transcription without a command',
    usage: 'Used for every transcription without a command.',
  },
  {
    prompt: 'shorten',
    title: 'Shorten',
    detail: 'Say “shorten: …”',
    usage: 'Used when you start with “shorten:”.',
  },
  {
    prompt: 'change_tone',
    title: 'Change Tone',
    detail: 'Say “make it formal: …” or “make it casual: …”',
    usage: 'Used when you start with “make it formal:” or “make it casual:”.',
  },
  {
    prompt: 'generate_reply',
    title: 'Reply',
    detail: 'Say “reply to: …”',
    usage: 'Used when you start with “reply to:”.',
  },
  {
    prompt: 'translate',
    title: 'Translate',
    detail: 'Say “translate to Japanese: …”',
    usage: 'Used when you start with “translate to <language>:”.',
  },
];

export function voiceCommand(prompt: PromptName): VoiceCommand {
  const command = VOICE_COMMANDS.find((candidate) => candidate.prompt === prompt);
  if (!command) throw new Error(`No voice command uses the prompt ${prompt}`);
  return command;
}
