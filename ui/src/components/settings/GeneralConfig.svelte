<script lang="ts">
  import { onMount } from 'svelte';
  import { useLifecycle } from '../../lib/lifecycle';
  import { createStatus } from '../../lib/status.svelte';
  import { safeInvoke as invoke } from '../../lib/tauri';
  import Group from '../ui/Group.svelte';
  import Pane from '../ui/Pane.svelte';
  import Row from '../ui/Row.svelte';
  import Select from '../ui/Select.svelte';
  import ShortcutField from '../ui/ShortcutField.svelte';
  import Switch from '../ui/Switch.svelte';

  const lifecycle = useLifecycle();
  const status = createStatus(lifecycle);

  const OUTPUT_MODES = [
    { id: 'clipboard', label: 'Copy to clipboard', detail: 'Paste it yourself with ⌘V' },
    { id: 'keyboard', label: 'Type it out', detail: 'Murmur types the text at your cursor' },
    { id: 'both', label: 'Type it out and copy', detail: 'Also keeps a copy on the clipboard' },
  ];

  const CHINESE_CONVERSIONS = [
    { value: 'traditional', label: 'Traditional (Taiwan)' },
    { value: 'none', label: "Don't convert" },
  ];

  // A fresh install's settings, shown until the stored ones arrive. Each moves
  // only after the backend has taken the change, so the page never claims a
  // setting that was not saved.
  let hotkey = $state('Ctrl+`');
  let outputMode = $state('clipboard');
  let chineseConversion = $state('traditional');
  let saveHistory = $state(true);

  // Why the last key press was not a shortcut. A shortcut the backend refuses is a toast instead.
  let shortcutError = $state('');

  onMount(loadConfig);

  async function loadConfig(): Promise<void> {
    await status.run('Failed to load settings', async () => {
      const config = await invoke<{
        hotkey: string;
        output_mode: string;
        save_history: boolean;
        chinese_conversion?: string;
      }>('get_config');
      hotkey = config.hotkey;
      outputMode = config.output_mode.toLowerCase();
      saveHistory = config.save_history !== false;
      chineseConversion = (config.chinese_conversion || 'traditional').toLowerCase();
    });
  }

  async function saveHotkey(next: string): Promise<void> {
    // Another shortcut was recorded, so the last explanation no longer applies.
    shortcutError = '';
    await status.run('Failed to set shortcut', async () => {
      await invoke('set_hotkey', { hotkey: next });
      hotkey = next;
    });
  }

  async function selectOutputMode(mode: string): Promise<void> {
    await status.run('Failed to set output mode', async () => {
      await invoke('set_output_mode', { mode });
      outputMode = mode;
    });
  }

  // The Select waits for this before it lines the element up with the page's
  // value, so a change the backend refuses goes back to what was saved.
  async function selectChineseConversion(mode: string): Promise<void> {
    await status.run('Failed to set Chinese conversion', async () => {
      await invoke('set_chinese_conversion', { mode });
      chineseConversion = mode;
    });
  }

  async function setSaveHistory(enabled: boolean): Promise<void> {
    await status.run('Failed to update history setting', async () => {
      await invoke('set_save_history', { enabled });
      saveHistory = enabled;
    });
  }
</script>

<Pane title="General" {status}>
  <Group title="Recording" error={shortcutError}>
    <Row label="Shortcut" detail="Starts and stops recording">
      {#snippet trailing()}
        <ShortcutField
          value={hotkey}
          label="Shortcut"
          onchange={saveHotkey}
          onerror={(message) => (shortcutError = message)}
        />
      {/snippet}
    </Row>
  </Group>

  <Group title="After Transcribing">
    {#each OUTPUT_MODES as mode (mode.id)}
      <Row
        label={mode.label}
        detail={mode.detail}
        current={outputMode === mode.id}
        onclick={() => selectOutputMode(mode.id)}
      />
    {/each}
  </Group>

  <Group title="Text">
    <Row label="Chinese characters" detail="Translations into Simplified Chinese are kept">
      {#snippet trailing()}
        <Select
          value={chineseConversion}
          options={CHINESE_CONVERSIONS}
          label="Chinese characters"
          onchange={selectChineseConversion}
        />
      {/snippet}
    </Row>
  </Group>

  <Group title="History">
    <Row label="Save transcription history" detail="Kept only on this Mac">
      {#snippet trailing()}
        <Switch checked={saveHistory} label="Save transcription history" onchange={setSaveHistory} />
      {/snippet}
    </Row>
  </Group>
</Pane>
