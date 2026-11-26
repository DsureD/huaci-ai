use crate::{api_manager, clipboard, config, logger, mouse_hook, translator};
use serde::Serialize;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, State, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    IsIconic, IsWindow, SetForegroundWindow, SetWindowPos, ShowWindow, HWND_TOPMOST,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_RESTORE, SW_SHOWNOACTIVATE,
};

// 串行化设置窗口的打开/重建，防止快速双击托盘或并发调用造成竞态
static SETTINGS_WINDOW_LOCK: Mutex<()> = Mutex::new(());
// 串行化悬浮窗的重建，防止并发重建冲突
static MAIN_WINDOW_LOCK: Mutex<()> = Mutex::new(());
// 主悬浮窗（main）前端最近一次心跳时间（毫秒）；用于识别长时间后台后 WebView2 静默死亡
static MAIN_HEARTBEAT_MS: AtomicU64 = AtomicU64::new(0);
static MAIN_REBUILDING: AtomicBool = AtomicBool::new(false);
// 当需要重建被挂起/失联的主窗口后，缓存待处理的划词锚点，前端就绪后补发
static PENDING_SELECTION: Mutex<Option<(i32, i32)>> = Mutex::new(None);
// 与前端心跳间隔 30s 配套：超过 120s 无心跳且已经收到过心跳，视为渲染进程疑似挂死
const MAIN_HEARTBEAT_STALE_MS: u64 = 120_000;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// 统一加锁读取配置，防止意外 panic 导致 Mutex 中毒后永久崩盘
fn lock_config(state: &AppState) -> std::sync::MutexGuard<'_, config::Config> {
    match state.config.lock() {
        Ok(g) => g,
        Err(poisoned) => {
            logger::log_warn("配置锁发生 poisoned，已自动恢复");
            poisoned.into_inner()
        }
    }
}

