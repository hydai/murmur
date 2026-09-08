<script lang="ts">
  import { onMount } from 'svelte';
  import { trapFocus } from '../../lib/focus';
  import { useLifecycle } from '../../lib/lifecycle';
  import { getVersion } from '@tauri-apps/api/app';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { Mic, Cpu, Keyboard, Type, BookOpen, FileCode, Bug, Info } from 'lucide-svelte';
  import ProviderConfig from './ProviderConfig.svelte';
  import DictionaryEditor from './DictionaryEditor.svelte';
  import LlmConfig from './LlmConfig.svelte';
  import HotkeyConfig from './HotkeyConfig.svelte';
  import OutputConfig from './OutputConfig.svelte';
  import PromptsEditor from './PromptsEditor.svelte';
  import DiagnosticsPanel from './DiagnosticsPanel.svelte';
  import AboutSection from './AboutSection.svelte';

  /// Closing the settings window is the settings window's own concern; the
  /// router that mounts it serves all three windows and should not carry a
  /// capability only this one needs.
  async function closeWindow() {
    try {
      await getCurrentWindow().close();
    } catch (error) {
      console.warn('Failed to close settings window:', error);
    }
  }

  let { visible, onClose = closeWindow }: { visible: boolean; onClose?: () => void } =
    $props();

  const searchParams =
    typeof window !== 'undefined' ? new URLSearchParams(window.location.search) : null;
  const standalone = searchParams?.get('view') === 'settings';
  const initialAction = searchParams?.get('action') ?? null;

  let activeTab = $state(initialAction === 'check-update' ? 'about' : 'providers');
  let pendingUpdateCheck = $state(initialAction === 'check-update');
  let appVersion = $state('');

  const lifecycle = useLifecycle();

  const navItems = [
    { id: 'providers', label: 'STT Providers', icon: Mic },
    { id: 'llm', label: 'LLM Processor', icon: Cpu },
    { id: 'hotkey', label: 'Hotkey', icon: Keyboard },
    { id: 'output', label: 'Output Mode', icon: Type },
    { id: 'dictionary', label: 'Dictionary', icon: BookOpen },
    { id: 'prompts', label: 'Prompts', icon: FileCode },
    { id: 'diagnostics', label: 'Diagnostics', icon: Bug },
    { id: 'about', label: 'About', icon: Info },
  ];

  onMount(() => {
    getVersion().then((version) => {
      if (!lifecycle.disposed) appVersion = version;
    }).catch((error) => console.warn('Failed to get app version:', error));

    void lifecycle.listen('open-about-and-check', () => {
      activeTab = 'about';
      pendingUpdateCheck = true;
    }).catch((error) => console.warn('Failed to listen for settings actions:', error));
  });

  function switchTab(tab: string) {
    activeTab = tab;
  }
</script>

