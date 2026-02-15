<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import { check } from '@tauri-apps/plugin-updater';
  import { relaunch } from '@tauri-apps/plugin-process';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';

  type UpdateState =
    | { kind: 'idle' }
    | { kind: 'checking' }
    | { kind: 'up-to-date' }
    | { kind: 'available'; version: string; body: string | null }
    | { kind: 'downloading'; progress: number; total: number }
    | { kind: 'ready' }
    | { kind: 'error'; message: string };

  let appVersion = $state('');
  let updateState: UpdateState = $state({ kind: 'idle' });

  // Holds the update object so we can call download/install on it
  let pendingUpdate: Awaited<ReturnType<typeof check>> = $state(null);

  const unlistens: UnlistenFn[] = [];

  onMount(async () => {
    appVersion = await getVersion();

    // Listen for background update-available event (from Rust startup check)
    unlistens.push(
      await listen<{ version: string; body?: string }>('update-available', (event) => {
        updateState = {
          kind: 'available',
          version: event.payload.version,
          body: event.payload.body ?? null,
        };
      })
    );

    // Listen for tray menu "Check for Updates" click
    unlistens.push(
      await listen('update-check-requested', () => {
        checkForUpdates();
      })
    );
  });

  onDestroy(() => {
    for (const unlisten of unlistens) {
      unlisten();
    }
  });

  async function checkForUpdates() {
    updateState = { kind: 'checking' };
    try {
      const update = await check();
      if (update) {
        pendingUpdate = update;
        updateState = {
          kind: 'available',
          version: update.version,
          body: update.body,
        };
      } else {
        updateState = { kind: 'up-to-date' };
      }
    } catch (e) {
      updateState = { kind: 'error', message: String(e) };
    }
  }

  async function downloadAndInstall() {
    if (!pendingUpdate) return;
    let totalBytes = 0;
    let downloadedBytes = 0;
    updateState = { kind: 'downloading', progress: 0, total: 0 };

    try {
      await pendingUpdate.downloadAndInstall((event) => {
        if (event.event === 'Started' && event.data.contentLength) {
          totalBytes = event.data.contentLength;
        } else if (event.event === 'Progress') {
          downloadedBytes += event.data.chunkLength;
          updateState = { kind: 'downloading', progress: downloadedBytes, total: totalBytes };
        } else if (event.event === 'Finished') {
          updateState = { kind: 'ready' };
        }
      });
      updateState = { kind: 'ready' };
    } catch (e) {
      updateState = { kind: 'error', message: String(e) };
    }
  }

  async function restartApp() {
    await relaunch();
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  }
</script>

<div class="about-section">
  <div class="app-info">
    <h2>Murmur</h2>
    <p class="version">Version {appVersion}</p>
    <p class="description">Privacy-first voice typing with on-device processing</p>
  </div>

  <div class="update-section">
    <h3>Updates</h3>

    {#if updateState.kind === 'idle'}
      <button class="update-btn" onclick={checkForUpdates}>Check for Updates</button>

    {:else if updateState.kind === 'checking'}
      <div class="status">
        <span class="spinner"></span>
        Checking for updates...
      </div>

    {:else if updateState.kind === 'up-to-date'}
      <div class="status success">You're on the latest version.</div>
      <button class="update-btn secondary" onclick={checkForUpdates}>Check Again</button>

    {:else if updateState.kind === 'available'}
      <div class="update-available">
        <div class="status info">Version {updateState.version} is available</div>
        {#if updateState.body}
          <div class="release-notes">{updateState.body}</div>
        {/if}
        <button class="update-btn" onclick={downloadAndInstall}>Download & Install</button>
      </div>

    {:else if updateState.kind === 'downloading'}
      <div class="download-progress">
        <div class="status">Downloading update...</div>
        <div class="progress-bar">
          <div
            class="progress-fill"
            style="width: {updateState.total > 0 ? (updateState.progress / updateState.total) * 100 : 0}%"
          ></div>
        </div>
        {#if updateState.total > 0}
          <span class="progress-text">
            {formatBytes(updateState.progress)} / {formatBytes(updateState.total)}
          </span>
        {/if}
      </div>

    {:else if updateState.kind === 'ready'}
      <div class="status success">Update installed. Restart to apply.</div>
      <button class="update-btn" onclick={restartApp}>Restart Now</button>

    {:else if updateState.kind === 'error'}
      <div class="status error">Update check failed: {updateState.message}</div>
      <button class="update-btn secondary" onclick={checkForUpdates}>Retry</button>
    {/if}
  </div>

  <div class="links">
    <a href="https://github.com/hydai/murmur" target="_blank" rel="noopener">GitHub</a>
    <span class="separator">·</span>
    <a href="https://github.com/hydai/murmur/releases" target="_blank" rel="noopener">Releases</a>
    <span class="separator">·</span>
    <a href="https://github.com/hydai/murmur/issues" target="_blank" rel="noopener">Report Issue</a>
  </div>
</div>

<style>
  .about-section {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .app-info h2 {
    margin: 0 0 4px;
    font-size: 20px;
    color: #fff;
  }

  .version {
    margin: 0 0 8px;
    font-size: 14px;
    color: rgba(255, 255, 255, 0.5);
    font-family: monospace;
  }

  .description {
    margin: 0;
    font-size: 14px;
    color: rgba(255, 255, 255, 0.7);
  }

  .update-section {
    background: rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    padding: 16px;
  }

  .update-section h3 {
    margin: 0 0 12px;
    font-size: 14px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.8);
  }

  .status {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.7);
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
  }

  .status.success {
    color: #22c55e;
  }

  .status.info {
    color: #3b82f6;
    font-weight: 500;
  }

  .status.error {
    color: #ef4444;
  }

  .spinner {
    width: 14px;
    height: 14px;
    border: 2px solid rgba(255, 255, 255, 0.2);
    border-top-color: #3b82f6;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .update-btn {
    background: #3b82f6;
    color: #fff;
    border: none;
    border-radius: 8px;
    padding: 8px 16px;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.2s;
  }

  .update-btn:hover {
    background: #2563eb;
  }

  .update-btn.secondary {
    background: rgba(255, 255, 255, 0.1);
  }

  .update-btn.secondary:hover {
    background: rgba(255, 255, 255, 0.15);
  }

  .release-notes {
    background: rgba(0, 0, 0, 0.3);
    border-radius: 8px;
    padding: 12px;
    margin-bottom: 12px;
    font-size: 13px;
    color: rgba(255, 255, 255, 0.6);
    max-height: 120px;
    overflow-y: auto;
    white-space: pre-wrap;
  }

  .progress-bar {
    height: 6px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 3px;
    overflow: hidden;
    margin-bottom: 8px;
  }

  .progress-fill {
    height: 100%;
    background: #3b82f6;
    border-radius: 3px;
    transition: width 0.3s ease;
  }

  .progress-text {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
    font-family: monospace;
  }

  .links {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.4);
  }

  .links a {
    color: #3b82f6;
    text-decoration: none;
  }

  .links a:hover {
    text-decoration: underline;
  }

  .separator {
    margin: 0 8px;
  }
</style>
