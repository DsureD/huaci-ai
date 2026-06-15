// 划词功能项可选的通用图标集合（stroke 风格，viewBox="0 0 24 24"）
// path 字段是 <svg> 的内部内容，由 Icon.svelte 套上统一的 svg 外壳渲染
export interface IconDef {
  key: string;
  label: string;
  path: string;
}

export const ICONS: IconDef[] = [
  { key: 'translate', label: '翻译', path: '<path d="M5 8h14M5 12h14M5 16h10"/>' },
  { key: 'explain', label: '解释', path: '<circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3M12 17h.01"/>' },
  { key: 'summarize', label: '总结', path: '<rect x="4" y="3" width="16" height="18" rx="2"/><path d="M8 8h8M8 12h8M8 16h5"/>' },
  { key: 'polish', label: '润色', path: '<path d="M9 4l1.4 3.6L14 9l-3.6 1.4L9 14l-1.4-3.6L4 9l3.6-1.4z"/><path d="M17 13l.8 2.2L20 16l-2.2.8L17 19l-.8-2.2L14 16l2.2-.8z"/>' },
  { key: 'proofread', label: '校对', path: '<path d="M9 11l3 3L22 4"/><path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11"/>' },
  { key: 'expand', label: '扩写', path: '<path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7"/>' },
  { key: 'shorten', label: '缩写', path: '<path d="M4 14h6v6M20 10h-6V4M14 10l7-7M10 14l-7 7"/>' },
  { key: 'rewrite', label: '改写', path: '<path d="M12 20h9"/><path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4z"/>' },
  { key: 'code', label: '代码', path: '<path d="M16 18l6-6-6-6M8 6l-6 6 6 6"/>' },
  { key: 'chat', label: '问答', path: '<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>' },
  { key: 'dictionary', label: '词典', path: '<path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/>' },
  { key: 'keywords', label: '关键词', path: '<path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"/><path d="M7 7h.01"/>' },
  { key: 'idea', label: '灵感', path: '<path d="M9 18h6M10 22h4"/><path d="M12 2a7 7 0 0 0-4 12.7c.6.5 1 1.3 1 2.3h6c0-1 .4-1.8 1-2.3A7 7 0 0 0 12 2z"/>' },
  { key: 'list', label: '列点', path: '<path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/>' },
  { key: 'quote', label: '引用', path: '<path d="M3 21c3 0 7-1 7-8V5a2 2 0 0 0-2-2H4a2 2 0 0 0-2 2v6a2 2 0 0 0 2 2h2M14 21c3 0 7-1 7-8V5a2 2 0 0 0-2-2h-4a2 2 0 0 0-2 2v6a2 2 0 0 0 2 2h2"/>' },
  { key: 'star', label: '收藏', path: '<path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01z"/>' },
];

// 拖拽手柄图标（仅排序手柄用，不在功能项可选列表里）
export const DRAG_ICON =
  '<path d="M8 6h.01M8 12h.01M8 18h.01M16 6h.01M16 12h.01M16 18h.01"/>';

const ICON_MAP: Record<string, string> = {};
for (const i of ICONS) ICON_MAP[i.key] = i.path;

// 取图标 path；'drag' 取手柄图标；未知 key 回退到通用「星」图标
export function iconPath(key: string): string {
  if (key === 'drag') return DRAG_ICON;
  return ICON_MAP[key] ?? ICON_MAP['star'];
}
