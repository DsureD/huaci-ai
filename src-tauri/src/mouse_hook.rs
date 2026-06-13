use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// 使用 Option<isize> 存储钩子句柄的原始值
static HOOK_HANDLE: Mutex<Option<isize>> = Mutex::new(None);
static APP_HANDLE: Mutex<Option<Arc<AppHandle>>> = Mutex::new(None);
static CAPTURE_ENABLED: Mutex<bool> = Mutex::new(true);

// 鼠标钩子回调函数
unsafe extern "system" fn mouse_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if n_code >= 0 {
        // 检测鼠标左键释放（划词完成）
        if w_param.0 as u32 == WM_LBUTTONUP {
            // 检查是否启用捕获
            if let Ok(enabled) = CAPTURE_ENABLED.lock() {
                if !*enabled {
                    return CallNextHookEx(HHOOK(0), n_code, w_param, l_param);
                }
            }

            // 延迟一小段时间，确保文本已被选中
            std::thread::sleep(std::time::Duration::from_millis(150));

            // 触发获取选中文本事件
            if let Ok(app_guard) = APP_HANDLE.lock() {
                if let Some(app) = app_guard.as_ref() {
                    let _ = app.emit("text-selected", ());
                }
            }
        }
    }

    CallNextHookEx(HHOOK(0), n_code, w_param, l_param)
}

// 安装全局鼠标钩子
pub fn install_mouse_hook(app_handle: AppHandle) -> anyhow::Result<()> {
    unsafe {
        // 保存 AppHandle
        *APP_HANDLE.lock().unwrap() = Some(Arc::new(app_handle));

        let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), HINSTANCE::default(), 0)?;

        // 存储句柄的原始值
        *HOOK_HANDLE.lock().unwrap() = Some(hook.0);

        println!("✓ 全局鼠标钩子已安装");
    }

    Ok(())
}

// 卸载钩子
pub fn uninstall_mouse_hook() -> anyhow::Result<()> {
    if let Some(hook_value) = HOOK_HANDLE.lock().unwrap().take() {
        unsafe {
            UnhookWindowsHookEx(HHOOK(hook_value))?;
            println!("✓ 全局鼠标钩子已卸载");
        }
    }
    Ok(())
}

// 启用/禁用捕获
pub fn set_capture_enabled(enabled: bool) {
    *CAPTURE_ENABLED.lock().unwrap() = enabled;
    println!(
        "✓ 划词捕获已{}",
        if enabled { "启用" } else { "禁用" }
    );
}

// 获取捕获状态
pub fn is_capture_enabled() -> bool {
    *CAPTURE_ENABLED.lock().unwrap()
}
