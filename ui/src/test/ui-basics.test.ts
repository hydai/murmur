import { afterEach, describe, expect, it, vi } from 'vitest';
import { createRawSnippet } from 'svelte';
import userEvent from '@testing-library/user-event';
import { Laptop } from 'lucide-svelte';
import Group from '../components/ui/Group.svelte';
import Row from '../components/ui/Row.svelte';
import Select from '../components/ui/Select.svelte';
import Switch from '../components/ui/Switch.svelte';
import RowHarness from './RowHarness.svelte';
import { render, settle, unmountAll } from './helpers';

afterEach(async () => {
  await unmountAll();
  vi.useRealTimers();
});

describe('Row', () => {
  it('keeps selection and trailing actions independently keyboard accessible', async () => {
    const select = vi.fn();
    const edit = vi.fn();
    const { target } = render(RowHarness, { select, edit });
    const user = userEvent.setup();
    expect(target.querySelector('button button')).toBeNull();
    await user.tab();
    expect(document.activeElement?.textContent).toContain('Cloud provider');
    await user.keyboard('{Enter}');
    expect(select).toHaveBeenCalledTimes(1);
    await user.tab();
    expect(document.activeElement?.textContent).toBe('Edit key');
    await user.keyboard(' ');
    expect(edit).toHaveBeenCalledTimes(1);
    expect(select).toHaveBeenCalledTimes(1);
  });

  it('marks the row in use for assistive technology', () => {
    const { target } = render(RowHarness, { select: vi.fn(), edit: vi.fn(), current: true });
    const main = target.querySelector('.row-main')!;
    expect(main.getAttribute('aria-current')).toBe('true');
    expect(main.textContent).toContain('In use');
  });

  it('keeps the full detail available when it is truncated', () => {
    const detail = 'https://very-long-host.example.com/v1/audio/transcriptions/with/a/long/path';
    const { target } = render(RowHarness, { select: vi.fn(), edit: vi.fn(), detail });
    expect(target.querySelector('.row-detail')?.getAttribute('title')).toBe(detail);
  });

  it('is not a button when there is nothing to select', () => {
    const { target } = render(Row, { label: 'Shortcut' });
    expect(target.querySelector('button')).toBeNull();
    expect(target.querySelector('div.row-main')?.textContent).toContain('Shortcut');
  });

  it('disables selection without hiding the row', () => {
    const onclick = vi.fn();
    const { target } = render(Row, { label: 'Apple Speech', detail: 'Requires macOS 26', disabled: true, onclick });
    const main = target.querySelector<HTMLButtonElement>('button.row-main')!;
    expect(main.disabled).toBe(true);
    main.click();
    expect(onclick).not.toHaveBeenCalled();
    expect(main.textContent).toContain('Requires macOS 26');
  });

  it('draws a lucide icon in a tile beside the label', () => {
    const { target } = render(Row, { label: 'Apple Speech', icon: Laptop });
    expect(target.querySelector('.row-main .row-icon svg')).not.toBeNull();
  });

  it('puts extra content under the main area, outside the button', () => {
    const progress = createRawSnippet(() => ({ render: () => '<div class="progress">Downloading… 40%</div>' }));
    const { target } = render(Row, { label: 'Apple Speech', onclick: vi.fn(), children: progress });
    expect(target.querySelector('.row > .row-extra .progress')).not.toBeNull();
    expect(target.querySelector('.row-main .progress')).toBeNull();
  });
});

describe('Switch', () => {
  it('flips a switch through its callback', () => {
    const onchange = vi.fn();
    const { target } = render(Switch, { checked: true, label: 'Save transcription history', onchange });
    const control = target.querySelector<HTMLButtonElement>('[role="switch"]')!;
    expect(control.getAttribute('aria-checked')).toBe('true');
    control.click();
    expect(onchange).toHaveBeenCalledWith(false);
  });

  it('states that an off switch is off and turns it on through its callback', () => {
    const onchange = vi.fn();
    const { target } = render(Switch, { checked: false, label: 'Save transcription history', onchange });
    const control = target.querySelector<HTMLButtonElement>('[role="switch"]')!;
    // role="switch" requires aria-checked, so "false" has to be written out rather than dropped.
    expect(control.getAttribute('aria-checked')).toBe('false');
    control.click();
    expect(onchange).toHaveBeenCalledWith(true);
  });
});

