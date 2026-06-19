# 部署指南

## 📦 上传到 GitHub

### 1. 初始化 Git 仓库（重要）

```bash
# 初始化 Git
git init
git add .
git commit -m "Initial commit: 划词AI翻译工具"
```

**⚠️ 重要：生成 package-lock.json**

GitHub Actions 需要完整的 package-lock.json 才能构建。请在推送前执行：

```bash
# 安装依赖（会生成完整的 package-lock.json）
npm install

# 提交 package-lock.json
git add package-lock.json
git commit -m "Add complete package-lock.json"
```

### 2. 创建 GitHub 仓库

在 GitHub 上创建一个新仓库，然后：

```bash
git remote add origin https://github.com/你的用户名/huaci-ai0613.git
git branch -M main
git push -u origin main
```

## 🚀 自动构建

### 触发构建

GitHub Actions 只在推送 `v*` tag 时自动构建并发布 Release，不再在每次推送 `main` 或 Pull Request 时构建。

发布版本：

```bash
git tag v1.0.5
git push origin v1.0.5
```

### 下载发布文件

1. 进入仓库的 **Releases** 页面
2. 打开最新版本
3. 下载：
   - `HuaciAI_x.x.x_x64_Portable.zip` - 便携版
   - `*.msi` - MSI 安装包
   - `*-setup.exe` - NSIS 安装程序

便携版只包含 `huaci-ai.exe`、`config.json` 和 `使用说明.txt`，不会打包 `启动程序.bat` 或项目 README。

## 🧪 本地测试

如果你想在有 Rust 环境的机器上测试：

### 安装依赖

```bash
npm install
```

### 开发模式运行

```bash
npm run tauri dev
```

### 本地构建

```bash
npm run tauri build
```

构建产物位于：`src-tauri/target/release/bundle/`

## ⚙️ 首次使用配置

### 方式一：便携模式（推荐）

1. 解压 `HuaciAI_x.x.x_x64_Portable.zip`
2. 双击 `huaci-ai.exe` 启动程序
3. 右键托盘图标选择「设置」，填入 API Key、模型等配置并保存
4. 也可以直接编辑程序同目录下的 `config.json`

### 方式二：标准模式

1. 首次运行程序，会在 `%APPDATA%\huaci-ai\` 生成默认配置
2. 编辑配置文件，填入你的 API Key
3. 重启程序

## 🔑 配置 API Key

编辑 `config.json`：

```json
{
  "api": {
    "endpoints": [
      {
        "name": "OpenAI",
        "base_url": "https://api.openai.com/v1",
        "api_key": "sk-你的真实API密钥",
        "model": "gpt-4o-mini",
        "enabled": true,
        "priority": 1
      }
    ]
  }
}
```

## 🐛 常见问题

### 1. 构建失败

**可能原因**：
- Rust 工具链版本过低
- Node.js 版本不匹配
- 网络问题（无法下载依赖）

**解决方法**：
- 查看 GitHub Actions 日志
- 确保使用 Node.js 20+ 和 Rust 1.70+

### 2. 程序无法启动

**可能原因**：
- 缺少 WebView2 运行时
- 配置文件格式错误

**解决方法**：
- 安装 WebView2：https://developer.microsoft.com/microsoft-edge/webview2/
- 检查 `config.json` 是否是有效的 JSON

### 3. 划词无反应

**可能原因**：
- 钩子被杀毒软件拦截
- API Key 配置错误
- 文本太短（小于最小长度）

**解决方法**：
- 将程序添加到杀毒软件白名单
- 检查配置文件中的 API Key
- 调整 `min_text_length` 设置

### 4. 翻译失败

**可能原因**：
- API Key 无效或额度耗尽
- 网络问题
- API 接口地址错误

**解决方法**：
- 验证 API Key 是否有效
- 检查代理设置
- 查看程序日志（开发模式运行）

## 📊 查看日志

开发模式下可以看到详细日志：

```bash
npm run tauri dev
```

日志会显示：
- ✓ 成功信息（绿色）
- ✗ 错误信息（红色）
- API 调用详情

## 🔄 更新程序

1. 下载新版本安装包
2. 运行安装程序（会自动覆盖旧版本）
3. 配置文件会保留

## 📝 配置迁移

如果需要在多台电脑之间同步配置：

1. 复制 `config.json` 文件
2. **注意**：配置文件包含 API Key，请妥善保管
3. 粘贴到新电脑的程序目录或 AppData 目录

---

**需要帮助？** 在 GitHub Issues 中提问！
