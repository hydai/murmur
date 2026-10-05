/** What the History pane works out about its entries before it shows them. */

export interface HistoryEntry {
  id: string;
  timestamp_ms: number;
  raw_text?: string;
  final_text: string;
  command_name?: string;
  processing_time_ms: number;
}

export interface DayGroup {
  label: string;
  entries: HistoryEntry[];
}

const DAY_MS = 86_400_000;

/**
 * The calendar day `date` falls on in local time, counted in whole days. It is built from the
 * year, month and day rather than from the moment, so a day that lasts 23 or 25 hours
 * (daylight saving time) does not shift the days after it.
 */
function calendarDay(date: Date): number {
  return Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) / DAY_MS;
}

function dayLabel(daysAgo: number, date: Date, locale?: string): string {
  if (daysAgo === 0) return 'Today';
  if (daysAgo === 1) return 'Yesterday';
  // From two to six days ago the weekday is enough, and from seven it would repeat today's.
  if (daysAgo < 7) return date.toLocaleDateString(locale, { weekday: 'long' });
  return date.toLocaleDateString(locale, { month: 'short', day: 'numeric', year: 'numeric' });
}

/**
 * Groups entries by the local calendar day they were made on, as of `now`. The groups come
 * in the order their first entry does, and each keeps its entries in the order they came, so
 * a history that is newest first stays that way. A day never gets two groups.
 */
export function groupByDay(entries: HistoryEntry[], now: Date, locale?: string): DayGroup[] {
  const today = calendarDay(now);
  const groups = new Map<number, DayGroup>();
  for (const entry of entries) {
    const date = new Date(entry.timestamp_ms);
    // A moment ahead of the clock, which a clock set back leaves behind, is today.
    const daysAgo = Math.max(0, today - calendarDay(date));
    let group = groups.get(daysAgo);
    if (!group) {
      group = { label: dayLabel(daysAgo, date, locale), entries: [] };
      groups.set(daysAgo, group);
    }
    group.entries.push(entry);
  }
  return [...groups.values()];
}

/** How long the processing took, in seconds with one decimal: 1400 is "1.4 s". */
export function formatProcessingTime(ms: number): string {
  return `${(ms / 1000).toFixed(1)} s`;
}

/**
 * Whether the entry has an original worth opening: a transcription that says something other
 * than the final text does. Spaces around either text are not a difference.
 */
export function showsOriginal(entry: HistoryEntry): boolean {
  const original = entry.raw_text?.trim();
  return Boolean(original) && original !== entry.final_text.trim();
}
