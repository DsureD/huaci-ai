use crate::{api_manager, clipboard, config, mouse_hook, translator};
use serde::Serialize;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowPos, ShowWindow, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE,
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

// 故障转移进度：第 index/total 个接口，名为 endpoint
#[derive(Serialize, Clone)]
struct TranslateProgress {
    index: usize,
    total: usize,
    endpoint: String,
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

// 翻译文本（prompt 为功能项的提示词模板，用 {text} 占位）
#[tauri::command]
pub async fn translate_text(
    app: AppHandle,
    text: String,
    prompt: String,
    state: State<'_, AppState>,
) -> Result<TranslateResult, String> {
    let config = state.config.lock().unwrap().clone();

    // 检查文本长度
    if text.trim().len() < config.app.min_text_length {
        return Err("文本太短".to_string());
    }

    // 调用 API（带故障转移），prompt 即所选功能项的提示词模板
    let timeout = Duration::from_secs(config.api.timeout_seconds);

    let (result_text, endpoint) = api_manager::translate_with_failover(
        &text,
        config.api.endpoints.clone(),
        &prompt,
        &config.proxy,
        timeout,
        |index, total, name| {
            // 每尝试一个接口前推送进度，让弹窗显示「正在请求 …（n/N）」
            let _ = app.emit(
                "translate-progress",
                TranslateProgress {
                    index,
                    total,
                    endpoint: name.to_string(),
                },
            );
        },
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
pub fn save_config(
    app: AppHandle,
    new_config: config::Config,
    state: State<'_, AppState>,
) -> Result<(), String> {
    config::save_config(&new_config).map_err(|e| e.to_string())?;
    *state.config.lock().unwrap() = new_config;
    // 通知常驻划词弹窗重新加载功能列表
    let _ = app.emit("config-changed", ());
    // 快捷键可能被改动，重新注册
    apply_hotkeys(&app);
    Ok(())
}

// 按当前配置（重新）注册全局快捷键：先清空再逐个注册，解析失败/冲突的忽略
pub fn apply_hotkeys(app: &AppHandle) {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    let hotkeys = {
        let state = app.state::<AppState>();
        let cfg = state.config.lock().unwrap();
        cfg.hotkeys.clone()
    };

    for spec in [&hotkeys.toggle_capture, &hotkeys.manual_translate] {
        if spec.trim().is_empty() {
            continue;
        }
        if let Ok(sc) = Shortcut::from_str(spec) {
            let _ = gs.register(sc);
        }
    }
}

// 每个快捷键当前是否已成功注册（被其它程序占用/格式错误都会是 false）
#[derive(Serialize)]
pub struct HotkeyStatus {
    toggle: bool,
    manual: bool,
}

// 查询两个快捷键是否已注册，供设置页显示「已生效/未生效」
#[tauri::command]
pub fn get_hotkey_status(app: AppHandle) -> Result<HotkeyStatus, String> {
    let gs = app.global_shortcut();
    let hotkeys = {
        let state = app.state::<AppState>();
        let cfg = state.config.lock().unwrap();
        cfg.hotkeys.clone()
    };
    let is_reg = |spec: &str| -> bool {
        Shortcut::from_str(spec)
            .map(|sc| gs.is_registered(sc))
            .unwrap_or(false)
    };
    Ok(HotkeyStatus {
        toggle: is_reg(&hotkeys.toggle_capture),
        manual: is_reg(&hotkeys.manual_translate),
    })
}

// 全局快捷键被按下时的分发：匹配配置里的两个快捷键
pub fn handle_global_shortcut(app: &AppHandle, shortcut: &Shortcut) {
    let hotkeys = {
        let state = app.state::<AppState>();
        let cfg = state.config.lock().unwrap();
        cfg.hotkeys.clone()
    };

    // 开关划词捕获
    if let Ok(sc) = Shortcut::from_str(&hotkeys.toggle_capture) {
        if shortcut == &sc {
            let new_state = !mouse_hook::is_capture_enabled();
            mouse_hook::set_capture_enabled(new_state);
            crate::sync_capture_state(new_state); // 同步托盘文字 + 图标，让切换可见
            return;
        }
    }

    // 手动翻译：在光标处按划词流程弹出（不受捕获开关限制）
    if let Ok(sc) = Shortcut::from_str(&hotkeys.manual_translate) {
        if shortcut == &sc {
            let _ = app.emit("text-selected", mouse_hook::cursor_pos());
        }
    }
}

// 加载配置
#[tauri::command]
pub fn load_config(state: State<'_, AppState>) -> Result<config::Config, String> {
    Ok(state.config.lock().unwrap().clone())
}

// 切换划词捕获（设置页开关调用）
#[tauri::command]
pub fn toggle_capture(enabled: bool) -> Result<bool, String> {
    mouse_hook::set_capture_enabled(enabled);
    crate::sync_capture_state(enabled); // 同步托盘文字 + 图标
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

        // 不抢占原程序焦点：否则用户选中的文本会失去焦点，无法复制/删除/编辑。
        // 取词已在显示前完成（UIA 优先，必要时回退 Ctrl+C），本窗口无需持有焦点；
        // 关闭改由全局点击驱动（mouse_hook 检测到点窗口外会发 close-popup）。
        #[cfg(target_os = "windows")]
        {
            match win.hwnd() {
                Ok(h) => {
                    mouse_hook::set_popup_hwnd(h.0 as isize);
                    unsafe {
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
                    }
                }
                Err(_) => {
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

        mouse_hook::set_popup_visible(true);
    }
    Ok(())
}

// 隐藏悬浮翻译窗口
#[tauri::command]
pub fn hide_popup(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        mouse_hook::set_popup_visible(false);
        // 用原生 SW_HIDE 隐藏，与 show_popup 的原生显示保持一致，避免可见状态不同步
        #[cfg(target_os = "windows")]
        {
            match win.hwnd() {
                Ok(h) => unsafe {
                    let _ = ShowWindow(HWND(h.0 as _), SW_HIDE);
                },
                Err(_) => {
                    win.hide().map_err(|e| e.to_string())?;
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            win.hide().map_err(|e| e.to_string())?;
        }
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