{#if visible}
  {#if standalone}
    <div class="settings-window">
      <div class="title-bar">
        <div class="traffic-lights">
          <span class="dot red"></span>
          <span class="dot yellow"></span>
          <span class="dot green"></span>
        </div>
        <span class="title-text">Murmur Settings</span>
      </div>

      <div class="body">
        <nav class="sidebar">
          <div class="brand">
            <span class="brand-name">Murmur</span>
            <span class="brand-sub">VOICE TYPING</span>
          </div>
          <div class="nav-separator"></div>
          <div class="nav-items">
            {#each navItems as item (item.id)}
              <button
                class="nav-item"
                class:active={activeTab === item.id}
                onclick={() => switchTab(item.id)}
              >
                <item.icon size={16} />
                <span>{item.label}</span>
              </button>
            {/each}
          </div>
        </nav>

        <main class="content">
          {#if activeTab === 'providers'}
            <ProviderConfig />
          {:else if activeTab === 'llm'}
            <LlmConfig />
          {:else if activeTab === 'hotkey'}
            <HotkeyConfig />
          {:else if activeTab === 'output'}
            <OutputConfig />
          {:else if activeTab === 'dictionary'}
            <DictionaryEditor />
          {:else if activeTab === 'prompts'}
            <PromptsEditor />
          {:else if activeTab === 'diagnostics'}
            <DiagnosticsPanel />
          {:else if activeTab === 'about'}
            <AboutSection
              pendingCheck={pendingUpdateCheck}
              onCheckConsumed={() => (pendingUpdateCheck = false)}
            />
          {/if}
        </main>
      </div>

      <div class="footer">
        <span class="footer-left">Murmur v{appVersion}</span>
        <span class="footer-right">Local-only &middot; No cloud required</span>
      </div>
    </div>
  {:else}
    <!-- Inline overlay mode -->
    <div class="settings-overlay" onclick={onClose} onkeydown={(e) => e.key === 'Escape' && onClose()} role="presentation">
      <div class="settings-dialog" onclick={(e) => e.stopPropagation()} onkeydown={(e) => { if (e.key === 'Escape') onClose(); e.stopPropagation(); }} use:trapFocus role="dialog" tabindex="-1" aria-modal="true" aria-label="Settings">
        <div class="body">
          <nav class="sidebar">
            <div class="brand">
              <span class="brand-name">Murmur</span>
              <span class="brand-sub">VOICE TYPING</span>
            </div>
            <div class="nav-separator"></div>
            <div class="nav-items">
              {#each navItems as item (item.id)}
                <button
                  class="nav-item"
                  class:active={activeTab === item.id}
                  onclick={() => switchTab(item.id)}
                >
                  <item.icon size={16} />
                  <span>{item.label}</span>
                </button>
              {/each}
            </div>
          </nav>

          <main class="content">
            {#if activeTab === 'providers'}
              <ProviderConfig />
            {:else if activeTab === 'llm'}
              <LlmConfig />
            {:else if activeTab === 'hotkey'}
              <HotkeyConfig />
            {:else if activeTab === 'output'}
              <OutputConfig />
            {:else if activeTab === 'dictionary'}
              <DictionaryEditor />
            {:else if activeTab === 'prompts'}
              <PromptsEditor />
            {:else if activeTab === 'diagnostics'}
              <DiagnosticsPanel />
            {:else if activeTab === 'about'}
              <AboutSection
                pendingCheck={pendingUpdateCheck}
                onCheckConsumed={() => (pendingUpdateCheck = false)}
              />
            {/if}
          </main>
        </div>

        <div class="footer">
          <span class="footer-left">Murmur v{appVersion}</span>
          <span class="footer-right">Local-only &middot; No cloud required</span>
        </div>

        <button class="close-btn" onclick={onClose}>✕</button>
      </div>
    </div>
  {/if}
{/if}

<style>
  /* ── Window shell (standalone) ── */
  .settings-window {
    background: var(--bg-primary);
    width: 100%;
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-radius: 16px;
  }

  /* ── Title bar ── */
  .title-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 44px;
    padding: 0 16px;
    background: var(--bg-title-bar);
    flex-shrink: 0;
    -webkit-app-region: drag;
  }

  .traffic-lights {
    display: flex;
    gap: 8px;
    -webkit-app-region: no-drag;
  }

  .traffic-lights .dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
  }

  .dot.red { background: #FF5F57; }
  .dot.yellow { background: #FEBC2E; }
  .dot.green { background: #28C840; }

  .title-text {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-secondary);
  }

  /* ── Body (sidebar + content) ── */
  .body {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

  /* ── Sidebar ── */
  .sidebar {
    width: 180px;
    flex-shrink: 0;
    background: var(--bg-sidebar);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    padding: 24px 16px;
    overflow-y: auto;
  }

  .brand {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding-bottom: 20px;
  }

  .brand-name {
    font-size: 18px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .brand-sub {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 2px;
    color: var(--text-muted);
  }

  .nav-separator {
    height: 1px;
    background: var(--border);
    width: 100%;
  }

  .nav-items {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding-top: 12px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 36px;
    padding: 0 10px;
    border-radius: 8px;
    border: none;
    background: transparent;
    color: var(--text-muted);
    font-size: 13px;
    font-weight: normal;
    cursor: pointer;
    transition: all 0.15s ease;
    width: 100%;
    text-align: left;
    border-left: 2px solid transparent;
  }

  .nav-item:hover {
    color: var(--text-secondary);
    background: rgba(255, 255, 255, 0.03);
  }

  .nav-item.active {
    color: var(--accent);
    font-weight: 500;
    background: var(--bg-nav-active);
    border-left-color: var(--accent);
  }

  /* ── Content area ── */
  .content {
    flex: 1;
    overflow-y: auto;
    padding: 20px 28px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  /* ── Footer ── */
  .footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 36px;
    padding: 0 20px;
    background: var(--bg-title-bar);
    flex-shrink: 0;
    font-size: 11px;
    color: var(--text-muted);
  }

  .footer-left, .footer-right {
    color: var(--text-muted);
  }

  /* ── Overlay mode ── */
  .settings-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.8);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
  }

  .settings-dialog {
    background: var(--bg-primary);
    border-radius: 16px;
    width: 720px;
    height: 560px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    position: relative;
  }

  .close-btn {
    position: absolute;
    top: 12px;
    right: 12px;
    background: none;
    border: none;
    font-size: 18px;
    color: var(--text-muted);
    cursor: pointer;
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 6px;
    transition: all 0.15s ease;
    z-index: 1;
  }

  .close-btn:hover {
    background: var(--surface-raised);
    color: var(--text-primary);
  }
</style>
