import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync, readdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * The ACL is enforced at runtime: a grant a window is missing fails only once
 * a user reaches that screen. This derives both sides from source — what each
 * window's component tree uses, and what its capability allows — so the two
 * cannot drift apart unnoticed.
 *
 * It reads the ACL *sources*, not `gen/schemas`. Those snapshots are written by
 * `tauri-build`, and CI runs `npm test` before any cargo step, so a source edit
 * without a rebuild would leave this checking last build's answer.
 */

const here = resolve(fileURLToPath(import.meta.url), '..');
const repo = resolve(here, '../../..');
const aclDir = join(repo, 'crates/lt-tauri');

// --- ACL sources -----------------------------------------------------------

/**
 * Pull `identifier` and `commands.allow` out of the `[[permission]]` blocks.
 * The file is ours and its shape is fixed, so this stays a targeted read rather
 * than a TOML implementation; `permissionSets` asserts it found something.
 */
function permissionSets() {
  const toml = readFileSync(join(aclDir, 'permissions/default.toml'), 'utf8');
  const sets = new Map();
  for (const block of toml.split('[[permission]]').slice(1)) {
    const identifier = block.match(/identifier\s*=\s*"([^"]+)"/)?.[1];
    assert.ok(identifier, 'a [[permission]] block has no identifier');
    const list = block.match(/commands\.allow\s*=\s*\[([\s\S]*?)\]/)?.[1] ?? '';
    sets.set(identifier, [...list.matchAll(/"([a-z_]+)"/g)].map((m) => m[1]));
  }
  assert.ok(sets.size >= 3, `parsed ${sets.size} permission sets, expected one per window`);
  return sets;
}

function capabilities() {
  const dir = join(aclDir, 'capabilities');
  return readdirSync(dir)
    .filter((name) => name.endsWith('.json'))
    .map((name) => JSON.parse(readFileSync(join(dir, name), 'utf8')));
}

// --- what each window uses -------------------------------------------------

/** App.svelte routes ?view= to exactly one component tree per window. */
const ROUTER = 'ui/src/App.svelte';
const WINDOW_SOURCES = {
  // The router loads in all three, so whatever it uses every window must be
  // granted.
  main: [ROUTER, 'ui/src/components/overlay'],
  settings: [ROUTER, 'ui/src/components/settings'],
  history: [ROUTER, 'ui/src/components/history'],
};

/**
 * Capability permissions that are not app commands, and the source pattern that
 * shows a window needs one. Without this, dropping a plugin grant leaves the
 * command coverage green while the call fails at runtime.
 */
const PLUGIN_GRANTS = [
  { permission: 'clipboard-manager:allow-write-text', pattern: /plugin-clipboard-manager/ },
  { permission: 'updater:default', pattern: /plugin-updater/ },
  { permission: 'process:default', pattern: /plugin-process/ },
  { permission: 'core:window:allow-start-dragging', pattern: /\bstartDragging\s*\(/ },
  { permission: 'core:window:allow-close', pattern: /getCurrentWindow\(\)\.close\s*\(/ },
];

/** Granted to every window as the baseline for events and the webview itself. */
const BASELINE = new Set(['core:default']);

function sourceFiles(relative) {
  const full = join(repo, relative);
  if (relative.endsWith('.svelte')) return [full];
  return readdirSync(full, { withFileTypes: true, recursive: true })
    .filter((entry) => entry.isFile() && /\.(svelte|ts)$/.test(entry.name))
    .map((entry) => join(entry.parentPath, entry.name));
}

function sourceOf(window) {
  return WINDOW_SOURCES[window]
    .flatMap(sourceFiles)
    .map((file) => readFileSync(file, 'utf8'))
    .join('\n');
}

function commandsInvokedBy(window) {
  const source = sourceOf(window);
  return new Set(
    [...source.matchAll(/\binvoke\s*(?:<[\s\S]*?>)?\s*\(\s*'([a-z_]+)'/g)].map((m) => m[1]),
  );
}

function pluginGrantsNeededBy(window) {
  const source = sourceOf(window);
  return new Set(PLUGIN_GRANTS.filter(({ pattern }) => pattern.test(source)).map((g) => g.permission));
}

// --- what each window is granted -------------------------------------------

function grantsTo(window) {
  const sets = permissionSets();
  const commands = new Set();
  const plugins = new Set();
  for (const capability of capabilities()) {
    if (!capability.windows?.includes(window)) continue;
    for (const permission of capability.permissions ?? []) {
      if (BASELINE.has(permission)) continue;
      const allowed = sets.get(permission);
      if (allowed) for (const command of allowed) commands.add(command);
      else plugins.add(permission);
    }
  }
  return { commands, plugins };
}

// --- checks ----------------------------------------------------------------

for (const window of Object.keys(WINDOW_SOURCES)) {
  test(`the ${window} window is granted every command its UI invokes`, () => {
    const invoked = commandsInvokedBy(window);
    assert.ok(invoked.size > 0, `found no invoke() calls for ${window}`);
    const { commands } = grantsTo(window);
    const missing = [...invoked].filter((command) => !commands.has(command)).sort();
    assert.deepEqual(missing, [], `${window} invokes commands it is not allowed`);
  });

  test(`the ${window} window is granted no command its UI never invokes`, () => {
    const invoked = commandsInvokedBy(window);
    const extra = [...grantsTo(window).commands].filter((c) => !invoked.has(c)).sort();
    assert.deepEqual(extra, [], `${window} is allowed commands it never calls`);
  });

  test(`the ${window} window's plugin grants match what it uses`, () => {
    const needed = pluginGrantsNeededBy(window);
    const { plugins } = grantsTo(window);
    assert.deepEqual(
      [...needed].filter((p) => !plugins.has(p)).sort(),
      [],
      `${window} uses plugin APIs it is not granted`,
    );
    assert.deepEqual(
      [...plugins].filter((p) => !needed.has(p)).sort(),
      [],
      `${window} is granted plugin permissions it never uses`,
    );
  });
}

test('the overlay stays away from configuration and history', () => {
  const { commands } = grantsTo('main');
  for (const forbidden of ['get_config', 'save_api_key', 'get_dictionary', 'get_history', 'clear_history']) {
    assert.ok(!commands.has(forbidden), `overlay must not reach ${forbidden}`);
  }
});

test('the committed generated ACL matches its sources', () => {
  // A stale snapshot would mean the app ships grants nobody reviewed.
  const generated = JSON.parse(
    readFileSync(join(aclDir, 'gen/schemas/acl-manifests.json'), 'utf8'),
  )['__app-acl__'].permissions;
  for (const [identifier, allowed] of permissionSets()) {
    assert.deepEqual(
      generated[identifier]?.commands?.allow ?? null,
      allowed,
      `gen/schemas is stale for ${identifier}; run cargo build -p lt-tauri`,
    );
  }
  assert.deepEqual(
    Object.keys(generated).sort(),
    [...permissionSets().keys()].sort(),
    'gen/schemas lists different permission sets than the source',
  );
});
