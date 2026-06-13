# 图标文件说明

由于无法在纯文本环境中生成二进制图标文件，请按以下步骤添加图标：

## 方法一：使用 Tauri 默认图标（推荐）

在 `src-tauri` 目录下运行：

```bash
# 这会生成默认图标
cargo tauri icon path/to/your-icon.png
```

如果没有自己的图标，可以下载一个临时图标：
https://icon-icons.com/icon/translate/154400

## 方法二：手动创建占位图标

创建 `src-tauri/icons/icon.ico` 文件（32x32 像素的简单图标）

## 方法三：禁用图标要求（临时方案）

tauri.conf.json 已更新为不需要图标文件，可以直接构建。

## 当前状态

配置已更新，构建时会使用 Tauri 的默认占位图标。

如果需要自定义图标，请在本地准备一个 PNG 图标（至少 512x512px），然后运行：

```bash
npm run tauri icon your-icon.png
```

这会自动生成所有需要的图标尺寸。
