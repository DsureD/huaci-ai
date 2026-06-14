// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api_manager;
mod clipboard;
mod commands;
mod config;
mod mouse_hook;
mod translator;

use commands::AppState;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WebviewUrl, WebviewWindowBuilder,
};

fn main() {
    // 加载配置
    let app_config = config::load_config().unwrap_or_else(|e| {
        eprintln!("加载配置失败: {}, 使用默认配置", e);
        config::Config::default()
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_shell::init())
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

            // 把可变菜单项交给闭包持有，便于动态更新文字
            let toggle_handle = toggle_item.clone();

            // 创建托盘图标（复用应用图标，避免出现“透明无图标”的托盘）
            let mut tray_builder = TrayIconBuilder::new()
                .tooltip("划词AI")
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "toggle" => {
                        let current = mouse_hook::is_capture_enabled();
                        mouse_hook::set_capture_enabled(!current);
                        // 更新菜单文字
                        let _ = toggle_handle.set_text(if !current {
                            "禁用划词监听"
                        } else {
                            "启用划词监听"
                        });
                    }
                    "settings" => {
                        open_settings_window(app);
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
                        open_settings_window(tray.app_handle());
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

            let _tray = tray_builder.build(app)?;

            // 安装全局鼠标钩子
            if let Err(e) = mouse_hook::install_mouse_hook(app.handle().clone()) {
                eprintln!("安装鼠标钩子失败: {}", e);
            }

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
        ])
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用时出错");
}

// 打开（或聚焦）设置窗口
fn open_settings_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }

    let _ = WebviewWindowBuilder::new(
        app,
        "settings",
        WebviewUrl::App("index.html?view=settings".into()),
    )
    .title("划词AI - 设置")
    .inner_size(560.0, 640.0)
    .resizable(true)
    .center()
    .build();
}