// 判断给定 WebviewWindow 的底层原生窗口句柄是否依然有效
fn is_window_handle_alive(win: &WebviewWindow) -> bool {
    #[cfg(target_os = "windows")]
    {
        match win.hwnd() {
            Ok(h) if !h.0.is_null() => unsafe { IsWindow(HWND(h.0 as _)).as_bool() },
            _ => false,
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        win.is_visible().is_ok()
    }
}

// 销毁指定窗口并等待 Tauri 释放 label（避免立刻用同 label 重建报 "already exists"）
fn destroy_and_wait_label_free(app: &AppHandle, label: &str) {
    if let Some(win) = app.get_webview_window(label) {
        if let Err(e) = win.destroy() {
            logger::log_warn(&format!("destroy 窗口 {} 失败: {}", label, e));
            let _ = win.close();
        }
    }
    for _ in 0..40 {
        if app.get_webview_window(label).is_none() {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    logger::log_warn(&format!("等待窗口 {} 释放超时", label));
}

// 将一个已存在的设置窗口可靠地带到前台（兼容隐藏 / 最小化 / 失焦状态）
fn focus_existing_window(win: &WebviewWindow) -> bool {
    if !is_window_handle_alive(win) {
        return false;
    }
    if win.is_minimized().unwrap_or(false) {
        let _ = win.unminimize();
    }
    if win.show().is_err() {
        return false;
    }
    let _ = win.unminimize();
    let _ = win.set_focus();

    #[cfg(target_os = "windows")]
    if let Ok(h) = win.hwnd() {
        unsafe {
            let hwnd = HWND(h.0 as _);
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
        }
    }

    true
}

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
    request_id: u64,
    index: usize,
    total: usize,
    endpoint: String,
}

// 获取选中文本及自动翻译开关状态,返回 (文本, 自动翻译, 旧剪贴板, 取词后的剪贴板序号)
// 设为 async + spawn_blocking：避免 UIA/剪贴板轮询在 Windows 上阻塞 Tauri 同步 IPC 线程
#[tauri::command]
pub async fn get_selected_text(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(String, bool, Option<String>, Option<u32>), String> {
    let app_config = lock_config(&state).app.clone();
    let app_clone = app.clone();
    let fallback_enabled = app_config.clipboard_fallback_enabled;
    let (text, old_clipboard, clipboard_sequence) = tauri::async_runtime::spawn_blocking(move || {
        clipboard::get_selected_text(&app_clone, fallback_enabled)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    let auto_translate = app_config.auto_translate;
    Ok((text, auto_translate, old_clipboard, clipboard_sequence))
}

// 恢复剪贴板内容(延迟恢复,避免触发终端清空选区)。
// 若取词后用户又手动 Ctrl+C，剪贴板序号会变化，此时跳过恢复，避免覆盖用户新复制的内容。
#[tauri::command]
pub fn restore_clipboard(
    app: AppHandle,
    text: Option<String>,
    sequence: Option<u32>,
) -> Result<bool, String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    if let Some(expected) = sequence {
        if clipboard::clipboard_sequence_number() != expected {
            return Ok(false);
        }
    }
    if let Some(t) = text {
        app.clipboard().write_text(t).map_err(|e| e.to_string())?;
    } else {
        app.clipboard()
            .write_text(String::new())
            .map_err(|e| e.to_string())?;
    }
    Ok(true)
}

// 翻译文本（prompt 为功能项的提示词模板，用 {text} 占位）
#[tauri::command]
pub async fn translate_text(
    app: AppHandle,
    text: String,
    prompt: String,
    request_id: u64,
    state: State<'_, AppState>,
) -> Result<TranslateResult, String> {
    let config = lock_config(&state).clone();

    // 检查文本长度（按字符数统计，与设置页“最小字符数”一致，避免中文被按字节算成 3 倍）
    if text.trim().chars().count() < config.app.min_text_length {
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
                    request_id,
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
    crate::autostart::set_auto_start(new_config.app.auto_start).map_err(|e| e.to_string())?;
    config::save_config(&new_config).map_err(|e| e.to_string())?;
    *lock_config(&state) = new_config;
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
        let cfg = lock_config(&state);
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
        let cfg = lock_config(&state);
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
        let cfg = lock_config(&state);
        cfg.hotkeys.clone()
    };

    // 开关划词捕获
    if let Ok(sc) = Shortcut::from_str(&hotkeys.toggle_capture) {
        if shortcut == &sc {
            let new_state = !mouse_hook::is_capture_enabled();
            mouse_hook::set_capture_enabled(new_state);
            crate::sync_capture_state(app, new_state); // 同步托盘/窗口文字与图标，让切换可见
            return;
        }
    }

    // 手动翻译：在光标处按划词流程弹出（不受捕获开关限制）
    if let Ok(sc) = Shortcut::from_str(&hotkeys.manual_translate) {
        if shortcut == &sc {
            let app_clone = app.clone();
            let pos = mouse_hook::cursor_pos();
            std::thread::spawn(move || {
                trigger_text_selected(&app_clone, pos);
            });
        }
    }
}

// 加载配置
#[tauri::command]
pub fn load_config(state: State<'_, AppState>) -> Result<config::Config, String> {
    Ok(lock_config(&state).clone())
}

// 切换划词捕获（设置页开关调用）
#[tauri::command]
pub fn toggle_capture(app: AppHandle, enabled: bool) -> Result<bool, String> {
    mouse_hook::set_capture_enabled(enabled);
    crate::sync_capture_state(&app, enabled); // 同步托盘/窗口文字与图标
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

// 兼容：从托盘强制恢复划词监听与窗口状态（无需完全重启进程即可自愈）
pub fn repair_runtime_async(app: &AppHandle) {
    let app_handle = app.clone();
    std::thread::spawn(move || {
        logger::log_info("用户触发托盘修复：重装鼠标钩子、重新注册快捷键并重建悬浮窗");
        mouse_hook::request_hook_reinstall();
        apply_hotkeys(&app_handle);
        if let Err(e) = rebuild_main_window_sync(&app_handle) {
            logger::log_error(&format!("修复时重建划词主窗口失败: {}", e));
        }
    });
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
        let cfg = lock_config(&state);
        (
            cfg.proxy.clone(),
            Duration::from_secs(cfg.api.timeout_seconds),
        )
    };

    translator::list_models(&base_url, &api_key, &proxy, timeout)
        .await
        .map_err(|e| e.to_string())
}

// 主划词窗口前端定时上报心跳，证明 WebView2 渲染进程存活；
// 如果之前因 WebView2 失联触发了重建，在前端就绪后补发那次划词坐标。
#[tauri::command]
pub fn popup_heartbeat(app: AppHandle) -> Result<(), String> {
    MAIN_HEARTBEAT_MS.store(now_ms(), Ordering::Relaxed);
    let pending = PENDING_SELECTION.lock().ok().and_then(|mut g| g.take());
    if let Some(pos) = pending {
        let _ = app.emit_to("main", "text-selected", pos);
    }
    Ok(())
}

// 供鼠标钩子 / 全局快捷键调用：触发划词前先校验 main 窗口是否存活，
// 长时间后台静默后若 WebView2 已异常退出或长期无心跳，自动重建 main 窗口恢复划词。
pub fn trigger_text_selected(app: &AppHandle, pos: (i32, i32)) {
    let main_alive = match app.get_webview_window("main") {
        Some(win) => {
            if !is_window_handle_alive(&win) {
                false
            } else {
                let last_hb = MAIN_HEARTBEAT_MS.load(Ordering::Relaxed);
                // 收到过心跳但超时很久未更新，说明 WebView2 渲染进程已挂起或被系统回收
                !(last_hb > 0 && now_ms().saturating_sub(last_hb) > MAIN_HEARTBEAT_STALE_MS)
            }
        }
        None => false,
    };

    if main_alive {
        if let Err(e) = app.emit_to("main", "text-selected", pos) {
            logger::log_warn(&format!("发送 text-selected 失败，准备重建主窗口: {}", e));
            rebuild_main_window_async(app, Some(pos));
        }
    } else {
        logger::log_warn("检测到划词悬浮窗(main)已失效或 WebView2 失联，正在自动重建...");
        rebuild_main_window_async(app, Some(pos));
    }
}

fn rebuild_main_window_async(app: &AppHandle, pending_pos: Option<(i32, i32)>) {
    if let Some(pos) = pending_pos {
        if let Ok(mut g) = PENDING_SELECTION.lock() {
            *g = Some(pos);
        }
    }
    if MAIN_REBUILDING.swap(true, Ordering::SeqCst) {
        return;
    }

    let app_handle = app.clone();
    std::thread::spawn(move || {
        let result = rebuild_main_window_sync(&app_handle);
        MAIN_REBUILDING.store(false, Ordering::SeqCst);
        if let Err(e) = result {
            logger::log_error(&format!("重建划词主窗口失败: {}", e));
        }
    });
}

fn rebuild_main_window_sync(app: &AppHandle) -> Result<(), String> {
    let _guard = match MAIN_WINDOW_LOCK.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };

    mouse_hook::set_popup_visible(false);
    mouse_hook::set_popup_hwnd(0);
    MAIN_HEARTBEAT_MS.store(0, Ordering::Relaxed);

    destroy_and_wait_label_free(app, "main");

    let win = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("划词AI")
        .inner_size(200.0, 60.0)
        .resizable(false)
        .fullscreen(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?;

    let win_clone = win.clone();
    win.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = win_clone.hide();
        }
    });

    let _ = win.hide();
    logger::log_info("✓ 划词主窗口(main)已重建完成");
    Ok(())
}

// 打开（或聚焦）设置窗口：设为 async 命令，避免在 Windows WebView2 IPC 回调线程直接 build() 触发死锁
#[tauri::command]
pub async fn open_settings(app: AppHandle) -> Result<(), String> {
    let app_clone = app.clone();
    tauri::async_runtime::spawn_blocking(move || open_settings_window_sync(&app_clone))
        .await
        .map_err(|e| e.to_string())?
}

// 供托盘菜单/双击事件调用：脱离主事件循环线程异步执行，避免 Windows 下同步建窗死锁与错误被静默吞掉
pub fn open_settings_window_async(app: &AppHandle) {
    let app_handle = app.clone();
    std::thread::spawn(move || {
        if let Err(e) = open_settings_window_sync(&app_handle) {
            logger::log_error(&format!("打开设置窗口失败: {}", e));
        }
    });
}

fn open_settings_window_sync(app: &AppHandle) -> Result<(), String> {
    let _guard = match SETTINGS_WINDOW_LOCK.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };

    // 1. 优先复用已存在的设置窗口（关闭时隐藏而不销毁，彻底避开反复销毁/重建 WebView2 导致的故障）
    if let Some(win) = app.get_webview_window("settings") {
        if focus_existing_window(&win) {
            if let Some(icon) = crate::capture_icon(mouse_hook::is_capture_enabled()) {
                let _ = win.set_icon(icon);
            }
            // 通知设置页刷新最新配置、快捷键和监听状态
            let _ = app.emit_to("settings", "settings-refresh", ());
            return Ok(());
        }

        // 窗口对象仍在注册表里但底层 HWND 已失效（例如 WebView2 异常退出），先彻底销毁并等 label 释放
        logger::log_warn("检测到设置窗口句柄已失效，正在销毁并重建...");
        destroy_and_wait_label_free(app, "settings");
    }

    // 2. 构建设置窗口；如果因为极端竞态导致 label 仍未释放，最多重试一次
    let win = match build_settings_window(app) {
        Ok(w) => w,
        Err(first_err) => {
            logger::log_warn(&format!("首次创建设置窗口失败({}), 清理后重试", first_err));
            destroy_and_wait_label_free(app, "settings");
            build_settings_window(app).map_err(|second_err| {
                format!("创建设置窗口失败: {} (重试仍失败: {})", first_err, second_err)
            })?
        }
    };

    if let Some(icon) = crate::capture_icon(mouse_hook::is_capture_enabled()) {
        let _ = win.set_icon(icon);
    }

    // 拦截关闭事件：改为隐藏而非销毁，避免长期反复创建/销毁 WebView2 实例引发卡死或打不开
    let win_for_event = win.clone();
    win.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = win_for_event.hide();
        }
    });

    let _ = focus_existing_window(&win);
    logger::log_info("✓ 设置窗口已打开");
    Ok(())
}

fn build_settings_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    WebviewWindowBuilder::new(
        app,
        "settings",
        WebviewUrl::App("index.html?view=settings".into()),
    )
    .title("划词AI - 设置")
    .inner_size(560.0, 640.0)
    .min_inner_size(480.0, 520.0)
    .resizable(true)
    .center()
    .build()
    .map_err(|e| e.to_string())
}