describe('Select', () => {
  const options = [
    { value: 'traditional', label: 'Traditional (Taiwan)' },
    { value: 'none', label: "Don't convert" },
  ];

  it('reports the chosen option of a select', () => {
    const onchange = vi.fn();
    const { target } = render(Select, { value: 'traditional', options, label: 'Chinese characters', onchange });
    const select = target.querySelector<HTMLSelectElement>('select[aria-label="Chinese characters"]')!;
    select.value = 'none';
    select.dispatchEvent(new Event('change', { bubbles: true }));
    expect(onchange).toHaveBeenCalledWith('none');
  });

  it('shows the current value, not just the first option', () => {
    const { target } = render(Select, { value: 'none', options, label: 'Chinese characters', onchange: vi.fn() });
    expect(target.querySelector('select')!.value).toBe('none');
  });

  /** What a user does to the native element, which then shows the pick whether or not anyone accepts it. */
  const pick = async (select: HTMLSelectElement, value: string) => {
    select.value = value;
    select.dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
  };

  it('goes back to the current value when the parent does not accept the pick', async () => {
    const onchange = vi.fn();
    const { target } = render(Select, { value: 'traditional', options, label: 'Chinese characters', onchange });
    const select = target.querySelector<HTMLSelectElement>('select')!;
    await pick(select, 'none');
    expect(onchange).toHaveBeenCalledWith('none');
    expect(select.value).toBe('traditional');
  });

  it('keeps the pick while the parent is still deciding, then goes back if it was refused', async () => {
    let answer!: () => void;
    const onchange = vi.fn(() => new Promise<void>(resolve => { answer = () => resolve(); }));
    const { target } = render(Select, { value: 'traditional', options, label: 'Chinese characters', onchange });
    const select = target.querySelector<HTMLSelectElement>('select')!;
    await pick(select, 'none');
    expect(select.value).toBe('none');
    answer();
    await settle();
    expect(select.value).toBe('traditional');
  });

  it('keeps a pick the parent accepted, reading the value only after it has answered', async () => {
    // The parent accepts by changing what it holds. A getter does that without a reactive parent.
    let held = 'traditional';
    const { target } = render(Select, {
      get value() { return held; },
      options,
      label: 'Chinese characters',
      onchange: async (next: string) => { held = next; },
    });
    const select = target.querySelector<HTMLSelectElement>('select')!;
    await pick(select, 'none');
    expect(select.value).toBe('none');
  });
});

describe('Group', () => {
  const members = createRawSnippet(() => ({ render: () => '<div class="member">Shortcut</div>' }));

  it('labels the container that holds the rows', () => {
    const { target } = render(Group, { title: 'Recording', label: 'Recording settings', children: members });
    const container = target.querySelector('[aria-label="Recording settings"]')!;
    expect(container.parentElement?.matches('section.group')).toBe(true);
    expect(container.querySelector('.member')).not.toBeNull();
    // A name only reaches assistive technology on an element with a role.
    expect(container.getAttribute('role')).toBe('group');
  });

  it('does not announce a group it has no name for', () => {
    const { target } = render(Group, { title: 'Recording', children: members });
    expect(target.querySelector('[role="group"]')).toBeNull();
  });

  it('shows the error in place of the footer', () => {
    const footer = 'Starts and stops recording';
    const quiet = render(Group, { footer, children: members }).target;
    expect(quiet.textContent).toContain(footer);
    expect(quiet.querySelector('.group-error')).toBeNull();

    const failed = render(Group, { footer, error: 'Shortcut already in use', children: members }).target;
    const error = failed.querySelector('.group-error');
    expect(error?.getAttribute('role')).toBe('alert');
    expect(error?.textContent).toBe('Shortcut already in use');
    expect(failed.textContent).not.toContain(footer);
  });
});
