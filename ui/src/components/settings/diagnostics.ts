export interface DiagnosticLogEntry {
  timestamp_ms: number;
  level: string;
  target: string;
  message: string;
}

// The interface is English whatever language the system is set to, so the times are too.
const LOCALE = 'en-US';

/** The time zone is the system's unless one is named; naming one is for tests. */
export function formatLogTimestamp(timestampMs: number, timeZone?: string): string {
  return new Date(timestampMs).toLocaleString(LOCALE, { timeZone });
}

export function formatDiagnosticLogsForClipboard(logs: DiagnosticLogEntry[]): string {
  if (logs.length === 0) {
    return 'No warnings or errors recorded.';
  }

  return logs
    .map((log) => {
      const timestamp = new Date(log.timestamp_ms).toISOString();
      const level = log.level.toUpperCase();
      return `[${timestamp}] ${level} ${log.target} - ${log.message}`;
    })
    .join('\n');
}
