/**
 * The settings window's panes: what the sidebar lists, in order, and how a
 * window URL picks the first one shown.
 */

export type PaneId = 'general' | 'transcription' | 'ai' | 'dictionary' | 'about';

export interface PaneInfo {
  id: PaneId;
  label: string;
  /** The sidebar sets the two groups apart with a gap. */
  group: 1 | 2;
}

export const PANES: readonly PaneInfo[] = [
  { id: 'general', label: 'General', group: 1 },
  { id: 'transcription', label: 'Transcription', group: 1 },
  { id: 'ai', label: 'AI Processing', group: 1 },
  { id: 'dictionary', label: 'Dictionary', group: 1 },
  { id: 'about', label: 'About', group: 2 },
];

/** The pane `value` names, or null for anything that is not one. */
export function parsePane(value: string | null | undefined): PaneId | null {
  return PANES.find((pane) => pane.id === value)?.id ?? null;
}

/**
 * Where a window opened with `search` starts. `pane` picks the pane; with no
 * `pane`, `action=check-update` means About. A `pane` that names nothing drops
 * the whole route, `action` included, the same as a `navigate` event for it.
 */
export function initialRoute(search: string): { pane: PaneId; checkUpdate: boolean } {
  const params = new URLSearchParams(search);
  const checkUpdate = params.get('action') === 'check-update';
  const requested = params.get('pane');
  if (requested === null) return { pane: checkUpdate ? 'about' : 'general', checkUpdate };
  const pane = parsePane(requested);
  return pane ? { pane, checkUpdate } : { pane: 'general', checkUpdate: false };
}
