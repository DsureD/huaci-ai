<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import { open } from '@tauri-apps/plugin-shell';
  import { marked } from 'marked';
  import Settings from './Settings.svelte';
  import Icon from './Icon.svelte';

  // 只配置一次,避免重复调用累积内存
  if (typeof window !== 'undefined' && !(window as any).__markedConfigured) {
    marked.setOptions({ breaks: true, gfm: true, async: false });
    (window as any).__markedConfigured = true;
  }

  const view = new URLSearchParams(window.location.search).get('view');
  const isSettings = view === 'settings';

  // 仅在划词窗口禁止页面滚动；设置窗口需要正常滚动
  if (!isSettings) {
    document.documentElement.classList.add('popup-window');
  }

  interface ActionItem { name: string; prompt: string; icon: string; enabled: boolean; auto: boolean }

  let selectedText = '';
  let mode: 'toolbar' | 'result' = 'toolbar'; // toolbar=工具条, result=翻译结果
  let resultText = '';
  let resultEndpoint = '';
  let resultModel = '';
  let isLoading = false;
  let loadingStatus = ''; // 加载中的进度提示（故障转移时显示正在请求哪个接口）
  let actions: ActionItem[] = []; // 划词功能项（来自配置）
  let currentAction: ActionItem | null = null;
  $: enabledActions = actions.filter((a) => a.enabled);
  let rootEl: HTMLElement;
  let pinned = false; // 钉住：不随失焦自动关闭
  let copied = false;
  let dragging = false; // 拖动中：临时屏蔽失焦关闭
  let popupAnchor: [number, number] = [0, 0]; // 最近一次划词锚点，切换到结果框时复用以原位重显
  let switching = false; // 工具条→结果框切换中：先隐藏旧帧再显示，期间屏蔽失焦/点外关闭
  let resizeQueued = false;
  let requestSeq = 0;
  let suppressAutoCloseUntil = 0;

  $: renderedHtml = resultText ? sanitizeHtml(marked.parse(resultText) as string) : '';

  function sanitizeHtml(html: string) {
    const template = document.createElement('template');
    template.innerHTML = html;

    template.content
      .querySelectorAll('script, style, iframe, object, embed, link, meta')
      .forEach((el) => el.remove());

    template.content.querySelectorAll('*').forEach((el) => {
      for (const attr of Array.from(el.attributes)) {
        const name = attr.name.toLowerCase();
        const value = attr.value.trim().toLowerCase();
        if (name.startsWith('on') || value.startsWith('javascript:')) {
          el.removeAttribute(attr.name);
        }
      }
    });

    return template.innerHTML;
  }

  // 根据内容把窗口调整为刚好包住内容的大小（交给 Rust 设置，避免前端 DPI 计算出错）
  async function resizeToContent() {
    await tick();
    // 等一帧，确保 WebView 已完成布局
    await new Promise((r) => requestAnimationFrame(() => r(null)));
    if (!rootEl) return;
    // 额外 2px 给透明窗口里的阴影/动画留出余量，避免 WebView 边界裁掉底部。
    const width = Math.ceil(rootEl.offsetWidth) + 2;
    const height = Math.ceil(rootEl.offsetHeight) + 2;
    if (width <= 0 || height <= 0) return;
    try {
      await invoke('resize_popup', { width, height });
    } catch (e) {
      console.error('调整窗口尺寸失败:', e);
    }
  }

  function scheduleResizeToContent() {
    if (resizeQueued) return;
    resizeQueued = true;
    requestAnimationFrame(() => {
      resizeQueued = false;
      resizeToContent();
    });
  }

  function keepPopupOpenFor(ms = 700) {
    suppressAutoCloseUntil = Math.max(suppressAutoCloseUntil, Date.now() + ms);
  }

  function canAutoClose() {
    return !pinned && !dragging && !switching && Date.now() > suppressAutoCloseUntil;
  }

  // 内容变化时重新测量
  $: if (!isSettings && rootEl) {
    void mode;
    void renderedHtml;
    void resultEndpoint;
    void isLoading;
    void loadingStatus;
    scheduleResizeToContent();
  }

  onMount(async () => {
    if (isSettings) return;

    const currentWin = getCurrentWindow();

    // 加载划词功能项，并在设置保存后实时重载
    await loadActions();
    await listen('config-changed', () => {
      loadActions();
    });

    // 故障转移进度：显示正在请求第几个接口
    await listen<{ request_id: number; index: number; total: number; endpoint: string }>('translate-progress', (event) => {
      if (!isLoading) return;
      const p = event.payload;
      if (p.request_id !== requestSeq) return;
      loadingStatus = p.total > 1
        ? `正在请求 ${p.endpoint}（${p.index}/${p.total}）…`
        : `正在请求 ${p.endpoint}…`;
    });

    await listen<[number, number]>('text-selected', async (event) => {
      const selectionSeq = ++requestSeq;
      try {
        const [text, , oldClipboard] = await invoke<[string, boolean, string | null]>('get_selected_text');
        if (selectionSeq !== requestSeq) return;
        if (!text || text.trim().length === 0) return;

        selectedText = text;
        mode = 'toolbar';
        resultText = '';
        resultEndpoint = '';
        resultModel = '';
        pinned = false;

        const [x, y] = event.payload ?? [0, 0];
        popupAnchor = [x, y];
        await invoke('show_popup', { x, y });
        // 显示后再按内容收紧尺寸 + 钳进屏幕
        await resizeToContent();

        // 延迟 300ms 恢复旧剪贴板(等弹窗稳定后,避免触发终端清空选区)
        if (oldClipboard) {
          setTimeout(() => {
            invoke('restore_clipboard', { text: oldClipboard }).catch(() => {});
          }, 300);
        }

        // 若有功能项被标记为「划词后自动执行」，直接运行第一个
        const autoAction = enabledActions.find((a) => a.auto);
        if (autoAction) {
          await doTranslate(autoAction);
        }
      } catch (error) {
        console.error('划词处理失败:', error);
      }
    });

    // 弹窗不抢焦点，靠后端全局点击：点到窗口之外时通知关闭
    await listen('close-popup', () => {
      if (canAutoClose()) closeWindow();
    });

    // 失焦自动关闭（用户与弹窗交互、使其获得焦点后才会触发）；钉住或拖动中时保持
    await currentWin.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        dragging = false;
        return;
      }
      if (canAutoClose()) closeWindow();
    });

    // 拖动结束（松开鼠标）后解除屏蔽
    window.addEventListener('mouseup', () => {
      if (dragging) {
        keepPopupOpenFor(500);
        setTimeout(() => (dragging = false), 350);
      }
    });

    // 按 ESC 关闭
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') closeWindow();
    });
  });

  async function loadActions() {
    try {
      const cfg = await invoke<{ actions?: ActionItem[] }>('load_config');
      actions = cfg.actions ?? [];
    } catch (e) {
      console.error('加载功能项失败:', e);
    }
  }

  async function doTranslate(action: ActionItem) {
    const runSeq = ++requestSeq;
    currentAction = action;
    // 先隐藏弹窗，把「工具条」这一帧从屏幕上彻底擦掉，避免它与结果框在透明窗口上重叠残留(tauri#12800)。
    // 切换期间屏蔽失焦/点外关闭，以免 hide 触发误关。
    switching = true;
    try {
      await invoke('hide_popup');
      if (runSeq !== requestSeq) return;
      mode = 'result';
      isLoading = true;
      loadingStatus = '';
      resultText = '';
      resultEndpoint = '';
      resultModel = '';
      await tick(); // 等 DOM 切到结果框，确保重新显示的是结果框而非旧的工具条
      await invoke('show_popup', { x: popupAnchor[0], y: popupAnchor[1] });
      await resizeToContent();
    } finally {
      if (runSeq === requestSeq) switching = false;
    }

    if (runSeq !== requestSeq) return;

    try {
      const result = await invoke<{ text: string; endpoint_name: string; model: string }>('translate_text', {
        text: selectedText,
        prompt: action.prompt,
        requestId: runSeq,
      });
      if (runSeq !== requestSeq) return;

      // 防止超大响应撑爆渲染(前端保护,后端也有 2MB 限制)
      if (result.text.length > 50000) {
        resultText = `响应内容过大(${result.text.length} 字符),已截断显示前 10000 字符:\n\n${result.text.substring(0, 10000)}\n\n... (已省略 ${result.text.length - 10000} 字符)`;
      } else {
        resultText = result.text;
      }

      resultEndpoint = result.endpoint_name;
      resultModel = result.model;
    } catch (error) {
      if (runSeq !== requestSeq) return;
      resultText = '失败: ' + error;
    } finally {
      if (runSeq === requestSeq) {
        isLoading = false;
        await resizeToContent();
      }
    }
  }

  // 拖动窗口（点在标题栏空白处时）
  function startDrag(e: MouseEvent) {
    if ((e.target as HTMLElement).closest('button')) return;
    keepPopupOpenFor(1200);
    dragging = true;
    getCurrentWindow().startDragging();
  }

  // 钉住=不随失焦/点击外部自动关闭（见 onFocusChanged 与 close-popup 里的 !pinned 判断）。
  // 窗口本身已是 alwaysOnTop（tauri.conf + show_popup 的原生 HWND_TOPMOST），无需再设置；
  // 之前调 setAlwaysOnTop 会激活这个「不抢焦点」的无边框弹窗，与全局鼠标钩子冲突，
  // 导致点击置顶时界面卡死、弹窗消失。这里只切 JS 状态即可。
  function togglePin() {
    pinned = !pinned;
  }

  async function copyResult() {
    try {
      await writeText(resultText);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch (e) {
      console.error('复制失败:', e);
    }
  }

  // 拦截 Markdown 中的链接点击，改用系统浏览器打开，避免 webview 跳转毁掉界面
  function onResultClick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest('a');
    if (a && a.getAttribute('href')) {
      e.preventDefault();
      open(a.getAttribute('href') as string).catch(() => {});
    }
  }

  async function closeWindow() {
    requestSeq++;
    selectedText = '';
    resultText = '';
    resultEndpoint = '';
    resultModel = '';
    currentAction = null;
    mode = 'toolbar';
    isLoading = false;
    loadingStatus = '';
    pinned = false;
    copied = false;
    switching = false;
    dragging = false;
    suppressAutoCloseUntil = 0;
    await invoke('hide_popup');
  }
