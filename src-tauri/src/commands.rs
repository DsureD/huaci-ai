use crate::{api_manager, clipboard, config, mouse_hook, translator};
use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindowBuilder};
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowPos, ShowWindow, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SW_SHOWNOACTIVATE,
};

// 全局配置状态
pub struct AppState {
    pub config: Mutex<config::Config>,
}

#[derive(Serialize)]
pub struct TranslateResult {
    text: String,
    endpoint_name: String,
    model: String,
}

// 获取选中文本及自动翻译开关状态,返回 (文本, 自动翻译, 旧剪贴板)
#[tauri::command]
pub fn get_selected_text(app: AppHandle, state: State<'_, AppState>) -> Result<(String, bool, Option<String>), String> {
    let (text, old_clipboard) = clipboard::get_selected_text(&app).map_err(|e| e.to_string())?;
    let auto_translate = state.config.lock().unwrap().app.auto_translate;
    Ok((text, auto_translate, old_clipboard))
}

// 恢复剪贴板内容(延迟恢复,避免触发终端清空选区)
#[tauri::command]
pub fn restore_clipboard(app: AppHandle, text: Option<String>) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    if let Some(t) = text {
        app.clipboard().write_text(t).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// 翻译文本
#[tauri::command]
pub async fn translate_text(
    text: String,
    mode: String,
    state: State<'_, AppState>,
) -> Result<TranslateResult, String> {
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

    let (result_text, endpoint) = api_manager::translate_with_failover(
        &text,
        config.api.endpoints.clone(),
        prompt_template,
        &config.proxy,
        timeout,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(TranslateResult {
        text: result_text,
        endpoint_name: endpoint.name,
        model: endpoint.model,
    })
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

        // 关键：以“不激活”方式显示，不抢占原应用焦点。
        // 否则弹窗一弹出就夺走焦点，用户在原程序里按 Ctrl+C 会发到弹窗而非原程序，
        // 导致“划词后无法复制”。不抢焦点后，关闭逻辑改由全局点击驱动（见 mouse_hook）。
        #[cfg(target_os = "windows")]
        {
            match win.hwnd() {
                Ok(h) => unsafe {
                    let hwnd = HWND(h.0 as _);
                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                    // 置顶但不激活
                    let _ = SetWindowPos(
                        hwnd,
                        HWND_TOPMOST,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                },
                Err(_) => {
                    // 拿不到原生句柄时退化为普通显示
                    win.show().map_err(|e| e.to_string())?;
                    let _ = win.set_always_on_top(true);
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            win.show().map_err(|e| e.to_string())?;
            let _ = win.set_always_on_top(true);
        }
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

// 根据内容把悬浮窗调整为合适大小（逻辑像素），消除多余透明空白，
// 并保证窗口完整落在显示器工作区内（屏幕底部/右侧自动上移、左移）
#[tauri::command]
pub fn resize_popup(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        let w = width.clamp(80.0, 900.0);
        let h = height.clamp(40.0, 700.0);
        win.set_size(LogicalSize::new(w, h)).map_err(|e| e.to_string())?;

        // 把窗口钳制进当前显示器的工作区（排除任务栏）
        if let Ok(Some(monitor)) = win.current_monitor() {
            let area = monitor.work_area();
            let ap = area.position;
            let asz = area.size;
            if let (Ok(win_size), Ok(pos)) = (win.outer_size(), win.outer_position()) {
                let min_x = ap.x;
                let min_y = ap.y;
                let max_x = (ap.x + asz.width as i32 - win_size.width as i32).max(min_x);
                let max_y = (ap.y + asz.height as i32 - win_size.height as i32).max(min_y);
                let nx = pos.x.clamp(min_x, max_x);
                let ny = pos.y.clamp(min_y, max_y);
                if nx != pos.x || ny != pos.y {
                    let _ = win.set_position(PhysicalPosition::new(nx, ny));
                }
            }
        }
    }
    Ok(())
}

// 查询某接口可用模型列表
#[tauri::command]
pub async fn list_models(
    base_url: String,
    api_key: String,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let (proxy, timeout) = {
        let cfg = state.config.lock().unwrap();
        (
            cfg.proxy.clone(),
            Duration::from_secs(cfg.api.timeout_seconds),
        )
    };

    translator::list_models(&base_url, &api_key, &proxy, timeout)
        .await
        .map_err(|e| e.to_string())
}

// 打开（或聚焦）设置窗口
#[tauri::command]
pub fn open_settings(app: AppHandle) -> Result<(), String> {
    // 先尝试销毁旧窗口(如果存在但已无效),避免累积
    if let Some(win) = app.get_webview_window("settings") {
        // 检查窗口是否仍然有效(可见或可聚焦)
        if win.is_visible().unwrap_or(false) || win.set_focus().is_ok() {
            let _ = win.show();
            let _ = win.set_focus();
            return Ok(());
        } else {
            // 窗口引用存在但已失效,销毁它
            let _ = win.close();
        }
    }

    let win = WebviewWindowBuilder::new(
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

    // 监听窗口关闭事件,主动销毁以释放资源
    let _ = win.on_window_event(|event| {
        if let tauri::WindowEvent::CloseRequested { .. } = event {
            // 用户点击关闭时,确保窗口被完全销毁
        }
    });

    Ok(())
}
