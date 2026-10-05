import { afterEach, describe, expect, it, vi } from 'vitest';
import { buildHotkey, formatShortcut, hasModifier, type KeyEventLike } from '../lib/shortcut';
import { unmountAll } from './helpers';

afterEach(async () => {
  await unmountAll();
  vi.useRealTimers();
});

/** A key press as the webview reports it: the physical key's `code`, and the character it typed. */
const key = (code: string, key: string, mods: Partial<KeyEventLike> = {}) =>
  ({ code, key, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...mods });

// The stored strings `buildHotkey` is expected to build below are the ones
// `the_backend_parses_every_shortcut_the_recorder_builds` (crates/lt-tauri/src/main.rs) feeds to
// the backend's parser. Change the two lists together.
describe('shortcut', () => {
  it('builds the stored format in Cmd, Ctrl, Alt, Shift order', () => {
    expect(buildHotkey(key('Backquote', '`', { ctrlKey: true }))).toBe('Ctrl+`');
    expect(buildHotkey(key('Space', ' ', { metaKey: true, shiftKey: true }))).toBe('Cmd+Shift+Space');
    expect(buildHotkey(key('KeyK', 'k', { metaKey: true, ctrlKey: true, altKey: true, shiftKey: true }))).toBe('Cmd+Ctrl+Alt+Shift+K');
  });

  it('names the physical key, so a shifted symbol never reaches the backend', () => {
    // Shift+= types `+`, and the backend splits a shortcut on `+`.
    expect(buildHotkey(key('Equal', '+', { ctrlKey: true, shiftKey: true }))).toBe('Ctrl+Shift+=');
    expect(buildHotkey(key('Digit1', '!', { metaKey: true, shiftKey: true }))).toBe('Cmd+Shift+1');
  });

  it('names the physical key, so an Option-composed character never reaches the backend', () => {
    expect(buildHotkey(key('KeyK', '˚', { altKey: true }))).toBe('Alt+K');
    // Option+E types nothing yet: it starts an accent.
    expect(buildHotkey(key('KeyE', 'Dead', { altKey: true }))).toBe('Alt+E');
  });

  it('writes each punctuation key as the character the backend reads', () => {
    const punctuation: Record<string, string> = {
      Backquote: '`', Minus: '-', Equal: '=', BracketLeft: '[', BracketRight: ']', Backslash: '\\',
      Semicolon: ';', Quote: "'", Comma: ',', Period: '.', Slash: '/',
    };
    for (const [code, character] of Object.entries(punctuation)) {
      expect(buildHotkey(key(code, character, { ctrlKey: true })), code).toBe(`Ctrl+${character}`);
    }
  });

  it('keeps the name of any other key as the webview reports it', () => {
    expect(buildHotkey(key('ArrowUp', 'ArrowUp', { metaKey: true }))).toBe('Cmd+ArrowUp');
    // The keypad's 1 is not the row's 1, and the backend tells them apart.
    expect(buildHotkey(key('Numpad1', '1', { ctrlKey: true }))).toBe('Ctrl+Numpad1');
  });

  it('falls back to the typed character when the event names no physical key', () => {
    const all = { metaKey: true, ctrlKey: true, altKey: true, shiftKey: true };
    expect(buildHotkey(key('', 'k', all))).toBe('Cmd+Ctrl+Alt+Shift+K');
    expect(buildHotkey(key('Unidentified', ' ', { metaKey: true, shiftKey: true }))).toBe('Cmd+Shift+Space');
    expect(buildHotkey(key('', 'ArrowUp', { metaKey: true }))).toBe('Cmd+ArrowUp');
  });

  it('ignores a modifier pressed on its own', () => {
    expect(buildHotkey(key('ShiftLeft', 'Shift', { shiftKey: true }))).toBeNull();
    expect(buildHotkey(key('MetaRight', 'Meta', { metaKey: true }))).toBeNull();
    // Without a code, the modifier's name says it.
    expect(buildHotkey(key('', 'Shift', { shiftKey: true }))).toBeNull();
    // A layout can report a modifier under another name; its code still says it.
    expect(buildHotkey(key('AltRight', 'AltGraph', { altKey: true }))).toBeNull();
    expect(buildHotkey(key('ControlLeft', 'Unidentified', { ctrlKey: true }))).toBeNull();
  });

  it('requires a modifier', () => {
    expect(hasModifier('Ctrl+`')).toBe(true);
    expect(hasModifier('K')).toBe(false);
  });

  it('shows macOS symbols in ⌃ ⌥ ⇧ ⌘ order', () => {
    expect(formatShortcut('Ctrl+`')).toEqual(['⌃', '`']);
    expect(formatShortcut('Cmd+Shift+Space')).toEqual(['⇧', '⌘', 'Space']);
    expect(formatShortcut('Cmd+Ctrl+Alt+Shift+K')).toEqual(['⌃', '⌥', '⇧', '⌘', 'K']);
    expect(formatShortcut('CommandOrControl+Option+L')).toEqual(['⌥', '⌘', 'L']);
  });

  it('keeps a key it has no symbol for as written', () => {
    expect(formatShortcut('Alt+ArrowUp')).toEqual(['⌥', 'ArrowUp']);
    expect(formatShortcut('F5')).toEqual(['F5']);
    expect(formatShortcut('Ctrl+Shift+=')).toEqual(['⌃', '⇧', '=']);
  });

  it('shows a stored shortcut that ends in + as written, though the backend rejects it', () => {
    // Display only: a hand-edited config can hold this, and the Rust test asserts the backend refuses it.
    expect(formatShortcut('Ctrl+Shift++')).toEqual(['⌃', '⇧', '+']);
  });
});
