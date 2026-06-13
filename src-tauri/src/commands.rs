use crate::{api_manager, clipboard, config, mouse_hook};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindowBuilder};

// 全局配置状态
pub struct AppState {
    pub config: Mutex<config::Config>,
}

// 获取选中文本及自动翻译开关状态
#[tauri::command]
pub fn get_selected_text(app: AppHandle, state: State<'_, AppState>) -> Result<(String, bool), String> {
    let text = clipboard::get_selected_text(&app).map_err(|e| e.to_string())?;
    let auto_translate = state.config.lock().unwrap().app.auto_translate;
    Ok((text, auto_translate))
}

// 翻译文本
#[tauri::command]
pub async fn translate_text(
    text: String,
    mode: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let config = state.config.lock().unwrap().clone();

    // 检查文本长度
    if text.trim().len() < config.app.min_text_length {
        return Err("文本太短".to_string());
    }

    // 选择提示词模板
    let prompt_template = match mode.as_str() {
        "translate" => &config.prompts.translate,
        "explain" => &config.prompts.explain,
        _ => &config.prompts.translate,
    };

    // 调用 API（带故障转移）
    let timeout = Duration::from_secs(config.api.timeout_seconds);

    api_manager::translate_with_failover(
        &text,
        config.api.endpoints.clone(),
        prompt_template,
        &config.proxy,
        timeout,
    )
    .await
    .map_err(|e| e.to_string())
}

// 保存配置
#[tauri::command]
pub fn save_config(new_config: config::Config, state: State<'_, AppState>) -> Result<(), String> {
    config::save_config(&new_config).map_err(|e| e.to_string())?;
    *state.config.lock().unwrap() = new_config;
    Ok(())
}

// 加载配置
#[tauri::command]
pub fn load_config(state: State<'_, AppState>) -> Result<config::Config, String> {
    Ok(state.config.lock().unwrap().clone())
}

// 切换划词捕获
#[tauri::command]
pub fn toggle_capture(enabled: bool) -> Result<bool, String> {
    mouse_hook::set_capture_enabled(enabled);
    Ok(enabled)
}

// 获取捕获状态
#[tauri::command]
pub fn get_capture_status() -> Result<bool, String> {
    Ok(mouse_hook::is_capture_enabled())
}

// 在指定屏幕坐标附近显示悬浮翻译窗口
#[tauri::command]
pub fn show_popup(app: AppHandle, x: i32, y: i32) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        // 让窗口出现在鼠标右下方，避免遮住选中文字
        let _ = win.set_position(PhysicalPosition::new(x + 12, y + 12));
        win.show().map_err(|e| e.to_string())?;
        let _ = win.set_always_on_top(true);
        let _ = win.set_focus();
    }
    Ok(())
}

// 隐藏悬浮翻译窗口
#[tauri::command]
pub fn hide_popup(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        win.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

// 打开（或聚焦）设置窗口
#[tauri::command]
pub fn open_settings(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(());
    }

    WebviewWindowBuilder::new(
        &app,
        "settings",
        WebviewUrl::App("index.html?view=settings".into()),
    )
    .title("划词AI - 设置")
    .inner_size(560.0, 640.0)
    .resizable(true)
    .center()
    .build()
    .map_err(|e| e.to_string())?;
    Ok(())
}
