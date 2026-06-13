<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  interface ApiEndpoint {
    name: string;
    base_url: string;
    api_key: string;
    model: string;
    enabled: boolean;
    priority: number;
  }

  interface Config {
    app: { auto_translate: boolean; min_text_length: number };
    hotkeys: { toggle_capture: string; manual_translate: string };
    api: { timeout_seconds: number; endpoints: ApiEndpoint[] };
    proxy: { enabled: boolean; host: string; port: number };
    ui: { opacity: number; font_size: number };
    prompts: { translate: string; explain: string };
  }

  let config: Config | null = null;
  let status = '';
  let saving = false;

  onMount(async () => {
    try {
      config = await invoke<Config>('load_config');
    } catch (e) {
      status = '加载配置失败: ' + e;
    }
  });

  function addEndpoint() {
    if (!config) return;
    config.api.endpoints = [
      ...config.api.endpoints,
      {
        name: '新接口',
        base_url: 'https://api.openai.com/v1',
        api_key: '',
        model: 'gpt-4o-mini',
        enabled: true,
        priority: config.api.endpoints.length + 1,
      },
    ];
  }

  function removeEndpoint(index: number) {
    if (!config) return;
    config.api.endpoints = config.api.endpoints.filter((_, i) => i !== index);
  }

  async function save() {
    if (!config) return;
    saving = true;
    status = '';
    try {
      await invoke('save_config', { newConfig: config });
      status = '✓ 已保存';
      setTimeout(() => (status = ''), 2000);
    } catch (e) {
      status = '保存失败: ' + e;
    } finally {
      saving = false;
    }
  }
</script>

