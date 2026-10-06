import assert from 'node:assert/strict';
import test from 'node:test';

import { formatDiagnosticLogsForClipboard, formatLogTimestamp } from './diagnostics.ts';

// 2023-11-14 22:13:20 UTC. Tests that print it pin the time zone, so the answer
// does not depend on the machine they run on.
const AT = 1_700_000_000_000;
const ENGLISH = '11/14/2023, 10:13:20 PM';

// Newer ICU puts a narrow no-break space before AM and PM; the words are what matter.
const plain = (text) => text.replace(/\s/g, ' ');

test('formats the time of a log entry in English', () => {
  assert.equal(plain(formatLogTimestamp(AT, 'UTC')), ENGLISH);
  // The zone follows the system when none is given, the language never does.
  assert.match(plain(formatLogTimestamp(AT)), /^\d{1,2}\/\d{1,2}\/\d{4}, \d{1,2}:\d{2}:\d{2} [AP]M$/);
});

test('formats the time in English even when the system is set to another language', () => {
  const toLocaleString = Date.prototype.toLocaleString;
  // A system set to Traditional Chinese: a call that names no locale answers in it.
  Date.prototype.toLocaleString = function (locales, options) {
    return toLocaleString.call(this, locales ?? 'zh-TW', options);
  };
  try {
    // The stand-in is in force, so a call that forgot to name the locale would fail below.
    assert.notEqual(plain(new Date(AT).toLocaleString(undefined, { timeZone: 'UTC' })), ENGLISH);
    assert.equal(plain(formatLogTimestamp(AT, 'UTC')), ENGLISH);
    assert.match(plain(formatLogTimestamp(AT)), /^\d{1,2}\/\d{1,2}\/\d{4}, \d{1,2}:\d{2}:\d{2} [AP]M$/);
  } finally {
    Date.prototype.toLocaleString = toLocaleString;
  }
});

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
