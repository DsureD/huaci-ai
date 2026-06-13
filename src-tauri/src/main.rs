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
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
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
            // 创建托盘菜单
            let toggle_item = MenuItem::with_id(app, "toggle", "启用划词监听", true, None::<&str>)?;
            let settings_item = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&toggle_item, &settings_item, &quit_item])?;

            // 创建托盘图标
            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "toggle" => {
                        let current = mouse_hook::is_capture_enabled();
                        mouse_hook::set_capture_enabled(!current);
                        println!("划词监听: {}", if !current { "已启用" } else { "已禁用" });
                    }
                    "settings" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        let _ = mouse_hook::uninstall_mouse_hook();
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|_tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        println!("托盘图标被点击");
                    }
                })
                .build(app)?;

            // 安装全局鼠标钩子
            if let Err(e) = mouse_hook::install_mouse_hook(app.handle().clone()) {
                eprintln!("安装鼠标钩子失败: {}", e);
            }

            println!("✓ 划词AI 已启动");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_selected_text,
            commands::translate_text,
            commands::save_config,
            commands::load_config,
            commands::toggle_capture,
            commands::get_capture_status,
        ])
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用时出错");
}
