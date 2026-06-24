export interface DiagnosticLogEntry {
  timestamp_ms: number;
  level: string;
  target: string;
  message: string;
}

export function formatLogTimestamp(timestampMs: number): string {
  return new Date(timestampMs).toLocaleString();
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
