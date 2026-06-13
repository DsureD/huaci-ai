# 便携版打包脚本
# 在构建完成后运行此脚本，生成便携版 ZIP

$exePath = "src-tauri\target\release\huaci-ai.exe"
$outputDir = "src-tauri\target\release\bundle\portable"
$zipFile = "src-tauri\target\release\bundle\portable\HuaciAI_0.1.0_x64_Portable.zip"

# 创建输出目录
New-Item -ItemType Directory -Force -Path $outputDir | Out-Null

# 复制文件到临时目录
$tempDir = Join-Path $outputDir "HuaciAI_Portable"
New-Item -ItemType Directory -Force -Path $tempDir | Out-Null

# 复制主程序
Copy-Item $exePath -Destination $tempDir

# 复制配置示例
Copy-Item "config.example.json" -Destination (Join-Path $tempDir "config.json")

# 复制说明文档
Copy-Item "README.md" -Destination $tempDir

# 创建启动说明
@"
# 划词AI - 便携版

## 快速开始

1. 编辑 config.json，填入你的 API Key
2. 双击 huaci-ai.exe 启动程序
3. 程序会最小化到系统托盘（右下角）
4. 在任意应用中选中文本，自动弹出翻译

## 配置说明

config.json 中需要配置：
- api.endpoints[0].api_key - 你的 OpenAI API 密钥
- api.endpoints[0].base_url - API 地址
- api.endpoints[0].model - 模型名称

## 托盘菜单

右键托盘图标：
- 启用/禁用划词监听
- 退出程序

## 注意事项

- 首次运行可能需要安装 WebView2 运行时
- 程序需要全局鼠标钩子权限，请添加到杀毒软件白名单
"@ | Out-File -FilePath (Join-Path $tempDir "使用说明.txt") -Encoding UTF8

# 打包成 ZIP
Compress-Archive -Path $tempDir -DestinationPath $zipFile -Force

Write-Host "✓ 便携版已生成: $zipFile"
