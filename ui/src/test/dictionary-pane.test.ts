import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync } from 'svelte';
import userEvent from '@testing-library/user-event';
import DictionaryEditor from '../components/settings/DictionaryEditor.svelte';
import { button, render, settle, unmountAll } from './helpers';

const mocks = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('../lib/tauri', () => ({ safeInvoke: mocks.invoke }));

/** A word as `get_dictionary` lists it. */
interface Entry {
  term: string;
  aliases: string[];
  description: string | null;
}

const entry = (term: string, aliases: string[] = [], description: string | null = null): Entry =>
  ({ term, aliases, description });

/**
 * A backend that remembers what it is told, the way the real one does: an added
 * word joins the list (the real one does not look for duplicates either), an
 * update replaces the word with that term, a delete removes it, and a word it
 * does not have is refused. Every other command succeeds.
 */
function backend(initial: Entry[] = []) {
  const state = { entries: initial.map(word => ({ ...word, aliases: [...word.aliases] })) };
  mocks.invoke.mockImplementation(async (command: string, args?: any) => {
    switch (command) {
      case 'get_dictionary':
        return { entries: state.entries.map(word => ({ ...word, aliases: [...word.aliases] })) };
      case 'add_dictionary_entry':
        state.entries.push(args.params);
        return undefined;
      case 'update_dictionary_entry': {
        const { old_term: oldTerm, ...updated } = args.params;
        const index = state.entries.findIndex(word => word.term === oldTerm);
        if (index < 0) throw new Error(`Entry '${oldTerm}' not found`);
        state.entries[index] = updated;
        return undefined;
      }
      case 'delete_dictionary_entry': {
        const index = state.entries.findIndex(word => word.term === args.term);
        if (index < 0) throw new Error(`Entry '${args.term}' not found`);
        state.entries.splice(index, 1);
        return undefined;
      }
      default: return undefined;
    }
  });
  return state;
}

/** Like `backend`, except `command` rejects with `error`. The page logs that, so the log is silenced. */
function failing(command: string, error: Error, initial: Entry[] = []) {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  backend(initial);
  const working = mocks.invoke.getMockImplementation()!;
  mocks.invoke.mockImplementation(async (name: string, args?: unknown) => {
    if (name === command) throw error;
    return working(name, args);
  });
}

/** `command` is answered, as `backend` would, only once the returned `release` is called; everything else is as `backend` made it. */
function holding(command: string) {
  let release!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  const working = mocks.invoke.getMockImplementation()!;
  mocks.invoke.mockImplementation(async (name: string, args?: unknown) => {
    if (name === command) {
      await held;
    }
    return working(name, args);
  });
  return release;
}

beforeEach(() => {
  backend();
});

