# 划词AI - 轻量级划词翻译工具

一个基于 Tauri + Rust 构建的轻量级 Windows 划词翻译工具，支持自定义 AI 大模型接口。

## ✨ 功能特性

- 🖱️ **划词自动翻译** - 鼠标选中文本后自动触发翻译
- 🔄 **多接口故障转移** - 支持配置多个 API 接口，自动切换
- 🌐 **代理支持** - 支持 HTTP 代理配置
- 🎨 **悬浮窗显示** - 翻译结果以悬浮窗形式展示
- 📋 **剪贴板保护** - 自动保存和恢复原剪贴板内容
- 💾 **便携模式** - 支持配置文件放在程序目录
- 🔧 **高度可配置** - 自定义 API、提示词、快捷键等

## 🚀 快速开始

### 下载安装

前往 [Releases](https://github.com/你的用户名/huaci-ai0613/releases) 页面下载最新版本：

- **`HuaciAI_x.x.x_x64_Portable.zip`** - 便携版（推荐，免安装）
- `huaci-ai_x.x.x_x64_en-US.msi` - MSI 安装包
- `huaci-ai_x.x.x_x64-setup.exe` - NSIS 安装程序

**推荐使用便携版**：解压即用，配置文件在同目录下，方便备份和迁移。

### 配置 API

**便携版**：编辑程序目录下的 `config.json`

**安装版**：首次运行会在以下位置生成配置文件：
- `C:\Users\你的用户名\AppData\Roaming\huaci-ai\config.json`

编辑配置文件，填入你的 API 信息：

```json
{
  "api": {
    "timeout_seconds": 30,
    "endpoints": [
      {
        "name": "OpenAI",
        "base_url": "https://api.openai.com/v1",
        "api_key": "sk-你的API密钥",
        "model": "gpt-4o-mini",
        "enabled": true,
        "priority": 1
      },
      {
        "name": "备用接口",
        "base_url": "https://你的备用接口/v1",
        "api_key": "sk-备用密钥",
        "model": "llama-3.1-70b",
        "enabled": true,
        "priority": 2
      }
    ]
  }
}
```

## 📖 使用方法

1. **启动程序** - 双击 `huaci-ai.exe`，程序会最小化到系统托盘（任务栏右下角）
2. **配置 API** - 首次使用前，编辑 `config.json` 填入你的 API Key
3. **划词翻译** - 在任意应用中用鼠标选中文本，等待 1-2 秒自动弹出翻译结果
4. **托盘菜单** - 右键托盘图标可以：
   - 启用/禁用划词监听
   - 退出程序

**提示**：如果没有反应，请检查：
- 是否已配置有效的 API Key
- 选中的文本是否太短（默认最少 1 个字符）
- 程序是否被杀毒软件拦截

## ⚙️ 配置说明

### 应用设置

```json
{
  "app": {
    "auto_translate": true,        // 是否自动翻译
    "min_text_length": 1           // 最小文本长度
  }
}
```

### 快捷键

```json
{
  "hotkeys": {
    "toggle_capture": "Ctrl+Shift+H",  // 切换划词监听
    "manual_translate": "Ctrl+Q"        // 手动翻译
  }
}
```

### 代理设置

```json
{
  "proxy": {
    "enabled": false,              // 是否启用代理
    "host": "127.0.0.1",          // 代理主机
    "port": 7890                   // 代理端口
  }
}
```

### 提示词模板

```json
{
  "prompts": {
    "translate": "你是专业的翻译助手。将以下文本翻译成中文：\n\n{text}",
    "explain": "请简洁地解释以下内容：\n\n{text}"
  }
}
```

## 🛠️ 本地开发

### 环境要求

- Node.js 20+
- Rust 1.70+
- Windows 10/11

### 首次初始化（推送到 GitHub 前）

如果你要推送到 GitHub 让 Actions 自动构建，先生成完整的 package-lock.json：

```bash
# 安装依赖（会生成完整的 package-lock.json）
npm install

# 提交更新后的 package-lock.json
git add package-lock.json
git commit -m "Add package-lock.json"
git push
```

### 安装依赖

```bash
npm install
```

### 开发模式

```bash
npm run tauri dev
```

### 构建

```bash
npm run tauri build
```

## 📦 技术栈

- **前端**: Svelte 5 + TypeScript + Vite
- **后端**: Rust + Tauri 2.0
- **Windows API**: windows-rs
- **HTTP 客户端**: reqwest

## ⚠️ 注意事项

1. **WebView2** - 需要 Windows 10 1809+ 或 Windows 11（内置 WebView2）
2. **权限** - 程序使用全局鼠标钩子，可能被杀毒软件拦截，请添加信任
3. **兼容性** - 部分应用（如密码管理器）可能无法捕获选中文本
4. **API Key 安全** - 请妥善保管配置文件，不要分享给他人

## 🐛 已知问题

- 某些使用自定义渲染的应用（如 Chrome Canvas）可能无法捕获文本
- 高 DPI 显示器可能需要调整窗口大小

## 📝 更新日志

### v0.1.0 (2026-06-13)

- 🎉 首次发布
- ✅ 基础划词翻译功能
- ✅ 多 API 接口支持
- ✅ 故障转移机制
- ✅ 系统托盘

## 📄 许可证

MIT License

## 🙏 致谢

- [Tauri](https://tauri.app/) - 跨平台桌面应用框架
- [Svelte](https://svelte.dev/) - 轻量级前端框架
- [windows-rs](https://github.com/microsoft/windows-rs) - Windows API 绑定

---

**Star ⭐ 这个项目如果你觉得有用！**
