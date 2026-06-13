@echo off
chcp 65001 >nul
echo ====================================
echo   划词AI - 配置检查
echo ====================================
echo.

if not exist "config.json" (
    echo [错误] 未找到 config.json 文件
    echo.
    echo 请将 config.example.json 复制为 config.json 并填入你的 API Key
    echo.
    pause
    exit /b 1
)

echo [✓] 找到配置文件: config.json
echo.

findstr /i "sk-your-api-key-here" config.json >nul
if %errorlevel% == 0 (
    echo [警告] 检测到默认 API Key，请修改为你的真实密钥
    echo.
    echo 编辑 config.json，将 "sk-your-api-key-here" 替换为：
    echo   - OpenAI: sk-xxx...
    echo   - DeepSeek: sk-xxx...
    echo   - 其他兼容接口的密钥
    echo.
    pause
    exit /b 1
)

echo [✓] API Key 已配置
echo.
echo 准备启动程序...
echo 程序将最小化到系统托盘（右下角）
echo.
echo 使用方法：
echo   1. 在任意应用中选中文本
echo   2. 等待 1-2 秒自动弹出翻译
echo   3. 右键托盘图标可开关监听
echo.
pause

start "" "huaci-ai.exe"