<div class="page">
  {#if !config}
    <p class="loading">加载中...</p>
  {:else}
    <header>
      <div class="title-wrap">
        <h1>划词AI 设置</h1>
        <p class="subtitle">配置 AI 接口与划词行为</p>
      </div>
      <div class="actions">
        {#if status}<span class="status" class:error={status.includes('失败')}>{status}</span>{/if}
        <button class="save" on:click={save} disabled={saving}>
          {saving ? '保存中...' : '保存'}
        </button>
      </div>
    </header>

    <div class="settings">
      <section>
        <div class="sec-head">
          <h2>API 接口</h2>
          <span class="sec-desc">按优先级自动故障转移，数字越小越优先</span>
        </div>

        <div class="sec-body">
          {#each config.api.endpoints as ep, i}
            <div class="endpoint" class:disabled={!ep.enabled}>
              <div class="endpoint-head">
                <label class="switch">
                  <input type="checkbox" bind:checked={ep.enabled} />
                  <span>{ep.enabled ? '已启用' : '已禁用'}</span>
                </label>
                <span class="ep-name">{ep.name || '未命名接口'}</span>
                <button class="remove" on:click={() => removeEndpoint(i)}>删除</button>
              </div>
              <div class="grid">
                <label>名称<input bind:value={ep.name} placeholder="OpenAI" /></label>
                <label>优先级<input type="number" bind:value={ep.priority} min="1" /></label>
                <label class="full">接口地址 (Base URL)<input bind:value={ep.base_url} placeholder="https://api.openai.com/v1" /></label>
                <label class="full">API Key<input type="password" bind:value={ep.api_key} placeholder="sk-..." /></label>
                <label class="full">模型<input bind:value={ep.model} placeholder="gpt-4o-mini" /></label>
              </div>
            </div>
          {/each}

          <button class="add" on:click={addEndpoint}>+ 添加接口</button>

          <div class="field-row">
            <span class="field-label">请求超时（秒）</span>
            <input type="number" bind:value={config.api.timeout_seconds} min="1" />
          </div>
        </div>
      </section>

      <section>
        <div class="sec-head">
          <h2>翻译行为</h2>
        </div>
        <div class="sec-body">
          <div class="field-row">
            <span class="field-label">最小触发字符数</span>
            <input type="number" bind:value={config.app.min_text_length} min="1" />
          </div>
          <label class="toggle-row">
            <input type="checkbox" bind:checked={config.app.auto_translate} />
            <span>选中后自动翻译</span>
          </label>
        </div>
      </section>

      <section>
        <div class="sec-head">
          <h2>代理</h2>
        </div>
        <div class="sec-body">
          <label class="toggle-row">
            <input type="checkbox" bind:checked={config.proxy.enabled} />
            <span>启用 HTTP 代理</span>
          </label>
          {#if config.proxy.enabled}
            <div class="grid">
              <label>主机<input bind:value={config.proxy.host} placeholder="127.0.0.1" /></label>
              <label>端口<input type="number" bind:value={config.proxy.port} /></label>
            </div>
          {/if}
        </div>
      </section>

      <section>
        <div class="sec-head">
          <h2>提示词</h2>
          <span class="sec-desc">用 <code>{'{text}'}</code> 代表选中文本</span>
        </div>
        <div class="sec-body">
          <label class="block">
            <span class="field-label">翻译提示词</span>
            <textarea rows="3" bind:value={config.prompts.translate}></textarea>
          </label>
          <label class="block">
            <span class="field-label">解释提示词</span>
            <textarea rows="3" bind:value={config.prompts.explain}></textarea>
          </label>
        </div>
      </section>
    </div>
  {/if}
</div>

<style>
  :global(html, body) {
    margin: 0;
    background: #eef0f4;
  }

  :global(body)::-webkit-scrollbar {
    width: 10px;
  }
  :global(body)::-webkit-scrollbar-thumb {
    background: rgba(0, 0, 0, 0.18);
    border-radius: 5px;
    border: 2px solid #eef0f4;
  }

  .page {
    font-family: -apple-system, 'Segoe UI', 'Microsoft YaHei', sans-serif;
    color: #1a1a1a;
  }

  header {
    position: sticky;
    top: 0;
    background: rgba(238, 240, 244, 0.85);
    backdrop-filter: blur(12px);
    padding: 16px 24px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    z-index: 10;
    border-bottom: 1px solid #dcdfe6;
  }

  .title-wrap {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  h1 {
    font-size: 19px;
    font-weight: 700;
    margin: 0;
  }

  .subtitle {
    font-size: 12px;
    color: #8a909c;
    margin: 0;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .status {
    font-size: 13px;
    color: #2e7d32;
    font-weight: 500;
  }

  .status.error {
    color: #d32f2f;
  }

  .settings {
    max-width: 640px;
    margin: 0 auto;
    padding: 24px 24px 48px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  section {
    background: #fff;
    border-radius: 14px;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04), 0 4px 16px rgba(0, 0, 0, 0.04);
    overflow: hidden;
  }

  .sec-head {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 16px 20px;
    border-bottom: 1px solid #f0f1f4;
    background: #fafbfc;
  }

  h2 {
    font-size: 14px;
    font-weight: 700;
    margin: 0;
    color: #2b2f38;
    position: relative;
    padding-left: 12px;
  }

  h2::before {
    content: '';
    position: absolute;
    left: 0;
    top: 50%;
    transform: translateY(-50%);
    width: 4px;
    height: 14px;
    border-radius: 2px;
    background: #4f6bed;
  }

  .sec-desc {
    font-size: 12px;
    color: #9aa0ac;
  }

  .sec-body {
    padding: 20px;
  }

  .endpoint {
    border: 1px solid #e8eaef;
    border-radius: 12px;
    padding: 16px;
    margin-bottom: 14px;
    background: #fdfdfe;
    transition: border-color 0.15s, opacity 0.15s;
  }

  .endpoint.disabled {
    opacity: 0.6;
  }

  .endpoint-head {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 14px;
  }

  .switch {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: #444;
    cursor: pointer;
  }

  .switch input {
    width: auto;
    cursor: pointer;
  }

  .ep-name {
    font-size: 13px;
    font-weight: 600;
    color: #2b2f38;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  label {
    font-size: 12px;
    color: #6a707c;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  label.full {
    grid-column: 1 / -1;
  }

  input,
  textarea {
    font-size: 13px;
    padding: 9px 11px;
    border: 1px solid #d8dbe2;
    border-radius: 9px;
    font-family: inherit;
    box-sizing: border-box;
    width: 100%;
    background: #fff;
    color: #1a1a1a;
    transition: border-color 0.15s, box-shadow 0.15s;
  }

  input:focus,
  textarea:focus {
    outline: none;
    border-color: #4f6bed;
    box-shadow: 0 0 0 3px rgba(79, 107, 237, 0.12);
  }

  /* 横向字段：标签 + 输入 */
  .field-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .field-row + .field-row,
  .field-row + .toggle-row,
  .toggle-row + .field-row {
    margin-top: 14px;
  }

  .field-label {
    font-size: 13px;
    color: #2b2f38;
    font-weight: 500;
  }

  .field-row input[type='number'] {
    width: 120px;
    flex: none;
  }

  .toggle-row {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: #2b2f38;
    cursor: pointer;
  }

  .toggle-row input {
    width: auto;
    cursor: pointer;
  }

  .block {
    display: block;
    font-size: 13px;
  }

  .block + .block {
    margin-top: 16px;
  }

  .block textarea {
    margin-top: 6px;
    resize: vertical;
    line-height: 1.5;
  }

  button {
    cursor: pointer;
    border: none;
    border-radius: 9px;
    font-size: 13px;
    padding: 8px 16px;
    font-family: inherit;
    transition: opacity 0.15s, background 0.15s;
  }

  .save {
    background: #4f6bed;
    color: #fff;
    font-weight: 600;
    box-shadow: 0 2px 6px rgba(79, 107, 237, 0.3);
  }

  .save:hover:not(:disabled) {
    background: #4159d8;
  }

  .save:disabled {
    opacity: 0.6;
    box-shadow: none;
  }

  .add {
    background: #eef1fd;
    color: #4f6bed;
    width: 100%;
    padding: 11px;
    font-weight: 600;
    margin-bottom: 16px;
  }

  .add:hover {
    background: #e3e8fc;
  }

  .remove {
    background: #fdecea;
    color: #d32f2f;
    padding: 5px 12px;
    font-size: 12px;
  }

  .remove:hover {
    background: #fbdbd7;
  }

  code {
    background: #eef1fd;
    color: #4f6bed;
    padding: 1px 6px;
    border-radius: 5px;
    font-size: 12px;
    font-family: 'SFMono-Regular', Consolas, monospace;
  }

  .loading {
    text-align: center;
    color: #999;
    margin-top: 40px;
  }
</style>
