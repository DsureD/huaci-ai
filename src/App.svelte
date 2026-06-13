<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Settings from './Settings.svelte';

  // 通过 URL 参数区分窗口：?view=settings 为设置窗，其余为划词悬浮窗
  const view = new URLSearchParams(window.location.search).get('view');
  const isSettings = view === 'settings';

  let selectedText = '';
  let translatedText = '';
  let isLoading = false;

  onMount(async () => {
    if (isSettings) return;

    // 监听划词事件，事件负载为鼠标屏幕坐标 [x, y]
    await listen<[number, number]>('text-selected', async (event) => {
      try {
        const text = await invoke<string>('get_selected_text');
        if (!text || text.trim().length === 0) return;

        selectedText = text;
        translatedText = '';
        isLoading = true;

        // 真正显示原生窗口，并定位到鼠标附近
        const [x, y] = event.payload ?? [0, 0];
        await invoke('show_popup', { x, y });

        await translateText();
      } catch (error) {
        console.error('划词处理失败:', error);
      }
    });
  });

  async function translateText() {
    if (!selectedText.trim()) return;
    isLoading = true;
    try {
      translatedText = await invoke<string>('translate_text', {
        text: selectedText,
        mode: 'translate',
      });
    } catch (error) {
      translatedText = '翻译失败: ' + error;
    } finally {
      isLoading = false;
    }
  }

  async function closeWindow() {
    selectedText = '';
    translatedText = '';
    await invoke('hide_popup');
  }
</script>

{#if isSettings}
  <Settings />
{:else}
  <main>
    <div class="popup-window">
      <div class="header" data-tauri-drag-region>
        <h3>划词翻译</h3>
        <button class="close-btn" on:click={closeWindow}>×</button>
      </div>

      <div class="content">
        <div class="text-section">
          <label>原文</label>
          <p class="source-text">{selectedText}</p>
        </div>

        <div class="text-section">
          <label>译文</label>
          {#if isLoading}
            <p class="loading">翻译中...</p>
          {:else}
            <p class="translated-text">{translatedText}</p>
          {/if}
        </div>
      </div>
    </div>
  </main>
{/if}

<style>
  :global(html, body) {
    margin: 0;
    padding: 0;
    background: transparent;
  }

  main {
    width: 100vw;
    height: 100vh;
    overflow: hidden;
  }

  .popup-window {
    width: 100%;
    height: 100%;
    background: rgba(255, 255, 255, 0.98);
    border-radius: 12px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.25);
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 16px;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    color: white;
    cursor: move;
    user-select: none;
  }

  .header h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }

  .close-btn {
    background: transparent;
    border: none;
    color: white;
    font-size: 24px;
    line-height: 1;
    cursor: pointer;
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 6px;
  }

  .close-btn:hover {
    background: rgba(255, 255, 255, 0.2);
  }

  .content {
    padding: 16px;
    overflow-y: auto;
    flex: 1;
  }

  .text-section {
    margin-bottom: 16px;
  }

  .text-section:last-child {
    margin-bottom: 0;
  }

  .text-section label {
    display: block;
    font-size: 11px;
    color: #888;
    margin-bottom: 6px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .source-text,
  .translated-text,
  .loading {
    margin: 0;
    padding: 10px 12px;
    background: #f5f7fa;
    border-radius: 8px;
    line-height: 1.6;
    font-size: 14px;
    color: #333;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .loading {
    color: #999;
    font-style: italic;
  }
</style>
