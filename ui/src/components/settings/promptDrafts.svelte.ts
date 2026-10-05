import type { PromptName } from '../../lib/voiceCommands';

/**
 * Edits that have not been saved, one per prompt, for as long as the window
 * lives. The editor is a subpage, so leaving it or switching to another pane
 * destroys it; the work in progress has to be somewhere that outlasts it, and
 * the Voice Commands list reads it to mark the prompt "Unsaved".
 *
 * An empty text is a draft too: the user may have cleared the editor on
 * purpose, and that must not read as "no draft, show the stored prompt".
 */
let drafts = $state<Partial<Record<PromptName, string>>>({});

export function getDraft(name: PromptName): string | undefined {
  return drafts[name];
}

export function setDraft(name: PromptName, content: string): void {
  drafts[name] = content;
}

export function clearDraft(name: PromptName): void {
  delete drafts[name];
}

export function hasDraft(name: PromptName): boolean {
  return drafts[name] !== undefined;
}

/** Forget every draft. For tests only: a draft otherwise ends with a save or a restore. */
export function resetDrafts(): void {
  drafts = {};
}