afterEach(async () => {
  await unmountAll();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

async function open(initial?: Entry[]) {
  if (initial) backend(initial);
  const { target } = render(DictionaryEditor, {});
  await settle();
  return target;
}

const called = (command: string) => mocks.invoke.mock.calls.filter(([name]) => name === command);
const errorToast = (target: Element) => target.querySelector('.toast-error')?.textContent;
const successToast = (target: Element) => target.querySelector('.toast-success')?.textContent?.trim();
const dialog = (target: Element) => target.querySelector<HTMLElement>('[role="dialog"]');
const dialogTitle = (target: Element) => dialog(target)?.querySelector('h2')?.textContent;
const field = (target: Element, id: string) =>
  dialog(target)!.querySelector<HTMLInputElement | HTMLTextAreaElement>(`#${id}`)!;
const sheetError = (target: Element) => dialog(target)?.querySelector('.sheet-error')?.textContent;
const addButton = (target: Element) => target.querySelector<HTMLButtonElement>('button[aria-label="Add Word"]')!;
const searchField = (target: Element) => target.querySelector<HTMLInputElement>('input[type="search"]')!;

/** Every word of the list as [word, the line under it]. */
const rowsOf = (target: Element) => [...target.querySelectorAll('.group .row')].map(row => [
  row.querySelector('.row-label')?.textContent,
  row.querySelector('.row-detail')?.textContent ?? null,
]);
const wordsOf = (target: Element) => rowsOf(target).map(([word]) => word);

/** A sheet's buttons as [name, type], left to right. */
const buttonsOf = (container: Element) =>
  [...container.querySelectorAll('button')].map(b => [b.textContent?.trim(), b.type]);

function fill(input: HTMLInputElement | HTMLTextAreaElement, value: string) {
  input.value = value;
  input.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
}

/** What a form does when Enter is pressed in a field. */
function submit(target: Element) {
  dialog(target)!.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
}

describe('Dictionary pane: toolbar and list', () => {
  it('is titled Dictionary, with a search field and an Add Word button in its toolbar', async () => {
    const target = await open();
    expect(target.querySelector('h1')?.textContent).toBe('Dictionary');
    const toolbar = target.querySelector('header.toolbar')!;
    const search = toolbar.querySelector<HTMLInputElement>('input[type="search"]')!;
    expect(search.getAttribute('aria-label')).toBe('Search dictionary');
    expect(search.placeholder).toBe('Search');
    const add = toolbar.querySelector<HTMLButtonElement>('button[aria-label="Add Word"]')!;
    expect(add.title).toBe('Add Word');
    expect(add.type).toBe('button');
    // An icon and nothing else: its name is the label, so no text of its own to collide with a lookup by text.
    expect(add.textContent?.trim()).toBe('');
    expect(add.querySelector('svg')?.getAttribute('class')).toContain('lucide-plus');
  });

  it('lists each word, with who it is also heard as, or else its note, or nothing', async () => {
    const target = await open([
      entry('Tauri', ['tori', 'towery']),
      entry('Murmur', [], 'Our voice typing app'),
      entry('WasmEdge'),
      entry('Svelte', ['felt'], 'A framework'),
    ]);
    expect(rowsOf(target)).toEqual([
      ['Tauri', 'Also heard as: tori, towery'],
      ['Murmur', 'Our voice typing app'],
      ['WasmEdge', null],
      // The aliases are what the line is for; the note is in the sheet.
      ['Svelte', 'Also heard as: felt'],
    ]);
  });

  it('keeps every word in one group without a title, each row a button', async () => {
    const target = await open([entry('Tauri'), entry('Murmur')]);
    expect(target.querySelectorAll('.group')).toHaveLength(1);
    expect(target.querySelector('.group-title')).toBeNull();
    for (const row of target.querySelectorAll('.group .row')) {
      expect(row.querySelector('.row-main')?.tagName).toBe('BUTTON');
    }
  });

  it('lists a word twice when the dictionary holds it twice', async () => {
    // Nothing refuses a second copy of a word, so the term alone cannot tell two rows apart.
    const target = await open([entry('Tauri', ['tori']), entry('Tauri', ['towery']), entry('Murmur')]);
    expect(rowsOf(target)).toEqual([
      ['Tauri', 'Also heard as: tori'],
      ['Tauri', 'Also heard as: towery'],
      ['Murmur', null],
    ]);
    fill(searchField(target), 'tow'); await settle();
    expect(rowsOf(target)).toEqual([['Tauri', 'Also heard as: towery']]);
  });

  it('shows neither the list nor the empty state before the dictionary has loaded', async () => {
    backend([entry('Tauri')]);
    const release = holding('get_dictionary');
    const target = await open();
    // An empty list is only known once the backend has said so.
    expect(target.querySelector('.empty')).toBeNull();
    expect(target.querySelector('.group')).toBeNull();
    release(); await settle();
    expect(wordsOf(target)).toEqual(['Tauri']);
    expect(target.querySelector('.empty')).toBeNull();
  });

  it('says so when the dictionary cannot be loaded, and does not claim it is empty', async () => {
    failing('get_dictionary', new Error('unavailable'));
    const target = await open();
    expect(errorToast(target)).toContain('Failed to load dictionary');
    expect(errorToast(target)).toContain('unavailable');
    expect(target.querySelector('.empty')).toBeNull();
    // The toolbar still works, and starting something new takes the old failure away.
    addButton(target).click(); await settle();
    expect(dialogTitle(target)).toBe('Add Word');
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('treats a dictionary without an entry list as empty', async () => {
    mocks.invoke.mockImplementation(async (command: string) => (command === 'get_dictionary' ? {} : undefined));
    const target = await open();
    expect(target.querySelector('.toast')).toBeNull();
    expect(target.querySelector('.empty-title')?.textContent).toBe('No words yet');
  });
});

describe('Dictionary pane: empty and searching', () => {
  it('invites the first word when the dictionary is empty', async () => {
    const target = await open();
    const empty = target.querySelector('.empty')!;
    expect(empty.querySelector('.empty-title')?.textContent).toBe('No words yet');
    expect(empty.querySelector('.empty-hint')?.textContent)
      .toBe('Add names, jargon, or product terms so Murmur spells them correctly.');
    expect(target.querySelector('.group')).toBeNull();
    button(empty, 'Add Word…').click(); await settle();
    expect(dialogTitle(target)).toBe('Add Word');
    expect(document.activeElement).toBe(field(target, 'term'));
  });

  it('opens the same sheet from the toolbar button and from the empty state', async () => {
    const target = await open();
    addButton(target).click(); await settle();
    const fromToolbar = [dialogTitle(target), buttonsOf(dialog(target)!)];
    button(dialog(target)!, 'Cancel').click(); await settle();
    button(target.querySelector('.empty')!, 'Add Word…').click(); await settle();
    expect([dialogTitle(target), buttonsOf(dialog(target)!)]).toEqual(fromToolbar);
  });

  it('filters the list by word, alias or note as you type, ignoring case', async () => {
    const target = await open([
      entry('Tauri', ['tori']),
      entry('Murmur', ['mermer'], 'Voice typing'),
      entry('WasmEdge'),
    ]);
    fill(searchField(target), 'TAU'); await settle();
    expect(wordsOf(target)).toEqual(['Tauri']);
    fill(searchField(target), 'merm'); await settle();
    expect(wordsOf(target)).toEqual(['Murmur']);
    fill(searchField(target), 'TYPING'); await settle();
    expect(wordsOf(target)).toEqual(['Murmur']);
    fill(searchField(target), ''); await settle();
    expect(wordsOf(target)).toEqual(['Tauri', 'Murmur', 'WasmEdge']);
  });

  it('shows the whole list for a search of only spaces', async () => {
    const target = await open([entry('Tauri'), entry('Murmur')]);
    fill(searchField(target), '   '); await settle();
    expect(wordsOf(target)).toEqual(['Tauri', 'Murmur']);
    expect(target.querySelector('.empty')).toBeNull();
  });

  it('ignores spaces around the search, both in what it finds and in what it quotes when it finds nothing', async () => {
    const target = await open([entry('Tauri'), entry('Murmur')]);
    fill(searchField(target), 'Tauri '); await settle();
    expect(wordsOf(target)).toEqual(['Tauri']);
    fill(searchField(target), '  zzz '); await settle();
    expect(target.querySelector('.empty-hint')?.textContent).toBe('No words match “zzz”.');
  });

  it('says no word matches, quoting the search, and offers no Add Word… button', async () => {
    const target = await open([entry('Tauri')]);
    fill(searchField(target), 'zzz'); await settle();
    expect(target.querySelector('.empty')?.textContent?.trim()).toBe('No words match “zzz”.');
    expect(target.querySelector('.group')).toBeNull();
    expect([...target.querySelectorAll('button')].some(b => b.textContent?.includes('Add Word…'))).toBe(false);
    // The toolbar button is still there for it.
    expect(addButton(target)).not.toBeNull();
    fill(searchField(target), ''); await settle();
    expect(wordsOf(target)).toEqual(['Tauri']);
    expect(target.querySelector('.empty')).toBeNull();
  });

  it('shows the empty state again once the last word is gone', async () => {
    const target = await open([entry('Tauri')]);
    button(target, 'Tauri').click(); await settle();
    button(dialog(target)!, 'Delete…').click(); await settle();
    button(dialog(target)!, 'Delete').click(); await settle();
    expect(target.querySelector('.empty-title')?.textContent).toBe('No words yet');
  });
});

describe('Dictionary pane: adding a word', () => {
  it('opens the add sheet with the fields the spec gives, and the cursor in Word', async () => {
    const target = await open();
    addButton(target).click(); await settle();
    const sheet = dialog(target)!;
    expect(dialogTitle(target)).toBe('Add Word');
    expect([...sheet.querySelectorAll('label')].map(label => label.textContent))
      .toEqual(['Word', 'Also heard as', 'Note']);
    expect([...sheet.querySelectorAll('input, textarea')].map(control => [control.tagName, control.id]))
      .toEqual([['INPUT', 'term'], ['INPUT', 'aliases'], ['TEXTAREA', 'description']]);
    expect(field(target, 'description').placeholder).toBe('Optional');
    // Every label points at its own field.
    for (const label of sheet.querySelectorAll('label')) {
      expect(sheet.querySelector(`#${label.htmlFor}`)).not.toBeNull();
    }
    // The hint belongs to the field it explains.
    const hint = sheet.querySelector(`#${field(target, 'aliases').getAttribute('aria-describedby')}`);
    expect(hint?.textContent).toBe('Separate with commas');
    expect(buttonsOf(sheet)).toEqual([['Cancel', 'button'], ['Add Word', 'submit']]);
    expect(sheet.querySelector('.sheet-leading')).toBeNull();
    expect(sheet.querySelector('.sheet-error')).toBeNull();
    expect(document.activeElement).toBe(field(target, 'term'));
  });

  it('renders the sheet inside the pane', async () => {
    const target = await open();
    addButton(target).click(); await settle();
    expect(target.querySelector('.pane [role="dialog"]')).not.toBeNull();
  });

  it('adds the word, closes the sheet, and shows it in the list without a toast', async () => {
    const target = await open();
    addButton(target).click(); await settle();
    fill(field(target, 'term'), 'Murmur');
    fill(field(target, 'aliases'), 'murmur, mermer');
    fill(field(target, 'description'), 'Voice typing app');
    button(dialog(target)!, 'Add Word').click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('add_dictionary_entry', {
      params: { term: 'Murmur', aliases: ['murmur', 'mermer'], description: 'Voice typing app' },
    });
    expect(dialog(target)).toBeNull();
    expect(rowsOf(target)).toEqual([['Murmur', 'Also heard as: murmur, mermer']]);
    // The list changing is the confirmation.
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('clears the search once the word is added, so that the word shows in the list', async () => {
    const target = await open([entry('Tauri')]);
    fill(searchField(target), 'tau'); await settle();
    expect(wordsOf(target)).toEqual(['Tauri']);
    addButton(target).click(); await settle();
    fill(field(target, 'term'), 'Murmur');
    button(dialog(target)!, 'Add Word').click(); await settle();
    // The word does not match the search. Left on, it would hide the one change that confirms the add.
    expect(searchField(target).value).toBe('');
    expect(wordsOf(target)).toEqual(['Tauri', 'Murmur']);
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('keeps the search when the word cannot be added', async () => {
    failing('add_dictionary_entry', new Error('disk full'), [entry('Tauri')]);
    const target = await open();
    fill(searchField(target), 'tau'); await settle();
    addButton(target).click(); await settle();
    fill(field(target, 'term'), 'Murmur');
    button(dialog(target)!, 'Add Word').click(); await settle();
    expect(errorToast(target)).toContain('disk full');
    expect(searchField(target).value).toBe('tau');
    expect(wordsOf(target)).toEqual(['Tauri']);
  });

  it('splits Also heard as on the ASCII, fullwidth and ideographic commas', async () => {
    const target = await open();
    addButton(target).click(); await settle();
    fill(field(target, 'term'), 'Tauri');
    fill(field(target, 'aliases'), 'tori\uFF0Ctowery\u3001tauri app, tauri');
    button(dialog(target)!, 'Add Word').click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('add_dictionary_entry', {
      params: { term: 'Tauri', aliases: ['tori', 'towery', 'tauri app', 'tauri'], description: null },
    });
  });

  it('sends the word trimmed, only the aliases that are not blank, and null for an empty note', async () => {
    const target = await open();
    addButton(target).click(); await settle();
    fill(field(target, 'term'), '  Murmur  ');
    fill(field(target, 'aliases'), ' murmur ,, mermer,  ');
    fill(field(target, 'description'), '   ');
    button(dialog(target)!, 'Add Word').click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('add_dictionary_entry', {
      params: { term: 'Murmur', aliases: ['murmur', 'mermer'], description: null },
    });
  });

  it('adds the word on Enter in a field', async () => {
    const target = await open();
    addButton(target).click(); await settle();
    await userEvent.setup().type(field(target, 'term'), 'Murmur{Enter}');
    await settle();
    expect(called('add_dictionary_entry')).toHaveLength(1);
    expect(mocks.invoke).toHaveBeenCalledWith('add_dictionary_entry', {
      params: { term: 'Murmur', aliases: [], description: null },
    });
    expect(dialog(target)).toBeNull();
  });

  it('rejects an empty word inside the sheet', async () => {
    const target = await open();
    addButton(target).click(); await settle();
    button(dialog(target)!, 'Add Word').click(); await settle();
    expect(sheetError(target)).toBe('Enter a word.');
    expect(called('add_dictionary_entry')).toHaveLength(0);
    // Not in the toast: the sheet is where the user is looking.
    expect(target.querySelector('.toast')).toBeNull();
    expect(dialog(target)).not.toBeNull();
  });

  it('treats a word of only spaces as empty, and drops the message once something is typed', async () => {
    const target = await open();
    addButton(target).click(); await settle();
    fill(field(target, 'term'), '   ');
    submit(target); await settle();
    expect(sheetError(target)).toBe('Enter a word.');
    expect(called('add_dictionary_entry')).toHaveLength(0);
    fill(field(target, 'term'), 'M'); await settle();
    expect(sheetError(target)).toBeUndefined();
  });

  it('refuses a word the dictionary already has, ignoring case and spaces, inside the sheet', async () => {
    const target = await open([entry('Murmur')]);
    addButton(target).click(); await settle();
    fill(field(target, 'term'), ' murmur ');
    button(dialog(target)!, 'Add Word').click(); await settle();
    expect(sheetError(target)).toBe('That word is already in your dictionary.');
    expect(called('add_dictionary_entry')).toHaveLength(0);
    expect(target.querySelector('.toast')).toBeNull();
    expect(dialogTitle(target)).toBe('Add Word');
    // Typing clears it, as it clears "Enter a word."
    fill(field(target, 'term'), 'Murmur app'); await settle();
    expect(sheetError(target)).toBeUndefined();
  });

  it('forgets the message when the sheet is opened again', async () => {
    const target = await open();
    addButton(target).click(); await settle();
    button(dialog(target)!, 'Add Word').click(); await settle();
    expect(sheetError(target)).toBe('Enter a word.');
    button(dialog(target)!, 'Cancel').click(); await settle();
    addButton(target).click(); await settle();
    expect(sheetError(target)).toBeUndefined();
  });

  it('keeps typed input when the backdrop is clicked', async () => {
    const target = await open();
    addButton(target).click(); await settle();
    fill(field(target, 'term'), 'Murmur');
    await userEvent.setup().click(target.querySelector('.sheet-backdrop')!);
    await settle();
    expect(dialog(target)).not.toBeNull();
    expect(field(target, 'term').value).toBe('Murmur');
    // The press on the backdrop did not take the focus out of the sheet.
    expect(dialog(target)!.contains(document.activeElement)).toBe(true);
  });

  it('forgets what was typed when the sheet is cancelled', async () => {
    const target = await open();
    addButton(target).click(); await settle();
    fill(field(target, 'term'), 'Murmur');
    fill(field(target, 'aliases'), 'mermer');
    fill(field(target, 'description'), 'note');
    button(dialog(target)!, 'Cancel').click(); await settle();
    expect(dialog(target)).toBeNull();
    expect(called('add_dictionary_entry')).toHaveLength(0);
    addButton(target).click(); await settle();
    expect([field(target, 'term').value, field(target, 'aliases').value, field(target, 'description').value])
      .toEqual(['', '', '']);
  });

  it('closes on Escape and puts the focus back on the button that opened it', async () => {
    const target = await open([entry('Tauri')]);
    const user = userEvent.setup();
    await user.click(addButton(target)); await settle();
    fill(field(target, 'term'), 'Murmur');
    await user.keyboard('{Escape}'); await settle();
    expect(dialog(target)).toBeNull();
    expect(called('add_dictionary_entry')).toHaveLength(0);
    expect(document.activeElement).toBe(addButton(target));
  });

  it('keeps the sheet and what was typed, and says why in a toast, when the word cannot be added', async () => {
    failing('add_dictionary_entry', new Error('disk full'));
    const target = await open();
    addButton(target).click(); await settle();
    fill(field(target, 'term'), 'Murmur');
    button(dialog(target)!, 'Add Word').click(); await settle();
    expect(errorToast(target)).toContain('Failed to add word');
    expect(errorToast(target)).toContain('disk full');
    expect(dialog(target)).not.toBeNull();
    expect(field(target, 'term').value).toBe('Murmur');
    expect(sheetError(target)).toBeUndefined();
    expect(target.querySelector('.group')).toBeNull();
  });

  it('takes the failure with it when the sheet it came from is cancelled', async () => {
    failing('add_dictionary_entry', new Error('disk full'));
    const target = await open();
    addButton(target).click(); await settle();
    fill(field(target, 'term'), 'Murmur');
    button(dialog(target)!, 'Add Word').click(); await settle();
    expect(errorToast(target)).toContain('disk full');
    button(dialog(target)!, 'Cancel').click(); await settle();
    expect(dialog(target)).toBeNull();
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('adds the word once when it is submitted again while the first is still being saved', async () => {
    backend();
    const release = holding('add_dictionary_entry');
    const target = await open();
    addButton(target).click(); await settle();
    fill(field(target, 'term'), 'Murmur');
    const save = button(dialog(target)!, 'Add Word');
    save.click(); await settle();
    expect(save.disabled).toBe(true);
    submit(target); await settle();
    release(); await settle();
    expect(called('add_dictionary_entry')).toHaveLength(1);
    expect(dialog(target)).toBeNull();
    expect(wordsOf(target)).toEqual(['Murmur']);
  });
});

describe('Dictionary pane: editing a word', () => {
  it('edits a word from its row', async () => {
    const target = await open([entry('Tauri', ['tori'])]);
    button(target, 'Tauri').click(); await settle();
    expect(dialogTitle(target)).toBe('Edit Word');
    expect(field(target, 'term').value).toBe('Tauri');
    fill(field(target, 'aliases'), 'tori, towery');
    button(dialog(target)!, 'Save').click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('update_dictionary_entry', {
      params: { old_term: 'Tauri', term: 'Tauri', aliases: ['tori', 'towery'], description: null },
    });
  });

  it('opens with the word, who it is also heard as, and its note filled in', async () => {
    const target = await open([entry('Murmur', ['mermer']), entry('Tauri', ['tori', 'towery'], 'Desktop framework')]);
    // The row that was clicked, not the first one.
    button(target, 'Tauri').click(); await settle();
    expect(dialogTitle(target)).toBe('Edit Word');
    expect(field(target, 'term').value).toBe('Tauri');
    expect(field(target, 'aliases').value).toBe('tori, towery');
    expect(field(target, 'description').value).toBe('Desktop framework');
    expect(document.activeElement).toBe(field(target, 'term'));
    expect(target.querySelector('.pane [role="dialog"]')).not.toBeNull();
  });

  it('puts Delete… at the left and Cancel and Save at the right, Save being the default', async () => {
    const target = await open([entry('Tauri')]);
    button(target, 'Tauri').click(); await settle();
    const sheet = dialog(target)!;
    expect(buttonsOf(sheet.querySelector('.sheet-leading')!)).toEqual([['Delete…', 'button']]);
    expect(buttonsOf(sheet.querySelector('.sheet-actions')!)).toEqual([['Cancel', 'button'], ['Save', 'submit']]);
    expect(sheet.querySelector<HTMLButtonElement>('button[type="submit"]')?.textContent?.trim()).toBe('Save');
  });

  it('shows the change in the list once it is saved, without a toast', async () => {
    const target = await open([entry('Tauri', ['tori'])]);
    button(target, 'Tauri').click(); await settle();
    fill(field(target, 'aliases'), 'tori, towery');
    button(dialog(target)!, 'Save').click(); await settle();
    expect(dialog(target)).toBeNull();
    expect(rowsOf(target)).toEqual([['Tauri', 'Also heard as: tori, towery']]);
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('splits the aliases of an edited word the same way, and drops the empty ones', async () => {
    const target = await open([entry('Tauri')]);
    button(target, 'Tauri').click(); await settle();
    fill(field(target, 'aliases'), '\uFF0Ctori\u3001\u3001 towery \uFF0C, ');
    button(dialog(target)!, 'Save').click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('update_dictionary_entry', {
      params: { old_term: 'Tauri', term: 'Tauri', aliases: ['tori', 'towery'], description: null },
    });
  });

  it('keeps the search after a word is edited', async () => {
    const target = await open([entry('Tauri', ['tori']), entry('Murmur')]);
    fill(searchField(target), 'tau'); await settle();
    button(target, 'Tauri').click(); await settle();
    fill(field(target, 'aliases'), 'tori, towery');
    button(dialog(target)!, 'Save').click(); await settle();
    expect(searchField(target).value).toBe('tau');
    expect(rowsOf(target)).toEqual([['Tauri', 'Also heard as: tori, towery']]);
  });

  it('saves a renamed word under the term it had, with the note and aliases it was given', async () => {
    const target = await open([entry('Murmur'), entry('Tauri', ['tori'])]);
    button(target, 'Tauri').click(); await settle();
    fill(field(target, 'term'), '  Tauri 2 ');
    fill(field(target, 'description'), 'Desktop framework');
    button(dialog(target)!, 'Save').click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('update_dictionary_entry', {
      params: { old_term: 'Tauri', term: 'Tauri 2', aliases: ['tori'], description: 'Desktop framework' },
    });
    expect(wordsOf(target)).toEqual(['Murmur', 'Tauri 2']);
  });

  it('saves on Enter in a field, not Delete…', async () => {
    const target = await open([entry('Tauri')]);
    button(target, 'Tauri').click(); await settle();
    // Delete… comes first in the sheet, so it would be the form's default button if it were not a plain one.
    await userEvent.setup().type(field(target, 'aliases'), 'tori{Enter}');
    await settle();
    expect(called('update_dictionary_entry')).toHaveLength(1);
    expect(called('delete_dictionary_entry')).toHaveLength(0);
    expect(dialog(target)).toBeNull();
  });

  it('discards what was changed when the sheet is cancelled', async () => {
    const target = await open([entry('Tauri', ['tori'], 'note')]);
    button(target, 'Tauri').click(); await settle();
    fill(field(target, 'term'), 'Changed');
    fill(field(target, 'aliases'), 'other');
    fill(field(target, 'description'), 'else');
    button(dialog(target)!, 'Cancel').click(); await settle();
    expect(dialog(target)).toBeNull();
    expect(called('update_dictionary_entry')).toHaveLength(0);
    expect(rowsOf(target)).toEqual([['Tauri', 'Also heard as: tori']]);
    button(target, 'Tauri').click(); await settle();
    expect([field(target, 'term').value, field(target, 'aliases').value, field(target, 'description').value])
      .toEqual(['Tauri', 'tori', 'note']);
  });

  it('refuses a blank word inside the sheet', async () => {
    const target = await open([entry('Tauri')]);
    button(target, 'Tauri').click(); await settle();
    fill(field(target, 'term'), '  ');
    button(dialog(target)!, 'Save').click(); await settle();
    expect(sheetError(target)).toBe('Enter a word.');
    expect(called('update_dictionary_entry')).toHaveLength(0);
    expect(target.querySelector('.toast')).toBeNull();
    expect(dialogTitle(target)).toBe('Edit Word');
  });

  it('refuses renaming a word to another word it has, but lets a word change its own case', async () => {
    const target = await open([entry('Murmur'), entry('Tauri')]);
    button(target, 'Tauri').click(); await settle();
    fill(field(target, 'term'), 'MURMUR');
    button(dialog(target)!, 'Save').click(); await settle();
    expect(sheetError(target)).toBe('That word is already in your dictionary.');
    expect(called('update_dictionary_entry')).toHaveLength(0);

    fill(field(target, 'term'), 'tauri');
    button(dialog(target)!, 'Save').click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('update_dictionary_entry', {
      params: { old_term: 'Tauri', term: 'tauri', aliases: [], description: null },
    });
  });

  it('lets either copy of a word an older dictionary holds twice be edited, as long as its word stays', async () => {
    const target = await open([entry('Murmur'), entry('murmur')]);
    button(target, 'murmur').click(); await settle();
    fill(field(target, 'aliases'), 'mermer');
    button(dialog(target)!, 'Save').click(); await settle();
    expect(sheetError(target)).toBeUndefined();
    expect(mocks.invoke).toHaveBeenCalledWith('update_dictionary_entry', {
      params: { old_term: 'murmur', term: 'murmur', aliases: ['mermer'], description: null },
    });
  });

  it('forgets the message when the sheet is opened again', async () => {
    const target = await open([entry('Tauri')]);
    button(target, 'Tauri').click(); await settle();
    fill(field(target, 'term'), '  ');
    button(dialog(target)!, 'Save').click(); await settle();
    expect(sheetError(target)).toBe('Enter a word.');
    button(dialog(target)!, 'Cancel').click(); await settle();
    button(target, 'Tauri').click(); await settle();
    expect(sheetError(target)).toBeUndefined();
  });

  it('keeps the sheet and the list as they were, and says why, when the change cannot be saved', async () => {
    failing('update_dictionary_entry', new Error('disk full'), [entry('Tauri', ['tori'])]);
    const target = await open();
    button(target, 'Tauri').click(); await settle();
    fill(field(target, 'aliases'), 'towery');
    button(dialog(target)!, 'Save').click(); await settle();
    expect(errorToast(target)).toContain('Failed to save word');
    expect(errorToast(target)).toContain('disk full');
    expect(dialogTitle(target)).toBe('Edit Word');
    expect(field(target, 'aliases').value).toBe('towery');
    expect(rowsOf(target)).toEqual([['Tauri', 'Also heard as: tori']]);
  });

  it('saves the change once when it is submitted again while the first is still being saved', async () => {
    backend([entry('Tauri')]);
    const release = holding('update_dictionary_entry');
    const target = await open();
    button(target, 'Tauri').click(); await settle();
    fill(field(target, 'aliases'), 'tori');
    const save = button(dialog(target)!, 'Save');
    save.click(); await settle();
    expect(save.disabled).toBe(true);
    submit(target); await settle();
    release(); await settle();
    expect(called('update_dictionary_entry')).toHaveLength(1);
    expect(dialog(target)).toBeNull();
    expect(errorToast(target)).toBeUndefined();
  });

  it('clears the toast of an earlier failure when another sheet is opened over it', async () => {
    failing('update_dictionary_entry', new Error('disk full'), [entry('Tauri'), entry('Murmur')]);
    const target = await open();
    button(target, 'Tauri').click(); await settle();
    button(dialog(target)!, 'Save').click(); await settle();
    expect(errorToast(target)).toContain('disk full');
    // Choosing Delete… is a new action.
    button(dialog(target)!, 'Delete…').click(); await settle();
    expect(dialogTitle(target)).toBe('Delete “Tauri”?');
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('puts the focus back on the row of the word that was saved', async () => {
    const target = await open([entry('Murmur'), entry('Tauri', ['tori'])]);
    const user = userEvent.setup();
    await user.click(button(target, 'Tauri')); await settle();
    fill(field(target, 'aliases'), 'tori, towery');
    await user.click(button(dialog(target)!, 'Save')); await settle();
    expect(dialog(target)).toBeNull();
    // The list is read again after a save, but the row of a word that is still there stays what it was.
    expect(document.activeElement).toBe(button(target, 'Tauri'));
    expect(document.activeElement?.classList.contains('row-main')).toBe(true);
  });

  it('puts the focus on the Add Word button when the saved word was renamed, its old row being gone', async () => {
    const target = await open([entry('Tauri')]);
    const user = userEvent.setup();
    await user.click(button(target, 'Tauri')); await settle();
    fill(field(target, 'term'), 'Tauri 2');
    await user.click(button(dialog(target)!, 'Save')); await settle();
    expect(wordsOf(target)).toEqual(['Tauri 2']);
    expect(document.activeElement).toBe(addButton(target));
  });

  it('puts the focus back on the row when the sheet is closed with Escape', async () => {
    const target = await open([entry('Tauri')]);
    const user = userEvent.setup();
    const row = button(target, 'Tauri');
    await user.click(row); await settle();
    await user.keyboard('{Escape}'); await settle();
    expect(dialog(target)).toBeNull();
    expect(document.activeElement).toBe(row);
  });
});

describe('Dictionary pane: deleting a word', () => {
  it('deletes a word after confirming', async () => {
    const target = await open([entry('Tauri', ['tori']), entry('Murmur')]);
    button(target, 'Tauri').click(); await settle();
    button(dialog(target)!, 'Delete…').click(); await settle();
    expect(dialogTitle(target)).toBe('Delete “Tauri”?');
    // Asking is not deleting.
    expect(called('delete_dictionary_entry')).toHaveLength(0);
    button(dialog(target)!, 'Delete').click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('delete_dictionary_entry', { term: 'Tauri' });
    expect(dialog(target)).toBeNull();
    expect(successToast(target)).toContain('Deleted “Tauri”');
    expect(wordsOf(target)).toEqual(['Murmur']);
  });

  it('replaces the edit sheet with the confirmation, one sheet at a time, with the focus on Cancel', async () => {
    const target = await open([entry('Tauri')]);
    const user = userEvent.setup();
    await user.click(button(target, 'Tauri')); await settle();
    await user.click(button(dialog(target)!, 'Delete…')); await settle();
    expect(target.querySelectorAll('[role="dialog"]')).toHaveLength(1);
    expect(dialogTitle(target)).toBe('Delete “Tauri”?');
    expect(document.activeElement).toBe(button(dialog(target)!, 'Cancel'));
    // The trap still holds: Tab goes round inside the confirmation.
    await user.tab();
    expect(document.activeElement).toBe(button(dialog(target)!, 'Delete'));
    await user.tab();
    expect(document.activeElement).toBe(button(dialog(target)!, 'Cancel'));
  });

  it('asks with a Cancel that does nothing and a Delete that is the destructive default', async () => {
    const target = await open([entry('Tauri')]);
    button(target, 'Tauri').click(); await settle();
    button(dialog(target)!, 'Delete…').click(); await settle();
    expect(buttonsOf(dialog(target)!)).toEqual([['Cancel', 'button'], ['Delete', 'submit']]);
    expect(button(dialog(target)!, 'Delete').classList.contains('btn-destructive')).toBe(true);
    expect(dialog(target)!.querySelector('.sheet-leading')).toBeNull();
    expect(target.querySelector('.pane [role="dialog"]')).not.toBeNull();
  });

  it('closes the confirmation on Cancel without deleting, and does not bring the edit sheet back', async () => {
    const target = await open([entry('Tauri')]);
    button(target, 'Tauri').click(); await settle();
    button(dialog(target)!, 'Delete…').click(); await settle();
    button(dialog(target)!, 'Cancel').click(); await settle();
    expect(dialog(target)).toBeNull();
    expect(called('delete_dictionary_entry')).toHaveLength(0);
    expect(wordsOf(target)).toEqual(['Tauri']);
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('closes the confirmation on Escape without deleting', async () => {
    const target = await open([entry('Tauri')]);
    const user = userEvent.setup();
    await user.click(button(target, 'Tauri')); await settle();
    await user.click(button(dialog(target)!, 'Delete…')); await settle();
    await user.keyboard('{Escape}'); await settle();
    expect(dialog(target)).toBeNull();
    expect(called('delete_dictionary_entry')).toHaveLength(0);
  });

  it('cancels, never deletes, on the Enter that follows Delete…', async () => {
    const target = await open([entry('Tauri')]);
    const user = userEvent.setup();
    await user.click(button(target, 'Tauri')); await settle();
    await user.click(button(dialog(target)!, 'Delete…')); await settle();
    // The focus starts on Cancel, so a second Enter is the safe answer.
    await user.keyboard('{Enter}'); await settle();
    expect(dialog(target)).toBeNull();
    expect(called('delete_dictionary_entry')).toHaveLength(0);
    expect(wordsOf(target)).toEqual(['Tauri']);
  });

  it('deletes on Enter once the focus is on Delete', async () => {
    const target = await open([entry('Tauri')]);
    const user = userEvent.setup();
    await user.click(button(target, 'Tauri')); await settle();
    await user.click(button(dialog(target)!, 'Delete…')); await settle();
    await user.tab();
    await user.keyboard('{Enter}'); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('delete_dictionary_entry', { term: 'Tauri' });
  });

  it('deletes the word that is saved, not what was typed over it in the edit sheet', async () => {
    const target = await open([entry('Tauri')]);
    button(target, 'Tauri').click(); await settle();
    fill(field(target, 'term'), 'Something else');
    button(dialog(target)!, 'Delete…').click(); await settle();
    expect(dialogTitle(target)).toBe('Delete “Tauri”?');
    button(dialog(target)!, 'Delete').click(); await settle();
    expect(mocks.invoke).toHaveBeenCalledWith('delete_dictionary_entry', { term: 'Tauri' });
    expect(called('update_dictionary_entry')).toHaveLength(0);
    expect(wordsOf(target)).toEqual([]);
  });

  it('keeps the confirmation and the word, and says why, when the word cannot be deleted', async () => {
    failing('delete_dictionary_entry', new Error('disk full'), [entry('Tauri')]);
    const target = await open();
    button(target, 'Tauri').click(); await settle();
    button(dialog(target)!, 'Delete…').click(); await settle();
    button(dialog(target)!, 'Delete').click(); await settle();
    expect(errorToast(target)).toContain('Failed to delete word');
    expect(errorToast(target)).toContain('disk full');
    expect(dialogTitle(target)).toBe('Delete “Tauri”?');
    expect(target.querySelector('.toast-success')).toBeNull();
    expect(wordsOf(target)).toEqual(['Tauri']);
  });

  it('lets the confirmation go away by itself after two seconds', async () => {
    vi.useFakeTimers();
    const target = await open([entry('Tauri')]);
    button(target, 'Tauri').click(); await settle();
    button(dialog(target)!, 'Delete…').click(); await settle();
    button(dialog(target)!, 'Delete').click(); await settle();
    expect(successToast(target)).toContain('Deleted “Tauri”');
    vi.advanceTimersByTime(1900); await settle();
    expect(successToast(target)).toContain('Deleted “Tauri”');
    vi.advanceTimersByTime(200); await settle();
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('deletes the word once when it is confirmed again while the first is still being deleted', async () => {
    backend([entry('Tauri')]);
    const release = holding('delete_dictionary_entry');
    const target = await open();
    button(target, 'Tauri').click(); await settle();
    button(dialog(target)!, 'Delete…').click(); await settle();
    const confirm = button(dialog(target)!, 'Delete');
    confirm.click(); await settle();
    expect(confirm.disabled).toBe(true);
    submit(target); await settle();
    release(); await settle();
    expect(called('delete_dictionary_entry')).toHaveLength(1);
    // A second delete would have been refused as an unknown word.
    expect(errorToast(target)).toBeUndefined();
    expect(successToast(target)).toContain('Deleted “Tauri”');
  });

  it('clears the toast of the last deletion when the next sheet is opened', async () => {
    const target = await open([entry('Tauri'), entry('Murmur')]);
    button(target, 'Tauri').click(); await settle();
    button(dialog(target)!, 'Delete…').click(); await settle();
    button(dialog(target)!, 'Delete').click(); await settle();
    expect(successToast(target)).toContain('Deleted “Tauri”');
    button(target, 'Murmur').click(); await settle();
    expect(dialogTitle(target)).toBe('Edit Word');
    expect(target.querySelector('.toast')).toBeNull();
  });

  it('does not leave the focus on a page that has lost the row it came from', async () => {
    const target = await open([entry('Tauri'), entry('Murmur')]);
    const user = userEvent.setup();
    await user.click(button(target, 'Tauri')); await settle();
    await user.click(button(dialog(target)!, 'Delete…')); await settle();
    await user.click(button(dialog(target)!, 'Delete')); await settle();
    expect(wordsOf(target)).toEqual(['Murmur']);
    expect(document.activeElement).not.toBe(document.body);
    expect(target.contains(document.activeElement)).toBe(true);
  });

  it('puts the focus back on the row when the confirmation is cancelled', async () => {
    const target = await open([entry('Tauri')]);
    const user = userEvent.setup();
    const row = button(target, 'Tauri');
    await user.click(row); await settle();
    await user.click(button(dialog(target)!, 'Delete…')); await settle();
    await user.click(button(dialog(target)!, 'Cancel')); await settle();
    expect(document.activeElement).toBe(row);
  });
});
