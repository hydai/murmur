<script lang="ts">
  import { onMount } from 'svelte';
  import type { Component, ComponentType, SvelteComponent } from 'svelte';
  import { BookOpen, Clock, Info, Mic, PenLine, SlidersHorizontal } from 'lucide-svelte';
  import type { IconProps } from 'lucide-svelte';
  import { useLifecycle } from '../../lib/lifecycle';
  import { PANES, initialRoute, parsePane, type PaneId } from './navigation';
  import LegacyPane from './LegacyPane.svelte';
  import GeneralConfig from './GeneralConfig.svelte';
  import ProviderConfig from './ProviderConfig.svelte';
  import DictionaryEditor from './DictionaryEditor.svelte';
  import LlmConfig from './LlmConfig.svelte';
  import PromptsEditor from './PromptsEditor.svelte';
  import DiagnosticsPanel from './DiagnosticsPanel.svelte';
  import AboutSection from './AboutSection.svelte';
  import HistoryPanel from '../history/HistoryPanel.svelte';

  // lucide-svelte icons are class components, which `Component` alone rejects.
  type IconComponent = Component<IconProps> | ComponentType<SvelteComponent<IconProps>>;

  const ICONS: Record<PaneId, IconComponent> = {
    general: SlidersHorizontal,
    transcription: Mic,
    ai: PenLine,
    dictionary: BookOpen,
    history: Clock,
    about: Info,
  };

  const GROUPS = ([1, 2] as const).map((id) => ({
    id,
    panes: PANES.filter((pane) => pane.group === id),
  }));

  const route = initialRoute(window.location.search);

  let activePane = $state<PaneId>(route.pane);
  let pendingUpdateCheck = $state(route.checkUpdate);

  const lifecycle = useLifecycle();

  onMount(() => {
    // Sent when the window is already open and something, such as the menu
    // bar, asks for another pane. A window opened fresh reads its URL instead.
    void lifecycle.listen('navigate', ({ payload }) => {
      const pane = parsePane(payload.pane);
      if (!pane) return;
      activePane = pane;
      if (payload.action === 'check-update') pendingUpdateCheck = true;
    }).catch((error) => console.warn('Failed to listen for navigation:', error));
  });
</script>

<div class="shell">
  <nav class="sidebar ui-v2" aria-label="Settings" data-tauri-drag-region="deep">
    {#each GROUPS as group (group.id)}
      <div class="nav-group">
        {#each group.panes as pane (pane.id)}
          {@const Icon = ICONS[pane.id]}
          <button
            type="button"
            class="nav-item"
            aria-current={activePane === pane.id ? 'page' : undefined}
            onclick={() => (activePane = pane.id)}
          >
            <span class="nav-icon"><Icon size={16} strokeWidth={1.75} aria-hidden="true" /></span>
            <span>{pane.label}</span>
          </button>
        {/each}
      </div>
    {/each}
  </nav>

  <main class="content">
    {#if activePane === 'general'}
      <GeneralConfig />
    {:else if activePane === 'transcription'}
      <ProviderConfig />
    {:else if activePane === 'ai'}
      <LegacyPane><LlmConfig /><PromptsEditor /></LegacyPane>
    {:else if activePane === 'dictionary'}
      <LegacyPane><DictionaryEditor /></LegacyPane>
    {:else if activePane === 'history'}
      <LegacyPane><HistoryPanel /></LegacyPane>
    {:else if activePane === 'about'}
      <LegacyPane>
        <AboutSection
          pendingCheck={pendingUpdateCheck}
          onCheckConsumed={() => (pendingUpdateCheck = false)}
        />
        <DiagnosticsPanel />
      </LegacyPane>
    {/if}
  </main>
</div>

<style>
  /* No background of its own: the window's material shows through the sidebar, and each pane paints its own. */
  .shell {
    display: flex;
    width: 100%;
    height: 100vh;
  }

  .sidebar {
    display: flex;
    flex: none;
    flex-direction: column;
    gap: 12px;
    width: 188px;
    /* The window controls float over the top 52px. */
    padding: 52px 10px 10px;
    border-right: 1px solid var(--separator);
    background: transparent;
  }

  .nav-group {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 28px;
    padding: 0 10px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-primary);
    font: inherit;
    font-size: 13px;
    font-weight: 400;
    text-align: left;
    transition: background-color 0.15s ease;
  }

  .nav-item[aria-current='page'] {
    background: var(--fill-selected);
    font-weight: 500;
  }

  .nav-icon {
    display: inline-flex;
    flex: none;
    color: var(--text-secondary);
  }

  .nav-item[aria-current='page'] .nav-icon {
    color: var(--accent);
  }

  /* The pane fills this column; it takes no padding or title of its own. */
  .content {
    flex: 1;
    min-width: 0;
  }

  @media (prefers-reduced-motion: reduce) {
    .nav-item {
      transition: none;
    }
  }
</style>
