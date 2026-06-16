# 便携版打包脚本
# 在构建完成后运行此脚本，生成便携版 ZIP

$exePath = "src-tauri\target\release\huaci-ai.exe"
$outputDir = "src-tauri\target\release\bundle\portable"
$version = (Get-Content "package.json" -Raw | ConvertFrom-Json).version
$zipFile = Join-Path $outputDir "HuaciAI_${version}_x64_Portable.zip"

# 创建输出目录
New-Item -ItemType Directory -Force -Path $outputDir | Out-Null

# 复制文件到临时目录
$tempDir = Join-Path $outputDir "HuaciAI_Portable"
if (Test-Path $tempDir) {
    Remove-Item $tempDir -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $tempDir | Out-Null

# 复制主程序
Copy-Item $exePath -Destination $tempDir

# 复制默认配置
Copy-Item "config.example.json" -Destination (Join-Path $tempDir "config.json")

# 创建便携版使用说明
@"
# 划词AI - 便携版使用说明

## 快速开始

1. 双击 huaci-ai.exe 启动程序，程序会最小化到系统托盘。
2. 右键托盘图标选择「设置」（或双击托盘图标）。
3. 在设置界面填入 API Key、模型、代理等配置并保存。
4. 在任意应用中选中文本，等待 1-2 秒自动弹出翻译结果。

## 配置说明

便携版配置文件为程序同目录下的 config.json，方便备份和迁移。

常用字段：
- api.endpoints[].base_url - OpenAI 兼容接口地址
- api.endpoints[].api_key - API 密钥
- api.endpoints[].model - 模型名称
- proxy - HTTP 代理配置

## 托盘菜单

右键托盘图标：
- 启用/禁用划词监听（菜单文字随状态变化）
- 设置
- 退出程序

双击托盘图标可直接打开设置。

## 注意事项

- 首次运行可能需要安装 WebView2 运行时
- 程序使用全局鼠标钩子，可能被杀毒软件拦截，请添加信任
- 请妥善保管 config.json，避免泄露 API Key
"@ | Out-File -FilePath (Join-Path $tempDir "使用说明.txt") -Encoding UTF8

# 打包成 ZIP
Compress-Archive -Path (Join-Path $tempDir "*") -DestinationPath $zipFile -Force

Write-Host "✓ 便携版已生成: $zipFile"
