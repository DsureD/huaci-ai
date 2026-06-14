<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import Settings from './Settings.svelte';

  const view = new URLSearchParams(window.location.search).get('view');
  const isSettings = view === 'settings';

  // 仅在划词窗口禁止页面滚动；设置窗口需要正常滚动
  if (!isSettings) {
    document.documentElement.classList.add('popup-window');
  }

  let selectedText = '';
  let mode: 'toolbar' | 'result' = 'toolbar'; // toolbar=工具条, result=翻译结果
  let resultText = '';
  let isLoading = false;
  let currentAction: 'translate' | 'explain' = 'translate';
  let rootEl: HTMLElement;

  // 根据内容把窗口调整为刚好包住内容的大小（交给 Rust 设置，避免前端 DPI 计算出错）
  async function resizeToContent() {
    await tick();
    // 等一帧，确保 WebView 已完成布局
    await new Promise((r) => requestAnimationFrame(() => r(null)));
    if (!rootEl) return;
    const width = Math.ceil(rootEl.offsetWidth);
    const height = Math.ceil(rootEl.offsetHeight);
    if (width <= 0 || height <= 0) return;
    try {
      await invoke('resize_popup', { width, height });
    } catch (e) {
      console.error('调整窗口尺寸失败:', e);
    }
  }

  // 内容变化时重新测量
  $: if (!isSettings && rootEl) {
    void mode;
    void resultText;
    void isLoading;
    resizeToContent();
  }

  onMount(async () => {
    if (isSettings) return;

    const currentWin = getCurrentWindow();

    await listen<[number, number]>('text-selected', async (event) => {
      try {
        const [text, autoTranslate] = await invoke<[string, boolean]>('get_selected_text');
        if (!text || text.trim().length === 0) return;

        selectedText = text;
        mode = 'toolbar';
        resultText = '';

        const [x, y] = event.payload ?? [0, 0];
        await invoke('show_popup', { x, y });
        // 显示后再按内容收紧尺寸
        await resizeToContent();

        // 如果开启了自动翻译，直接执行
        if (autoTranslate) {
          await doTranslate('translate');
        }
      } catch (error) {
        console.error('划词处理失败:', error);
      }
    });

    // 点击窗口外部（弹窗失去焦点）自动关闭
    await currentWin.onFocusChanged(({ payload: focused }) => {
      if (!focused) closeWindow();
    });

    // 按 ESC 关闭
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') closeWindow();
    });
  });

  async function doTranslate(action: 'translate' | 'explain') {
    currentAction = action;
    mode = 'result';
    isLoading = true;
    resultText = '';
    await resizeToContent();

    try {
      resultText = await invoke<string>('translate_text', {
        text: selectedText,
        mode: action,
      });
    } catch (error) {
      resultText = '失败: ' + error;
    } finally {
      isLoading = false;
      await resizeToContent();
    }
  }

  async function closeWindow() {
    selectedText = '';
    resultText = '';
    mode = 'toolbar';
    await invoke('hide_popup');
  }
</script>

