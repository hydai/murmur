<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';

  interface Props {
    status: string;
  }

  let { status }: Props = $props();

  let isDragging = $state(false);
  let dragStartX = $state(0);
  let dragStartY = $state(0);

  async function handleMouseDown(e: MouseEvent) {
    isDragging = true;
    const appWindow = getCurrentWindow();

    const position = await appWindow.outerPosition();
    dragStartX = e.clientX - position.x;
    dragStartY = e.clientY - position.y;
  }

  async function handleMouseMove(e: MouseEvent) {
    if (!isDragging) return;

    const appWindow = getCurrentWindow();
    await appWindow.setPosition({
      x: e.screenX - dragStartX,
      y: e.screenY - dragStartY,
    });
  }

  function handleMouseUp() {
    isDragging = false;
  }
</script>

<svelte:window
  onmousemove={handleMouseMove}
  onmouseup={handleMouseUp}
/>

<div class="overlay-container">
  <div
    class="overlay-window"
    onmousedown={handleMouseDown}
    role="button"
    tabindex="0"
  >
    <div class="status-indicator">
      <div class="status-dot {status.toLowerCase()}"></div>
      <span class="status-text">{status}</span>
    </div>
    <div class="app-title">Localtype</div>
    <div class="hint-text">Press Cmd+Shift+Space to toggle</div>
  </div>
</div>

<style>
  .overlay-container {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 20px;
  }

  .overlay-window {
    background: rgba(30, 30, 30, 0.95);
    backdrop-filter: blur(10px);
    border-radius: 16px;
    padding: 24px 32px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    border: 1px solid rgba(255, 255, 255, 0.1);
    cursor: move;
    min-width: 400px;
    text-align: center;
  }

  .status-indicator {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    margin-bottom: 16px;
  }

  .status-dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    animation: pulse 2s ease-in-out infinite;
  }

  .status-dot.ready {
    background: #4ade80;
    box-shadow: 0 0 8px rgba(74, 222, 128, 0.6);
  }

  .status-dot.initializing {
    background: #facc15;
    box-shadow: 0 0 8px rgba(250, 204, 21, 0.6);
  }

  .status-dot.error {
    background: #ef4444;
    box-shadow: 0 0 8px rgba(239, 68, 68, 0.6);
  }

  @keyframes pulse {
    0%, 100% {
      opacity: 1;
    }
    50% {
      opacity: 0.5;
    }
  }

  .status-text {
    color: rgba(255, 255, 255, 0.9);
    font-size: 14px;
    font-weight: 500;
  }

  .app-title {
    font-size: 24px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.95);
    margin-bottom: 8px;
  }

  .hint-text {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.6);
  }
</style>
