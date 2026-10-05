/**
 * A shortcut is stored as `Cmd+Ctrl+Alt+Shift+Key` and shown the way macOS
 * writes it: ⌃ ⌥ ⇧ ⌘, then the key.
 *
 * The key is the physical key that was pressed (`event.code`), not the
 * character it typed. The backend registers a physical key and parses only the
 * names it knows, and a typed character can be neither: Shift+= types `+`,
 * which it reads as a separator, and Option+K types `˚`, which it has no name
 * for. They are stored as `Shift+=` and `Alt+K`.
 */

export type KeyEventLike = Pick<KeyboardEvent, 'key' | 'code' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey'>;

export const MISSING_MODIFIER_MESSAGE = 'Include at least one modifier key (⌃ ⌥ ⇧ ⌘).';

/** Pressed on their own these are not a shortcut yet; the user is still reaching for the key. */
const MODIFIER_KEYS = new Set(['Meta', 'Control', 'Alt', 'Shift']);

/** The same modifiers by physical key: a layout can report one under another `key` (AltGraph), never another `code`. */
const MODIFIER_CODES = new Set([
  'MetaLeft', 'MetaRight', 'ControlLeft', 'ControlRight', 'AltLeft', 'AltRight', 'ShiftLeft', 'ShiftRight',
]);

/**
 * The punctuation keys, written as the character the backend also accepts for
 * them, so that the stored form stays readable. Other keys are stored by name.
 */
const PUNCTUATION = new Map([
  ['Backquote', '`'],
  ['Minus', '-'],
  ['Equal', '='],
  ['BracketLeft', '['],
  ['BracketRight', ']'],
  ['Backslash', '\\'],
  ['Semicolon', ';'],
  ['Quote', "'"],
  ['Comma', ','],
  ['Period', '.'],
  ['Slash', '/'],
]);

/**
 * macOS order, with every spelling the backend accepts for each modifier.
 * The stored format only ever uses Cmd, Ctrl, Alt, and Shift.
 */
const MODIFIERS: readonly (readonly [symbol: string, spellings: readonly string[]])[] = [
  ['⌃', ['Ctrl', 'Control']],
  ['⌥', ['Alt', 'Option']],
  ['⇧', ['Shift']],
  ['⌘', ['Cmd', 'Command', 'Super', 'Meta', 'CommandOrControl', 'CmdOrCtrl']],
];

const isModifier = (token: string) => MODIFIERS.some(([, spellings]) => spellings.includes(token));

/**
 * Split a stored shortcut into its modifier tokens and the key. A trailing `+`
 * is read as the key, so that a hand-edited config still shows what it holds;
 * the backend cannot register one, and the recorder builds one only for an
 * event that names no physical key.
 */
function parse(hotkey: string): { modifiers: string[]; key: string } {
  const match = /^((?:[^+]+\+)*)(.+)$/.exec(hotkey);
  if (!match) return { modifiers: [], key: hotkey };
  return { modifiers: match[1].split('+').filter(Boolean), key: match[2] };
}

/** The key's stored name: its physical key, which no modifier changes. */
function keyName(event: KeyEventLike): string {
  const { code } = event;
  if (code && code !== 'Unidentified') {
    const letterOrDigit = /^(?:Key([A-Z])|Digit([0-9]))$/.exec(code);
    if (letterOrDigit) return letterOrDigit[1] ?? letterOrDigit[2];
    return PUNCTUATION.get(code) ?? code;
  }

  // No physical key to name (a synthetic event, some input methods), so the
  // character it typed is all there is.
  if (event.key === ' ') return 'Space';
  return event.key.length === 1 ? event.key.toUpperCase() : event.key;
}

/** The stored form of a key press, or `null` while only a modifier is held. */
export function buildHotkey(event: KeyEventLike): string | null {
  if (MODIFIER_KEYS.has(event.key) || MODIFIER_CODES.has(event.code)) return null;

  const parts: string[] = [];
  if (event.metaKey) parts.push('Cmd');
  if (event.ctrlKey) parts.push('Ctrl');
  if (event.altKey) parts.push('Alt');
  if (event.shiftKey) parts.push('Shift');

  parts.push(keyName(event));
  return parts.join('+');
}

export function hasModifier(hotkey: string): boolean {
  return parse(hotkey).modifiers.some(isModifier);
}

/** `Cmd+Shift+K` becomes `['⇧', '⌘', 'K']`, one entry per key cap. */
export function formatShortcut(hotkey: string): string[] {
  const { modifiers, key } = parse(hotkey);
  const symbols = MODIFIERS
    .filter(([, spellings]) => modifiers.some(token => spellings.includes(token)))
    .map(([symbol]) => symbol);
  // A spelling we do not know is shown as written rather than dropped.
  const unknown = modifiers.filter(token => !isModifier(token));
  return [...symbols, ...unknown, key];
}