</script>

{#if isSettings}
  <Settings />
{:else}
  <div class="popup" bind:this={rootEl} on:mousedown={() => keepPopupOpenFor()}>
    {#if mode === 'toolbar'}
      <div class="toolbar">
        {#each enabledActions as action, i}
          {#if i > 0}<div class="divider"></div>{/if}
          <button class="action-btn" on:click={() => doTranslate(action)}>
            <Icon name={action.icon} size={16} />
            {action.name}
          </button>
        {/each}
        <button class="close-btn" on:click={closeWindow} aria-label="关闭">×</button>
      </div>
    {:else}
      <div class="result-card" class:loading={isLoading}>
        <div class="card-head" on:mousedown={startDrag} role="toolbar" tabindex="-1">
          <div class="head-left">
            <span class="badge">
              {currentAction?.name ?? ''}
            </span>
            {#if isLoading}
              <span class="model-info loading-title">{loadingStatus || '正在请求…'}</span>
            {:else if resultEndpoint}
              <span class="model-info">{resultEndpoint} · {resultModel}</span>
            {/if}
          </div>
          <div class="head-actions">
            <button
              class="icon-btn"
              class:active={pinned}
              on:click={togglePin}
              title={pinned ? '取消置顶' : '钉住（保持显示并置顶）'}
              aria-label="置顶"
            >
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M12 17v5" />
                <path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z" />
              </svg>
            </button>
            <button class="icon-btn" on:click={copyResult} title="复制内容" aria-label="复制" disabled={isLoading || !resultText}>
              {#if copied}
                <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M20 6 9 17l-5-5" />
                </svg>
              {:else}
                <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                </svg>
              {/if}
            </button>
            <button class="icon-btn" on:click={closeWindow} title="关闭" aria-label="关闭">
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                <path d="M18 6 6 18M6 6l12 12" />
              </svg>
            </button>
          </div>
        </div>
        {#if isLoading}
          <div class="loading-dots">
            <span></span><span></span><span></span>
          </div>
        {:else}
          <div class="result-text" on:click={onResultClick}>{@html renderedHtml}</div>
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

  /* 内容容器：宽高随内容自适应（窗口被 Rust 设为同样大小） */
  .popup {
    display: inline-block;
    padding: 10px;
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
    padding: 8px 12px 12px;
    background: rgba(252, 252, 253, 0.97);
    backdrop-filter: blur(24px) saturate(180%);
    border: 1px solid rgba(0, 0, 0, 0.1);
    border-radius: 14px;
    box-shadow: 0 3px 12px rgba(0, 0, 0, 0.16);
    box-sizing: border-box;
  }

  .result-card.loading {
    min-height: 66px;
  }

  .card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
    cursor: move;
    user-select: none;
    gap: 10px;
  }

  .head-left {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
  }

  .badge {
    font-size: 11px;
    font-weight: 600;
    color: #4f6bed;
    background: rgba(79, 107, 237, 0.1);
    padding: 2px 8px;
    border-radius: 6px;
    letter-spacing: 0.5px;
    flex-shrink: 0;
  }

  .model-info {
    font-size: 11px;
    color: #8a909c;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 500;
  }

  .loading-title {
    color: #6a707c;
  }

  .head-actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .icon-btn {
    width: 26px;
    height: 26px;
    padding: 0;
    border: none;
    background: transparent;
    color: #6a707c;
    cursor: pointer;
    border-radius: 7px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.15s, color 0.15s;
  }

  .icon-btn:hover:not(:disabled) {
    background: rgba(0, 0, 0, 0.07);
    color: #1a1a1a;
  }

  .icon-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .icon-btn.active {
    background: rgba(79, 107, 237, 0.14);
    color: #4f6bed;
  }

  .result-text {
    margin: 0;
    font-size: 14px;
    line-height: 1.65;
    color: #1a1a1a;
    word-break: break-word;
    overflow-wrap: anywhere;
    max-height: 380px;
    overflow-y: auto;
    overflow-x: hidden;
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

  /* Markdown 渲染样式（@html 内容需用 :global） */
  .result-text :global(p) {
    margin: 0 0 8px;
  }
  .result-text :global(p:last-child) {
    margin-bottom: 0;
  }
  .result-text :global(h1),
  .result-text :global(h2),
  .result-text :global(h3),
  .result-text :global(h4) {
    margin: 10px 0 6px;
    font-size: 15px;
    font-weight: 700;
    line-height: 1.3;
  }
  .result-text :global(h1) {
    font-size: 17px;
  }
  .result-text :global(ul),
  .result-text :global(ol) {
    margin: 4px 0 8px;
    padding-left: 20px;
  }
  .result-text :global(li) {
    margin: 2px 0;
  }
  .result-text :global(code) {
    background: rgba(0, 0, 0, 0.06);
    padding: 1px 5px;
    border-radius: 4px;
    font-family: 'SFMono-Regular', Consolas, monospace;
    font-size: 12.5px;
  }
  .result-text :global(pre) {
    background: rgba(0, 0, 0, 0.05);
    padding: 10px 12px;
    border-radius: 8px;
    overflow-x: auto;
    margin: 6px 0 10px;
  }
  .result-text :global(pre code) {
    background: none;
    padding: 0;
    font-size: 12.5px;
    line-height: 1.5;
  }
  .result-text :global(blockquote) {
    margin: 6px 0;
    padding: 2px 12px;
    border-left: 3px solid rgba(79, 107, 237, 0.4);
    color: #555;
  }
  .result-text :global(a) {
    color: #4f6bed;
    text-decoration: none;
  }
  .result-text :global(a:hover) {
    text-decoration: underline;
  }
  .result-text :global(strong) {
    font-weight: 700;
  }
  .result-text :global(hr) {
    border: none;
    border-top: 1px solid rgba(0, 0, 0, 0.1);
    margin: 10px 0;
  }
  .result-text :global(table) {
    border-collapse: collapse;
    margin: 6px 0;
    font-size: 13px;
  }
  .result-text :global(th),
  .result-text :global(td) {
    border: 1px solid rgba(0, 0, 0, 0.12);
    padding: 4px 8px;
  }

  .loading-dots {
    display: flex;
    gap: 6px;
    justify-content: center;
    align-items: center;
    height: 24px;
    padding: 8px 0 2px;
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
</style>
