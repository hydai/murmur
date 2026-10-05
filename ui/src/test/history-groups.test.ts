import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  formatProcessingTime,
  groupByDay,
  showsOriginal,
  type HistoryEntry,
} from '../components/history/historyGroups';
import { unmountAll } from './helpers';

afterEach(async () => {
  await unmountAll();
  vi.useRealTimers();
});

// 2026-10-05 is a Monday.
const now = new Date(2026, 9, 5, 10, 0);
const at = (month: number, day: number, hour: number): HistoryEntry => ({
  id: `${month}-${day}-${hour}`, final_text: '', processing_time_ms: 0,
  timestamp_ms: new Date(2026, month, day, hour).getTime(),
});

describe('groupByDay', () => {
  it('labels calendar days, not 24-hour windows', () => {
    // 10/4 23:00 is less than 24 hours before now, but it is the day before.
    const groups = groupByDay([at(9, 5, 1), at(9, 4, 23), at(9, 3, 9), at(9, 1, 9), at(8, 20, 9)], now, 'en-US');
    expect(groups.map(g => g.label)).toEqual(['Today', 'Yesterday', 'Saturday', 'Thursday', 'Sep 20, 2026']);
  });

  it('keeps entries of the same day together in their original order', () => {
    const groups = groupByDay([at(9, 5, 9), at(9, 5, 8)], now, 'en-US');
    expect(groups).toHaveLength(1);
    expect(groups[0].entries.map(e => e.id)).toEqual(['9-5-9', '9-5-8']);
  });

  it('names the weekday from two to six days ago and the date from seven', () => {
    const groups = groupByDay([at(9, 3, 12), at(8, 29, 12), at(8, 28, 12)], now, 'en-US');
    // Seven days ago is a Monday too; "Monday" would be taken for today.
    expect(groups.map(g => g.label)).toEqual(['Saturday', 'Tuesday', 'Sep 28, 2026']);
  });

  it('changes the day at local midnight', () => {
    const lastMoment = new Date(2026, 9, 4, 23, 59, 59, 999).getTime();
    const firstMoment = new Date(2026, 9, 5, 0, 0, 0, 0).getTime();
    const groups = groupByDay([
      { id: 'first', final_text: '', processing_time_ms: 0, timestamp_ms: firstMoment },
      { id: 'last', final_text: '', processing_time_ms: 0, timestamp_ms: lastMoment },
    ], now, 'en-US');
    expect(groups.map(g => [g.label, g.entries.map(e => e.id)])).toEqual([
      ['Today', ['first']],
      ['Yesterday', ['last']],
    ]);
  });

  it('counts days on the calendar when a day is not 24 hours long', () => {
    // 8 March 2026 is when US clocks go forward, so that day has 23 hours there; elsewhere it has 24.
    // Dividing the time between two moments by 24 hours would call Saturday "Yesterday" from just after midnight.
    const justAfterMidnight = new Date(2026, 2, 9, 0, 30);
    const groups = groupByDay([
      { id: 'sunday', final_text: '', processing_time_ms: 0, timestamp_ms: new Date(2026, 2, 8, 12).getTime() },
      { id: 'saturday', final_text: '', processing_time_ms: 0, timestamp_ms: new Date(2026, 2, 7, 12).getTime() },
    ], justAfterMidnight, 'en-US');
    expect(groups.map(g => g.label)).toEqual(['Yesterday', 'Saturday']);
  });

  it('puts a day that comes back under its first heading', () => {
    // History is newest first, so this is not expected, but a day must never get two headings.
    const groups = groupByDay([at(9, 5, 9), at(9, 4, 9), at(9, 5, 8)], now, 'en-US');
    expect(groups.map(g => [g.label, g.entries.map(e => e.id)])).toEqual([
      ['Today', ['9-5-9', '9-5-8']],
      ['Yesterday', ['9-4-9']],
    ]);
  });

  it('calls an entry from the future today', () => {
    // A clock that was set back would otherwise give "-1 days".
    const groups = groupByDay([at(9, 7, 9), at(9, 6, 9), at(9, 5, 9)], now, 'en-US');
    expect(groups.map(g => [g.label, g.entries.length])).toEqual([['Today', 3]]);
  });

  it('gives no groups for no entries', () => {
    expect(groupByDay([], now, 'en-US')).toEqual([]);
  });

  it('uses the language it is given for the weekday and the date', () => {
    const [weekday, date] = groupByDay([at(9, 3, 9), at(8, 20, 9)], now, 'de-DE').map(g => g.label);
    expect(weekday).toBe('Samstag');
    // The abbreviation of September differs between versions of the locale data, so ask the same function.
    expect(date).toBe(new Date(2026, 8, 20).toLocaleDateString('de-DE', { month: 'short', day: 'numeric', year: 'numeric' }));
    expect(date).not.toBe('Sep 20, 2026');
  });
});

describe('formatProcessingTime', () => {
  it('formats processing time in seconds with one decimal', () => {
    expect(formatProcessingTime(1400)).toBe('1.4 s');
    expect(formatProcessingTime(0)).toBe('0.0 s');
    expect(formatProcessingTime(12345)).toBe('12.3 s');
  });
});

describe('showsOriginal', () => {
  const entry = (final_text: string, raw_text?: string): HistoryEntry =>
    ({ id: '1', final_text, raw_text, timestamp_ms: 0, processing_time_ms: 0 });

  it('offers the original only when it differs from the final text', () => {
    expect(showsOriginal({ id: '1', final_text: 'a', raw_text: 'b', timestamp_ms: 0, processing_time_ms: 0 })).toBe(true);
    expect(showsOriginal({ id: '1', final_text: 'a', raw_text: 'a', timestamp_ms: 0, processing_time_ms: 0 })).toBe(false);
    expect(showsOriginal({ id: '1', final_text: 'a', raw_text: '  ', timestamp_ms: 0, processing_time_ms: 0 })).toBe(false);
  });

  it('offers nothing when there is no original', () => {
    expect(showsOriginal(entry('a'))).toBe(false);
    expect(showsOriginal(entry('a', ''))).toBe(false);
  });

  it('does not count a difference in the spaces around the text', () => {
    expect(showsOriginal(entry('hello world', ' hello world\n'))).toBe(false);
    expect(showsOriginal(entry(' hello world ', 'hello world'))).toBe(false);
    expect(showsOriginal(entry('hello world', 'hello  world'))).toBe(true);
  });
});
