<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import Settings from './Settings.svelte';

  const view = new URLSearchParams(window.location.search).get('view');
  const isSettings = view === 'settings';

  let selectedText = '';
  let mode: 'toolbar' | 'result' = 'toolbar'; // toolbar=工具条, result=翻译结果
  let resultText = '';
  let isLoading = false;
  let currentAction: 'translate' | 'explain' = 'translate';

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

        // 如果开启了自动翻译，直接执行
        if (autoTranslate) {
          await doTranslate('translate');
        }
      } catch (error) {
        console.error('划词处理失败:', error);
      }
    });

    // 点击窗口外部或按 ESC 关闭
    document.addEventListener('click', (e) => {
      if (e.target === document.body) closeWindow();
    });
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') closeWindow();
    });
  });

  async function doTranslate(action: 'translate' | 'explain') {
    currentAction = action;
    mode = 'result';
    isLoading = true;
    resultText = '';

    try {
      resultText = await invoke<string>('translate_text', {
        text: selectedText,
        mode: action,
      });
    } catch (error) {
      resultText = '失败: ' + error;
    } finally {
      isLoading = false;
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
  <main>
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
        <button class="close-btn" on:click={closeWindow}>×</button>
      </div>
    {:else}
      <div class="result-card">
        {#if isLoading}
          <div class="loading-dots">
            <span></span><span></span><span></span>
          </div>
        {:else}
          <p class="result-text">{resultText}</p>
        {/if}
        <button class="mini-close" on:click={closeWindow}>×</button>
      </div>
    {/if}
  </main>
{/if}

<style>
  :global(html, body) {
    margin: 0;
    padding: 0;
    background: transparent;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Microsoft YaHei', sans-serif;
  }

  main {
    width: 100vw;
    height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 8px;
    box-sizing: border-box;
  }

  /* 工具条 - 玻璃态横条 */
  .toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 8px;
    background: rgba(255, 255, 255, 0.85);
    backdrop-filter: blur(20px) saturate(180%);
    border: 1px solid rgba(0, 0, 0, 0.08);
    border-radius: 10px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.08), 0 1px 4px rgba(0, 0, 0, 0.04);
  }

  .action-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 12px;
    border: none;
    background: transparent;
    color: #333;
    font-size: 13px;
    border-radius: 7px;
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

  /* 翻译结果卡片 - 窄长条半透明 */
  .result-card {
    position: relative;
    width: 100%;
    max-width: 500px;
    padding: 12px 36px 12px 14px;
    background: rgba(250, 250, 250, 0.92);
    backdrop-filter: blur(24px) saturate(180%);
    border: 1px solid rgba(0, 0, 0, 0.06);
    border-radius: 12px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.1), 0 2px 6px rgba(0, 0, 0, 0.06);
    box-sizing: border-box;
  }

  .result-text {
    margin: 0;
    font-size: 14px;
    line-height: 1.6;
    color: #1a1a1a;
    word-break: break-word;
    max-height: 200px;
    overflow-y: auto;
  }

  .result-text::-webkit-scrollbar {
    width: 4px;
  }

  .result-text::-webkit-scrollbar-thumb {
    background: rgba(0, 0, 0, 0.2);
    border-radius: 2px;
  }

  .loading-dots {
    display: flex;
    gap: 6px;
    justify-content: center;
    align-items: center;
    padding: 8px 0;
  }

  .loading-dots span {
    width: 6px;
    height: 6px;
    background: rgba(0, 0, 0, 0.3);
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
    position: absolute;
    top: 8px;
    right: 8px;
    width: 22px;
    height: 22px;
    padding: 0;
    border: none;
    background: rgba(0, 0, 0, 0.05);
    color: #666;
    font-size: 16px;
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
