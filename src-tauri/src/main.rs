// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api_manager;
mod autostart;
mod clipboard;
mod commands;
mod config;
mod mouse_hook;
mod translator;
mod uia;

use commands::AppState;
use std::sync::Mutex;
use std::sync::OnceLock;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tauri_plugin_global_shortcut::ShortcutState;

// 托盘里的「启用/禁用划词监听」菜单项，供切换捕获后同步文字
pub static TOGGLE_MENU_ITEM: OnceLock<MenuItem<tauri::Wry>> = OnceLock::new();
// 托盘图标句柄，供切换捕获后同步图标（启用=彩色，禁用=灰色）
pub static TRAY_ICON: OnceLock<tauri::tray::TrayIcon<tauri::Wry>> = OnceLock::new();

// 托盘图标资源：编译期内嵌，切换时无需读取运行目录下的外部文件
const ICON_ACTIVE: &[u8] = include_bytes!("../icons/icon.ico");
const ICON_DISABLED: &[u8] = include_bytes!("../icons/icon-gray.ico");

pub fn capture_icon(enabled: bool) -> Option<Image<'static>> {
    let bytes = if enabled { ICON_ACTIVE } else { ICON_DISABLED };
    Image::from_bytes(bytes).ok()
}

// 切换捕获状态后同步托盘：菜单文字 + 图标（托盘 / 快捷键 / 设置页共用这一处逻辑）
pub fn sync_capture_state(app: &tauri::AppHandle, enabled: bool) {
    if let Some(item) = TOGGLE_MENU_ITEM.get() {
        let _ = item.set_text(if enabled {
            "禁用划词监听"
        } else {
            "启用划词监听"
        });
    }
    if let Some(tray) = TRAY_ICON.get() {
        if let Some(icon) = capture_icon(enabled) {
            let _ = tray.set_icon(Some(icon));
        }
    }
    if let Some(win) = app.get_webview_window("settings") {
        if let Some(icon) = capture_icon(enabled) {
            let _ = win.set_icon(icon);
        }
    }
}

fn main() {
    // 加载配置
    let app_config = config::load_config().unwrap_or_else(|e| {
        eprintln!("加载配置失败: {}, 使用默认配置", e);
        config::Config::default()
    });
    if let Err(e) = autostart::set_auto_start(app_config.app.auto_start) {
        eprintln!("同步开机自启失败: {}", e);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    // 低级钩子会同时上报按下/抬起，只在按下时触发一次
                    if event.state() == ShortcutState::Pressed {
                        commands::handle_global_shortcut(app, shortcut);
                    }
                })
                .build(),
        )
        .manage(AppState {
            config: Mutex::new(app_config),
        })
        .setup(|app| {
            // 隐藏主窗口（仅用于划词悬浮翻译）
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }

            // 创建托盘菜单
            let toggle_item =
                MenuItem::with_id(app, "toggle", "禁用划词监听", true, None::<&str>)?;
            let settings_item =
                MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

            let menu =
                Menu::with_items(app, &[&toggle_item, &settings_item, &sep, &quit_item])?;

            // 菜单项存入全局，供托盘 / 快捷键 / 设置页切换捕获时同步文字与图标
            let _ = TOGGLE_MENU_ITEM.set(toggle_item.clone());

            // 创建托盘图标（复用应用图标，避免出现“透明无图标”的托盘）
            let mut tray_builder = TrayIconBuilder::new()
                .tooltip("划词AI")
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "toggle" => {
                        let new_state = !mouse_hook::is_capture_enabled();
                        mouse_hook::set_capture_enabled(new_state);
                        sync_capture_state(app, new_state); // 文字 + 图标一起同步
                    }
                    "settings" => {
                        let _ = commands::open_settings_window(app);
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    // 左键双击托盘打开设置
                    if let TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        let _ = commands::open_settings_window(tray.app_handle());
                    } else if let TrayIconEvent::Click {
                        button: MouseButton::Right,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        // 右键由菜单处理
                    }
                });

            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }

            let tray = tray_builder.build(app)?;
            // 句柄存入全局，供切换捕获时更换图标（启动时捕获默认开启，沿用彩色图标）
            let _ = TRAY_ICON.set(tray);

            // 安装全局鼠标钩子
            if let Err(e) = mouse_hook::install_mouse_hook(app.handle().clone()) {
                eprintln!("安装鼠标钩子失败: {}", e);
            }

            // 按配置注册全局快捷键
            commands::apply_hotkeys(app.handle());

            println!("✓ 划词AI 已启动");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_selected_text,
            commands::restore_clipboard,
            commands::translate_text,
            commands::save_config,
            commands::load_config,
            commands::toggle_capture,
            commands::get_capture_status,
            commands::show_popup,
            commands::hide_popup,
            commands::resize_popup,
            commands::list_models,
            commands::open_settings,
            commands::get_hotkey_status,
        ])
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用时出错");
}