{#if isSettings}
  <Settings />
{:else}
  <div class="popup" bind:this={rootEl}>
    {#if mode === 'toolbar'}
      <div class="toolbar">
        <button class="action-btn" on:click={() => doTranslate('translate')}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M5 8h14M5 12h14M5 16h10" />
          </svg>
          翻译
        </button>
        <div class="divider"></div>
        <button class="action-btn" on:click={() => doTranslate('explain')}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="10" />
            <path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3M12 17h.01" />
          </svg>
          解释
        </button>
        <button class="close-btn" on:click={closeWindow} aria-label="关闭">×</button>
      </div>
    {:else}
      <div class="result-card">
        <div class="card-head">
          <span class="badge" class:explain={currentAction === 'explain'}>
            {currentAction === 'translate' ? '翻译' : '解释'}
          </span>
          <button class="mini-close" on:click={closeWindow} aria-label="关闭">×</button>
        </div>
        {#if isLoading}
          <div class="loading-dots">
            <span></span><span></span><span></span>
          </div>
        {:else}
          <div class="result-text">{resultText}</div>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  :global(html, body) {
    margin: 0;
    padding: 0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Microsoft YaHei', sans-serif;
  }

  /* 仅划词悬浮窗：透明背景 + 禁止滚动 */
  :global(html.popup-window),
  :global(html.popup-window body) {
    background: transparent;
    overflow: hidden;
  }

  /* 内容容器：宽高随内容自适应（窗口被 Rust 设为同样大小）
     留一点点内边距给阴影，营造悬浮感但不显空旷 */
  .popup {
    display: inline-block;
    padding: 8px;
    box-sizing: border-box;
  }

  /* 工具条 - 玻璃态横条 */
  .toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 8px;
    background: rgba(255, 255, 255, 0.92);
    backdrop-filter: blur(20px) saturate(180%);
    border: 1px solid rgba(0, 0, 0, 0.1);
    border-radius: 12px;
    box-shadow: 0 3px 10px rgba(0, 0, 0, 0.16);
  }

  .action-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 6px 12px;
    border: none;
    background: transparent;
    color: #333;
    font-size: 13px;
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.15s;
    white-space: nowrap;
  }

  .action-btn:hover {
    background: rgba(0, 0, 0, 0.06);
  }

  .action-btn svg {
    flex-shrink: 0;
  }

  .divider {
    width: 1px;
    height: 16px;
    background: rgba(0, 0, 0, 0.1);
    margin: 0 2px;
  }

  .close-btn {
    padding: 4px 8px;
    border: none;
    background: transparent;
    color: #666;
    font-size: 18px;
    line-height: 1;
    cursor: pointer;
    border-radius: 6px;
    transition: background 0.15s, color 0.15s;
  }

  .close-btn:hover {
    background: rgba(0, 0, 0, 0.08);
    color: #333;
  }

  /* 翻译结果卡片 */
  .result-card {
    width: 360px;
    padding: 10px 12px 12px;
    background: rgba(252, 252, 253, 0.97);
    backdrop-filter: blur(24px) saturate(180%);
    border: 1px solid rgba(0, 0, 0, 0.1);
    border-radius: 14px;
    box-shadow: 0 3px 12px rgba(0, 0, 0, 0.16);
    box-sizing: border-box;
  }

  .card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }

  .badge {
    font-size: 11px;
    font-weight: 600;
    color: #4f6bed;
    background: rgba(79, 107, 237, 0.1);
    padding: 2px 8px;
    border-radius: 6px;
    letter-spacing: 0.5px;
  }

  .badge.explain {
    color: #d97706;
    background: rgba(217, 119, 6, 0.12);
  }

  .result-text {
    margin: 0;
    font-size: 14px;
    line-height: 1.65;
    color: #1a1a1a;
    white-space: pre-wrap;
    word-break: break-word;
    overflow-wrap: anywhere;
    max-height: 380px;
    overflow-y: auto;
    overflow-x: hidden;
    /* Firefox 滚动条 */
    scrollbar-width: thin;
    scrollbar-color: rgba(0, 0, 0, 0.18) transparent;
  }

  .result-text::-webkit-scrollbar {
    width: 6px;
  }

  .result-text::-webkit-scrollbar-track {
    background: transparent;
  }

  .result-text::-webkit-scrollbar-thumb {
    background: rgba(0, 0, 0, 0.18);
    border-radius: 3px;
  }

  .result-text::-webkit-scrollbar-thumb:hover {
    background: rgba(0, 0, 0, 0.3);
  }

  .loading-dots {
    display: flex;
    gap: 6px;
    justify-content: center;
    align-items: center;
    padding: 12px 0;
  }

  .loading-dots span {
    width: 6px;
    height: 6px;
    background: rgba(79, 107, 237, 0.6);
    border-radius: 50%;
    animation: bounce 1.2s infinite ease-in-out;
  }

  .loading-dots span:nth-child(1) {
    animation-delay: -0.32s;
  }

  .loading-dots span:nth-child(2) {
    animation-delay: -0.16s;
  }

  @keyframes bounce {
    0%,
    80%,
    100% {
      transform: scale(0);
      opacity: 0.5;
    }
    40% {
      transform: scale(1);
      opacity: 1;
    }
  }

  .mini-close {
    width: 22px;
    height: 22px;
    padding: 0;
    border: none;
    background: rgba(0, 0, 0, 0.05);
    color: #666;
    font-size: 15px;
    line-height: 1;
    cursor: pointer;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.15s, color 0.15s;
  }

  .mini-close:hover {
    background: rgba(0, 0, 0, 0.12);
    color: #333;
  }
</style>
