<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import { check } from '@tauri-apps/plugin-updater';
  import { relaunch } from '@tauri-apps/plugin-process';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { Settings, Shield, Cpu, ExternalLink, RefreshCw } from 'lucide-svelte';
  import PageHeader from './ui/PageHeader.svelte';
  import SectionHeader from './ui/SectionHeader.svelte';
  import StatusRow from './ui/StatusRow.svelte';

  type UpdateState =
    | { kind: 'idle' }
    | { kind: 'checking' }
    | { kind: 'up-to-date' }
    | { kind: 'available'; version: string; body: string | null }
    | { kind: 'downloading'; progress: number; total: number }
    | { kind: 'ready' }
    | { kind: 'error'; message: string };

  let {
    pendingCheck = false,
    onCheckConsumed = () => {},
  }: {
    pendingCheck?: boolean;
    onCheckConsumed?: () => void;
  } = $props();

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
  });

  $effect(() => {
    if (pendingCheck) {
      onCheckConsumed();
      checkForUpdates();
    }
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
          body: update.body ?? null,
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

<div class="page">
  <PageHeader title="About" description="Application information and updates" />

  <SectionHeader label="APPLICATION" />
  <div class="app-card">
    <div class="app-row">
      <Settings size={14} color="var(--text-muted)" />
      <span class="app-row-label">Murmur</span>
      <span class="spacer"></span>
      <span class="version-tag">v{appVersion}</span>
    </div>
    <div class="card-separator"></div>
    <div class="app-row">
      <Shield size={14} color="var(--text-muted)" />
      <span class="app-row-text">Privacy-first voice typing</span>
    </div>
    <div class="app-row">
      <Cpu size={14} color="var(--text-muted)" />
      <span class="app-row-text">On-device processing</span>
    </div>
  </div>

  <SectionHeader label="UPDATES" />
  {#if updateState.kind === 'idle'}
    <button class="primary-btn" onclick={checkForUpdates}>Check for Updates</button>

  {:else if updateState.kind === 'checking'}
    <div class="status-card">
      <RefreshCw size={14} color="var(--text-secondary)" class="spin" />
      <span class="status-text">Checking for updates...</span>
    </div>

  {:else if updateState.kind === 'up-to-date'}
    <StatusRow
      label="Up to date"
      status="green"
      statusText="Latest"
    />
    <button class="secondary-btn" onclick={checkForUpdates}>Check Again</button>

  {:else if updateState.kind === 'available'}
    <StatusRow
      label="Version {updateState.version} available"
      status="yellow"
      statusText="New"
    />
    {#if updateState.body}
      <div class="release-notes">{updateState.body}</div>
    {/if}
    <button class="primary-btn" onclick={downloadAndInstall}>Download & Install</button>

  {:else if updateState.kind === 'downloading'}
    <div class="download-section">
      <span class="download-label">Downloading update...</span>
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
    <StatusRow
      label="Update installed"
      status="green"
      statusText="Ready"
    />
    <button class="primary-btn" onclick={restartApp}>Restart Now</button>

  {:else if updateState.kind === 'error'}
    <StatusRow
      label="Update failed"
      value={updateState.message}
      status="red"
      statusText="Error"
    />
    <button class="secondary-btn" onclick={checkForUpdates}>Retry</button>
  {/if}

  <SectionHeader label="LINKS" />
  <div class="section-rows">
    <StatusRow
      label="GitHub"
      onclick={() => window.open('https://github.com/hydai/murmur', '_blank')}
    >
      {#snippet children()}
        <ExternalLink size={12} color="var(--text-muted)" />
      {/snippet}
    </StatusRow>
    <StatusRow
      label="Releases"
      onclick={() => window.open('https://github.com/hydai/murmur/releases', '_blank')}
    >
      {#snippet children()}
        <ExternalLink size={12} color="var(--text-muted)" />
      {/snippet}
    </StatusRow>
    <StatusRow
      label="Report Issue"
      onclick={() => window.open('https://github.com/hydai/murmur/issues', '_blank')}
    >
      {#snippet children()}
        <ExternalLink size={12} color="var(--text-muted)" />
      {/snippet}
    </StatusRow>
  </div>
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .app-card {
    display: flex;
    flex-direction: column;
    gap: 0;
    background: var(--bg-card);
    border-radius: 8px;
    padding: 12px 14px;
  }

  .app-row {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 30px;
  }

  .app-row-label {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
  }

  .app-row-text {
    font-size: 12px;
    color: var(--text-secondary);
  }

  .spacer {
    flex: 1;
  }

  .version-tag {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
  }

  .card-separator {
    height: 1px;
    background: var(--border);
    width: 100%;
    margin: 4px 0;
  }

  .status-card {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 38px;
    padding: 0 12px;
    background: var(--bg-card);
    border-radius: 8px;
  }

  .status-text {
    font-size: 13px;
    color: var(--text-secondary);
  }

  :global(.spin) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .release-notes {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 10px 12px;
    font-size: 12px;
    color: var(--text-secondary);
    max-height: 100px;
    overflow-y: auto;
    white-space: pre-wrap;
  }

  .download-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 14px;
    background: var(--bg-card);
    border-radius: 8px;
  }

  .download-label {
    font-size: 12px;
    color: var(--text-secondary);
  }

  .progress-bar {
    height: 4px;
    background: var(--border);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 2px;
    transition: width 0.3s ease;
  }

  .progress-text {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-muted);
  }

  .primary-btn {
    height: 34px;
    padding: 0 16px;
    background: var(--accent);
    color: var(--text-primary);
    border: none;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .primary-btn:hover {
    background: var(--accent-hover);
  }

  .secondary-btn {
    height: 34px;
    padding: 0 16px;
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-primary);
    border: none;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .secondary-btn:hover {
    background: rgba(255, 255, 255, 0.15);
  }

  .section-rows {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
</style>
