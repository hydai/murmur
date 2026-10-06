<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import { useLifecycle } from '../../lib/lifecycle';
  import { getVersion } from '@tauri-apps/api/app';
  import { check } from '@tauri-apps/plugin-updater';
  import { relaunch } from '@tauri-apps/plugin-process';
  import { ChevronRight, ExternalLink, LoaderCircle } from 'lucide-svelte';
  import glyph from '../../assets/murmur-glyph.png';
  import Group from '../ui/Group.svelte';
  import Pane from '../ui/Pane.svelte';
  import Row from '../ui/Row.svelte';
  import DiagnosticsPanel from './DiagnosticsPanel.svelte';

  /** The step an error came from: asking for an update, or downloading and installing the one that was found. */
  type UpdatePhase = 'check' | 'install';

  type UpdateState =
    | { kind: 'idle' }
    | { kind: 'checking' }
    | { kind: 'up-to-date' }
    | { kind: 'available'; version: string; body: string | null }
    | { kind: 'downloading'; progress: number; total: number }
    | { kind: 'ready' }
    | { kind: 'error'; message: string; phase: UpdatePhase };

  /** What the error row says went wrong, by the step that failed. */
  const ERROR_LABELS: Record<UpdatePhase, string> = {
    check: "Couldn't check for updates",
    install: "Couldn't install the update",
  };

  const REPOSITORY = 'https://github.com/hydai/murmur';
  const LINKS = [
    { label: 'Source Code on GitHub', url: REPOSITORY },
    { label: 'Release Notes', url: `${REPOSITORY}/releases` },
    { label: 'Report an Issue', url: `${REPOSITORY}/issues` },
  ];

  let {
    pendingCheck = false,
    onCheckConsumed = () => {},
  }: {
    pendingCheck?: boolean;
    onCheckConsumed?: () => void;
  } = $props();

  let appVersion = $state('');
  let updateState: UpdateState = $state({ kind: 'idle' });

  // The Diagnostics Log takes the place of the whole pane, so there is one
  // heading on screen and the toolbar's back arrow leads here. This component
  // stays where it is, so what it knows about updates is kept meanwhile.
  let showingDiagnostics = $state(false);
  // The Troubleshooting group, for putting the focus back on its row.
  let troubleshooting = $state<HTMLElement>();

  // Holds the update object so we can call download/install on it
  let pendingUpdate: Awaited<ReturnType<typeof check>> = $state(null);

  const lifecycle = useLifecycle();

  function releasePendingUpdate() {
    const update = pendingUpdate;
    pendingUpdate = null;
    if (update) void update.close().catch((error) => console.warn('Failed to release update:', error));
  }

  lifecycle.onCleanup(releasePendingUpdate);

  onMount(() => {
    getVersion().then((version) => {
      if (!lifecycle.disposed) appVersion = version;
    }).catch((error) => console.warn('Failed to get app version:', error));

    void lifecycle.listen('update-available', () => {
      // Event metadata alone cannot download an update: check creates its resource.
      void checkForUpdates();
    }).catch((error) => console.warn('Failed to listen for updates:', error));
  });

  $effect(() => {
    if (pendingCheck) {
      onCheckConsumed();
      untrack(() => {
        // The check and its answer are shown on About itself, so an open log gives way to them.
        if (showingDiagnostics) void closeDiagnostics();
        void checkForUpdates();
      });
    }
  });

  async function checkForUpdates() {
    if (lifecycle.disposed || ['checking', 'downloading', 'ready'].includes(updateState.kind)) return;
    releasePendingUpdate();
    updateState = { kind: 'checking' };
    try {
      const update = await check();
      if (lifecycle.disposed) {
        if (update) await update.close();
        return;
      }
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
      updateState = { kind: 'error', message: String(e), phase: 'check' };
    }
  }

  async function downloadAndInstall() {
    if (!pendingUpdate || updateState.kind === 'downloading') return;
    let totalBytes = 0;
    let downloadedBytes = 0;
    updateState = { kind: 'downloading', progress: 0, total: 0 };

    try {
      await pendingUpdate.downloadAndInstall((event) => {
        if (lifecycle.disposed) return;
        if (event.event === 'Started' && event.data.contentLength) {
          totalBytes = event.data.contentLength;
        } else if (event.event === 'Progress') {
          downloadedBytes += event.data.chunkLength;
          updateState = { kind: 'downloading', progress: downloadedBytes, total: totalBytes };
        }
      });
      if (!lifecycle.disposed) updateState = { kind: 'ready' };
    } catch (e) {
      updateState = { kind: 'error', message: String(e), phase: 'install' };
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

  /** How much of the download has come, as a share of a total that may not be known yet. */
  function downloadShare(progress: number, total: number): number {
    return total > 0 ? Math.min(100, (progress / total) * 100) : 0;
  }

  function openLink(url: string) {
    window.open(url, '_blank');
  }

  /**
   * Back from the log. The pane was rebuilt, and the row that opened the log went
   * with the old one, which leaves the focus on the page. Its new button takes the
   * focus, so the keyboard carries on from where it left.
   */
  async function closeDiagnostics() {
    showingDiagnostics = false;
    await tick();
    troubleshooting?.querySelector<HTMLElement>('.row-main')?.focus();
  }
</script>

{#if showingDiagnostics}
  <DiagnosticsPanel onback={closeDiagnostics} />
{:else}
  <Pane title="About">
    <div class="app">
      <div class="glyph"><img src={glyph} alt="" width="64" height="64" /></div>
      <p class="app-name">Murmur</p>
      <p class="app-version">{appVersion ? `Version ${appVersion}` : 'Version'}</p>
    </div>

    <Group title="Software Update">
      {#if updateState.kind === 'idle'}
        <Row label="Software Update">
          {#snippet trailing()}
            <button type="button" class="btn btn-small" onclick={checkForUpdates}>Check for Updates</button>
          {/snippet}
        </Row>
      {:else if updateState.kind === 'checking'}
        <Row label="Checking for updates…">
          {#snippet accessory()}
            <span class="spinner"><LoaderCircle size={16} aria-hidden="true" /></span>
          {/snippet}
        </Row>
      {:else if updateState.kind === 'up-to-date'}
        <Row label="Murmur is up to date">
          {#snippet trailing()}
            <button type="button" class="btn btn-small" onclick={checkForUpdates}>Check Again</button>
          {/snippet}
        </Row>
      {:else if updateState.kind === 'available'}
        <Row label="Version {updateState.version} is available">
          {#snippet trailing()}
            <button type="button" class="btn btn-small btn-primary" onclick={downloadAndInstall}>
              Download and Install
            </button>
          {/snippet}
        </Row>
        {#if updateState.body}
          <!--
            A box that scrolls has to take the focus itself, or the keyboard could not reach
            what is below its edge. A named region is how that is announced; the rule below
            only knows widgets, and a box of text is not one.
          -->
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div class="release-notes" role="region" aria-label="Release notes" tabindex="0">{updateState.body}</div>
        {/if}
      {:else if updateState.kind === 'downloading'}
        {@const download = updateState}
        {@const share = downloadShare(download.progress, download.total)}
        <Row
          label="Downloading update…"
          detail={download.total > 0 ? `${formatBytes(download.progress)} of ${formatBytes(download.total)}` : undefined}
        >
          <!-- Without a total there is no share to claim, so the bar says nothing of one. -->
          <div
            class="progress"
            role="progressbar"
            aria-label="Update download"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={download.total > 0 ? Math.round(share) : undefined}
          >
            <div class="progress-fill" style:width="{share}%"></div>
          </div>
        </Row>
      {:else if updateState.kind === 'ready'}
        <Row label="Restart Murmur to finish updating">
          {#snippet trailing()}
            <button type="button" class="btn btn-small btn-primary" onclick={restartApp}>Restart Now</button>
          {/snippet}
        </Row>
      {:else if updateState.kind === 'error'}
        <Row label={ERROR_LABELS[updateState.phase]} detail={updateState.message}>
          {#snippet trailing()}
            <button type="button" class="btn btn-small" onclick={checkForUpdates}>Try Again</button>
          {/snippet}
        </Row>
      {/if}
    </Group>

    <Group title="Links">
      {#each LINKS as link (link.url)}
        <Row label={link.label} onclick={() => openLink(link.url)}>
          <!-- Not a control, so inside the button: the whole width of the row opens the link. -->
          {#snippet accessory()}
            <span class="accessory-icon"><ExternalLink size={14} aria-hidden="true" /></span>
          {/snippet}
        </Row>
      {/each}
    </Group>

    <!-- Wrapped so that its row can be found when the pane is rebuilt on coming back from the log. -->
    <div bind:this={troubleshooting}>
      <Group title="Troubleshooting">
        <Row label="Diagnostics Log" detail="Recent warnings and errors" onclick={() => (showingDiagnostics = true)}>
          {#snippet accessory()}
            <span class="accessory-icon"><ChevronRight size={16} aria-hidden="true" /></span>
          {/snippet}
        </Row>
      </Group>
    </div>
  </Pane>
{/if}

<style>
  .app {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding-top: 4px;
  }

  /*
   * The picture has a white ground of its own, so it sits on a white tile in
   * dark mode too, where a bare square would show its corners.
   */
  .glyph {
    width: 64px;
    height: 64px;
    overflow: hidden;
    border-radius: 15px;
    background: #fff;
    box-shadow: 0 0 0 .5px var(--separator), 0 1px 3px rgba(0, 0, 0, .18);
  }

  .glyph img {
    display: block;
    width: 100%;
    height: 100%;
  }

  .app-name {
    margin-top: 10px;
    font-size: 16px;
    font-weight: 600;
    line-height: 1.3;
  }

  .app-version {
    margin-top: 3px;
    font-size: 12px;
    line-height: 1.3;
    color: var(--text-secondary);
  }

  .spinner {
    display: inline-flex;
    color: var(--text-secondary);
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .release-notes {
    max-height: 120px;
    padding: 10px 12px;
    overflow-y: auto;
    font-size: 12px;
    line-height: 1.4;
    color: var(--text-secondary);
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }

  /* An outset ring would be clipped by the group's rounded edge. */
  .release-notes:focus-visible {
    outline-offset: -2px;
  }

  .progress {
    height: 4px;
    overflow: hidden;
    border-radius: 2px;
    background: var(--fill-selected);
  }

  .progress-fill {
    height: 100%;
    border-radius: 2px;
    background: var(--accent);
    transition: width 0.3s ease;
  }

  .accessory-icon {
    display: inline-flex;
    color: var(--text-secondary);
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation: none;
    }

    .progress-fill {
      transition: none;
    }
  }
</style>
