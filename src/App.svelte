<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';

  let selectedText: string = '';
  let translatedText: string = '';
  let isLoading: boolean = false;
  let showWindow: boolean = false;

  onMount(async () => {
    // 监听划词事件
    await listen('text-selected', async () => {
      try {
        selectedText = await invoke('get_selected_text');
        if (selectedText && selectedText.trim().length > 0) {
          showWindow = true;
          await translateText();
        }
      } catch (error) {
        console.error('获取选中文本失败:', error);
      }
    });
  });

  async function translateText() {
    if (!selectedText.trim()) return;

    isLoading = true;
    try {
      translatedText = await invoke('translate_text', {
        text: selectedText,
        mode: 'translate'
      });
    } catch (error) {
      console.error('翻译失败:', error);
      translatedText = '翻译失败: ' + error;
    } finally {
      isLoading = false;
    }
  }

  function closeWindow() {
    showWindow = false;
    selectedText = '';
    translatedText = '';
  }
</script>

<main class:visible={showWindow}>
  {#if showWindow}
    <div class="popup-window">
      <div class="header">
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
  {/if}
</main>

<style>
  main {
    display: none;
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    pointer-events: none;
  }

  main.visible {
    display: block;
  }

  .popup-window {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 400px;
    max-height: 500px;
    background: rgba(255, 255, 255, 0.95);
    backdrop-filter: blur(10px);
    border-radius: 12px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
    pointer-events: auto;
    overflow: hidden;
  }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    color: white;
  }

  .header h3 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }

  .close-btn {
    background: transparent;
    border: none;
    color: white;
    font-size: 28px;
    line-height: 1;
    cursor: pointer;
    padding: 0;
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 6px;
    transition: background 0.2s;
  }

  .close-btn:hover {
    background: rgba(255, 255, 255, 0.2);
  }

  .content {
    padding: 20px;
    max-height: 400px;
    overflow-y: auto;
  }

  .text-section {
    margin-bottom: 20px;
  }

  .text-section:last-child {
    margin-bottom: 0;
  }

  .text-section label {
    display: block;
    font-size: 12px;
    color: #666;
    margin-bottom: 8px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .source-text, .translated-text, .loading {
    margin: 0;
    padding: 12px;
    background: #f5f7fa;
    border-radius: 8px;
    line-height: 1.6;
    font-size: 14px;
    color: #333;
  }

  .loading {
    color: #999;
    font-style: italic;
  }
</style>
