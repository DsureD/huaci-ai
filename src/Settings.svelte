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

<div class="settings">
  {#if !config}
    <p class="loading">加载中...</p>
  {:else}
    <header>
      <h1>划词AI 设置</h1>
      <div class="actions">
        {#if status}<span class="status">{status}</span>{/if}
        <button class="save" on:click={save} disabled={saving}>
          {saving ? '保存中...' : '保存'}
        </button>
      </div>
    </header>

    <section>
      <h2>API 接口</h2>
      <p class="hint">支持配置多个接口，按优先级自动故障转移（数字越小越优先）。</p>

      {#each config.api.endpoints as ep, i}
        <div class="endpoint">
          <div class="endpoint-head">
            <label class="switch">
              <input type="checkbox" bind:checked={ep.enabled} />
              <span>启用</span>
            </label>
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

      <label class="row">
        请求超时（秒）
        <input type="number" bind:value={config.api.timeout_seconds} min="1" />
      </label>
    </section>

    <section>
      <h2>翻译行为</h2>
      <label class="row">
        最小触发字符数
        <input type="number" bind:value={config.app.min_text_length} min="1" />
      </label>
      <label class="row checkbox">
        <input type="checkbox" bind:checked={config.app.auto_translate} />
        选中后自动翻译
      </label>
    </section>

    <section>
      <h2>代理</h2>
      <label class="row checkbox">
        <input type="checkbox" bind:checked={config.proxy.enabled} />
        启用 HTTP 代理
      </label>
      {#if config.proxy.enabled}
        <div class="grid">
          <label>主机<input bind:value={config.proxy.host} placeholder="127.0.0.1" /></label>
          <label>端口<input type="number" bind:value={config.proxy.port} /></label>
        </div>
      {/if}
    </section>

    <section>
      <h2>提示词</h2>
      <label class="block">
        翻译提示词（用 <code>{'{text}'}</code> 代表选中文本）
        <textarea rows="3" bind:value={config.prompts.translate}></textarea>
      </label>
      <label class="block">
        解释提示词
        <textarea rows="3" bind:value={config.prompts.explain}></textarea>
      </label>
    </section>
  {/if}
</div>

<style>
  :global(html, body) {
    margin: 0;
    background: #f0f2f5;
  }

  .settings {
    font-family: -apple-system, "Segoe UI", "Microsoft YaHei", sans-serif;
    color: #1a1a1a;
    padding: 0 24px 32px;
    max-width: 640px;
    margin: 0 auto;
  }

  header {
    position: sticky;
    top: 0;
    background: #f0f2f5;
    padding: 20px 0 12px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    z-index: 10;
    border-bottom: 1px solid #e0e0e0;
  }

  h1 {
    font-size: 20px;
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
  }

  section {
    margin-top: 24px;
    background: #fff;
    border-radius: 12px;
    padding: 20px;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
  }

  h2 {
    font-size: 15px;
    margin: 0 0 12px;
    color: #444;
  }

  .hint {
    font-size: 12px;
    color: #999;
    margin: 0 0 16px;
  }

  .endpoint {
    border: 1px solid #eaeaea;
    border-radius: 10px;
    padding: 14px;
    margin-bottom: 14px;
  }

  .endpoint-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }

  .switch {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
  }

  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  label {
    font-size: 12px;
    color: #666;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  label.full {
    grid-column: 1 / -1;
  }

  input,
  textarea {
    font-size: 13px;
    padding: 8px 10px;
    border: 1px solid #d5d5d5;
    border-radius: 8px;
    font-family: inherit;
    box-sizing: border-box;
    width: 100%;
  }

  input:focus,
  textarea:focus {
    outline: none;
    border-color: #667eea;
  }

  .row {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    margin-top: 12px;
    font-size: 13px;
    color: #444;
  }

  .row input[type='number'] {
    width: 120px;
  }

  .row.checkbox {
    justify-content: flex-start;
    gap: 8px;
    cursor: pointer;
  }

  .row.checkbox input {
    width: auto;
  }

  .block {
    display: block;
    margin-top: 12px;
    font-size: 13px;
    color: #444;
  }

  .block textarea {
    margin-top: 6px;
    resize: vertical;
  }

  button {
    cursor: pointer;
    border: none;
    border-radius: 8px;
    font-size: 13px;
    padding: 8px 16px;
    font-family: inherit;
  }

  .save {
    background: #667eea;
    color: #fff;
    font-weight: 600;
  }

  .save:disabled {
    opacity: 0.6;
  }

  .add {
    background: #eef0fb;
    color: #667eea;
    width: 100%;
    padding: 10px;
    font-weight: 600;
  }

  .remove {
    background: #fdecea;
    color: #d32f2f;
    padding: 4px 12px;
  }

  code {
    background: #eee;
    padding: 1px 5px;
    border-radius: 4px;
    font-size: 12px;
  }

  .loading {
    text-align: center;
    color: #999;
    margin-top: 40px;
  }
</style>
