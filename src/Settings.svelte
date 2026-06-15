<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { getVersion } from '@tauri-apps/api/app';
  import Icon from './Icon.svelte';
  import { ICONS } from './icons';

  interface ApiEndpoint {
    name: string;
    base_url: string;
    api_key: string;
    model: string;
    enabled: boolean;
    priority: number;
  }

  interface ActionItem {
    name: string;
    prompt: string;
    icon: string;
    enabled: boolean;
  }

  interface Config {
    app: { auto_translate: boolean; min_text_length: number };
    hotkeys: { toggle_capture: string; manual_translate: string };
    api: { timeout_seconds: number; endpoints: ApiEndpoint[] };
    proxy: { enabled: boolean; host: string; port: number };
    ui: { opacity: number; font_size: number };
    prompts: { translate: string; explain: string };
    actions: ActionItem[];
  }

  let config: Config | null = null;
  let status = '';
  let saving = false;
  let tab: 'api' | 'features' | 'about' = 'api';
  let appVersion = '';

  onMount(async () => {
    try {
      config = await invoke<Config>('load_config');
    } catch (e) {
      status = '加载配置失败: ' + e;
    }
    try {
      appVersion = await getVersion();
    } catch {
      appVersion = '';
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

  // ===== 划词功能项 =====
  let iconPickerOpen: Record<number, boolean> = {};

  function addAction() {
    if (!config) return;
    config.actions = [
      ...config.actions,
      { name: '新功能', prompt: '请处理以下内容：\n\n{text}', icon: 'star', enabled: true },
    ];
  }

  function removeAction(index: number) {
    if (!config) return;
    config.actions = config.actions.filter((_, i) => i !== index);
  }

  function pickIcon(i: number, key: string) {
    if (!config) return;
    config.actions[i].icon = key;
    config.actions = config.actions;
    iconPickerOpen[i] = false;
    iconPickerOpen = iconPickerOpen;
  }

  // 模型查询状态（按接口索引）
  let modelOptions: Record<number, string[]> = {};
  let modelLoading: Record<number, boolean> = {};
  let modelMsg: Record<number, string> = {};
  let modelOpen: Record<number, boolean> = {};

  async function queryModels(i: number) {
    if (!config) return;
    const ep = config.api.endpoints[i];
    if (!ep.base_url) {
      modelMsg[i] = '请先填写接口地址';
      modelMsg = modelMsg;
      return;
    }
    modelLoading[i] = true;
    modelLoading = modelLoading;
    modelMsg[i] = '';
    modelMsg = modelMsg;
    try {
      const list = await invoke<string[]>('list_models', {
        baseUrl: ep.base_url,
        apiKey: ep.api_key,
      });
      modelOptions[i] = list;
      modelOptions = modelOptions;
      if (list.length) {
        modelMsg[i] = `找到 ${list.length} 个模型，点击输入框可下拉选择`;
        modelOpen[i] = true;
        modelOpen = modelOpen;
      } else {
        modelMsg[i] = '接口未返回模型';
      }
      modelMsg = modelMsg;
    } catch (e) {
      modelMsg[i] = '查询失败: ' + e;
      modelMsg = modelMsg;
    } finally {
      modelLoading[i] = false;
      modelLoading = modelLoading;
    }
  }

  function pickModel(i: number, m: string) {
    if (!config) return;
    config.api.endpoints[i].model = m;
    config.api.endpoints = config.api.endpoints;
    modelOpen[i] = false;
    modelOpen = modelOpen;
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

    <nav class="tabs">
      <button class="tab" class:active={tab === 'api'} on:click={() => (tab = 'api')}>API</button>
      <button class="tab" class:active={tab === 'features'} on:click={() => (tab = 'features')}>功能</button>
      <button class="tab" class:active={tab === 'about'} on:click={() => (tab = 'about')}>关于</button>
    </nav>

    <div class="settings">
      {#if tab === 'api'}
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
                <div class="full model-field">
                  <span class="label-text">模型</span>
                  <div class="model-row">
                    <div class="combo">
                      <input
                        bind:value={ep.model}
                        placeholder="gpt-4o-mini"
                        on:focus={() => {
                          if (modelOptions[i]?.length) {
                            modelOpen[i] = true;
                            modelOpen = modelOpen;
                          }
                        }}
                        on:blur={() => {
                          setTimeout(() => {
                            modelOpen[i] = false;
                            modelOpen = modelOpen;
                          }, 150);
                        }}
                      />
                      {#if modelOptions[i]?.length}
                        <button
                          class="combo-toggle"
                          aria-label="展开模型列表"
                          on:mousedown|preventDefault={() => {
                            modelOpen[i] = !modelOpen[i];
                            modelOpen = modelOpen;
                          }}
                        >▾</button>
                      {/if}
                      {#if modelOpen[i] && modelOptions[i]?.length}
                        <ul class="combo-list">
                          {#each modelOptions[i].slice(0, 100) as m}
                            <li
                              class:active={m === ep.model}
                              on:mousedown|preventDefault={() => pickModel(i, m)}
                            >{m}</li>
                          {/each}
                          {#if modelOptions[i].length > 100}
                            <li class="overflow-hint">... 还有 {modelOptions[i].length - 100} 个模型,请输入筛选</li>
                          {/if}
                        </ul>
                      {/if}
                    </div>
                    <button class="query" on:click={() => queryModels(i)} disabled={modelLoading[i]}>
                      {modelLoading[i] ? '查询中…' : '查询模型'}
                    </button>
                  </div>
                  {#if modelMsg[i]}
                    <span class="model-msg" class:err={modelMsg[i].includes('失败')}>{modelMsg[i]}</span>
                  {/if}
                </div>
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
      {/if}

      {#if tab === 'features'}
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
          <h2>划词功能</h2>
          <span class="sec-desc">弹窗里的功能按钮，用 <code>{'{text}'}</code> 代表选中文本</span>
        </div>
        <div class="sec-body">
          {#each config.actions as act, i}
            <div class="action-item" class:disabled={!act.enabled}>
              <div class="action-head">
                <button
                  class="icon-pick"
                  title="选择图标"
                  on:click={() => {
                    iconPickerOpen[i] = !iconPickerOpen[i];
                    iconPickerOpen = iconPickerOpen;
                  }}
                >
                  <Icon name={act.icon} size={18} />
                </button>
                <input class="action-name" bind:value={act.name} placeholder="功能名称" />
                <label class="switch">
                  <input type="checkbox" bind:checked={act.enabled} />
                  <span>{act.enabled ? '显示' : '隐藏'}</span>
                </label>
                <button class="remove" on:click={() => removeAction(i)}>删除</button>
              </div>
              {#if iconPickerOpen[i]}
                <div class="icon-grid">
                  {#each ICONS as ic}
                    <button
                      class="icon-cell"
                      class:active={ic.key === act.icon}
                      title={ic.label}
                      on:click={() => pickIcon(i, ic.key)}
                    >
                      <Icon name={ic.key} size={18} />
                    </button>
                  {/each}
                </div>
              {/if}
              <textarea class="action-prompt" rows="3" bind:value={act.prompt} placeholder="提示词模板，用 {'{text}'} 代表选中文本"></textarea>
            </div>
          {/each}

          <button class="add" on:click={addAction}>+ 添加功能</button>
        </div>
      </section>
      {/if}

      {#if tab === 'about'}
      <section>
        <div class="sec-head">
          <h2>关于</h2>
        </div>
        <div class="sec-body about">
          <div class="about-logo">划词AI</div>
          <p class="about-line">版本 {appVersion || '—'}</p>
          <p class="about-line">作者 DsureD</p>
          <p class="about-desc">轻量级划词翻译工具</p>
        </div>
      </section>
      {/if}
    </div>
  {/if}
</div>

<style>
  :global(html, body) {
    margin: 0;
    background: #eef0f4;
  }

  :global(body) {
    transform: translateZ(0);
    backface-visibility: hidden;
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
    background: #eef0f4;
    padding: 16px 24px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    z-index: 10;
    border-bottom: 1px solid #dcdfe6;
    transform: translateZ(0);
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

  /* ===== Tab 栏 ===== */
  .tabs {
    display: flex;
    gap: 4px;
    max-width: 640px;
    margin: 0 auto;
    padding: 12px 24px 0;
  }

  .tab {
    background: transparent;
    color: #6a707c;
    font-weight: 600;
    padding: 8px 18px;
    border-radius: 9px 9px 0 0;
  }

  .tab:hover {
    color: #2b2f38;
    background: rgba(0, 0, 0, 0.03);
  }

  .tab.active {
    color: #4f6bed;
    background: #fff;
    box-shadow: 0 -1px 4px rgba(0, 0, 0, 0.04);
  }

  /* ===== 关于页 ===== */
  .sec-body.about {
    text-align: center;
    padding: 32px 20px;
  }

  .about-logo {
    font-size: 22px;
    font-weight: 800;
    color: #4f6bed;
    margin-bottom: 14px;
  }

  .about-line {
    font-size: 14px;
    color: #2b2f38;
    margin: 6px 0;
  }

  .about-desc {
    font-size: 12px;
    color: #9aa0ac;
    margin-top: 16px;
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
    transform: translateZ(0);
  }

  .sec-head {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 16px 20px;
    border-bottom: 1px solid #f0f1f4;
    background: #fafbfc;
    border-radius: 14px 14px 0 0;
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
    transform: translateZ(0);
    will-change: opacity;
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

  /* 模型字段：输入 + 查询按钮 */
  .model-field {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .label-text {
    font-size: 12px;
    color: #6a707c;
  }

  .model-row {
    display: flex;
    gap: 8px;
    align-items: stretch;
  }

  .model-row input {
    flex: 1;
  }

  /* 组合框：单输入框 + 下拉 */
  .combo {
    position: relative;
    flex: 1;
    display: flex;
  }

  .combo input {
    flex: 1;
    padding-right: 30px;
  }

  .combo-toggle {
    position: absolute;
    right: 1px;
    top: 1px;
    bottom: 1px;
    width: 28px;
    padding: 0;
    border: none;
    background: transparent;
    color: #8a909c;
    font-size: 12px;
    cursor: pointer;
    border-radius: 0 8px 8px 0;
  }

  .combo-toggle:hover {
    color: #4f6bed;
  }

  .combo-list {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    width: 100%;
    margin: 0;
    padding: 4px;
    list-style: none;
    background: #fff;
    border: 1px solid #d8dbe2;
    border-radius: 10px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
    max-height: 220px;
    overflow-y: auto;
    z-index: 50;
    box-sizing: border-box;
  }

  .combo-list li {
    padding: 7px 10px;
    font-size: 13px;
    color: #2b2f38;
    border-radius: 7px;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .combo-list li:hover {
    background: #f0f2f8;
  }

  .combo-list li.active {
    background: #eef1fd;
    color: #4f6bed;
    font-weight: 600;
  }

  .overflow-hint {
    color: #8a909c;
    font-size: 12px;
    padding: 7px 10px;
    text-align: center;
    cursor: default !important;
    background: transparent !important;
  }

  .query {
    flex: none;
    white-space: nowrap;
    background: #eef1fd;
    color: #4f6bed;
    font-weight: 600;
    padding: 0 14px;
  }

  .query:hover:not(:disabled) {
    background: #e3e8fc;
  }

  .query:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .model-msg {
    font-size: 12px;
    color: #2e7d32;
  }

  .model-msg.err {
    color: #d32f2f;
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

  /* ===== 划词功能项 ===== */
  .action-item {
    border: 1px solid #e8eaef;
    border-radius: 12px;
    padding: 14px;
    margin-bottom: 14px;
    background: #fdfdfe;
    transition: border-color 0.15s, opacity 0.15s, box-shadow 0.15s;
  }

  .action-item.disabled {
    opacity: 0.6;
  }

  .action-head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 10px;
  }

  .icon-pick {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    padding: 0;
    flex: none;
    background: #eef1fd;
    color: #4f6bed;
    border-radius: 9px;
  }

  .icon-pick:hover {
    background: #e3e8fc;
  }

  .action-name {
    flex: 1;
    min-width: 0;
  }

  .icon-grid {
    display: grid;
    grid-template-columns: repeat(8, 1fr);
    gap: 6px;
    padding: 10px;
    margin-bottom: 10px;
    background: #f6f7fb;
    border-radius: 10px;
  }

  .icon-cell {
    display: flex;
    align-items: center;
    justify-content: center;
    aspect-ratio: 1;
    padding: 0;
    background: #fff;
    color: #6a707c;
    border: 1px solid #e8eaef;
    border-radius: 8px;
  }

  .icon-cell:hover {
    color: #4f6bed;
    border-color: #c3ccf6;
  }

  .icon-cell.active {
    background: #eef1fd;
    color: #4f6bed;
    border-color: #4f6bed;
  }

  .action-prompt {
    resize: vertical;
    line-height: 1.5;
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
