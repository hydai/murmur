import assert from 'node:assert/strict';
import test from 'node:test';

import { formatDiagnosticLogsForClipboard } from './diagnostics.ts';

test('formats diagnostic logs for support-friendly clipboard output', () => {
  const text = formatDiagnosticLogsForClipboard([
    {
      timestamp_ms: 1_700_000_000_000,
      level: 'warn',
      target: 'lt_stt::custom',
      message: 'Custom STT request failed',
    },
    {
      timestamp_ms: 1_700_000_001_000,
      level: 'error',
      target: 'lt_pipeline',
      message: 'Pipeline error',
    },
  ]);

  assert.match(text, /WARN/);
  assert.match(text, /lt_stt::custom/);
  assert.match(text, /Custom STT request failed/);
  assert.match(text, /ERROR/);
  assert.match(text, /lt_pipeline/);
});
