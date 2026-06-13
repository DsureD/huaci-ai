use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// 是否启用划词捕获（原子量，可跨线程安全读写）
static CAPTURE_ENABLED: AtomicBool = AtomicBool::new(true);
// 钩子线程向工作线程发送鼠标坐标的通道
static SIGNAL_TX: Mutex<Option<Sender<(i32, i32)>>> = Mutex::new(None);

// 鼠标钩子回调：必须立刻返回，绝不能 sleep，否则系统会卸载钩子
unsafe extern "system" fn mouse_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if n_code == HC_ACTION as i32 && w_param.0 as u32 == WM_LBUTTONUP {
        if CAPTURE_ENABLED.load(Ordering::Relaxed) {
            // 取出鼠标坐标，丢给工作线程处理（不在回调里做任何耗时操作）
            let info = &*(l_param.0 as *const MSLLHOOKSTRUCT);
            let (x, y) = (info.pt.x, info.pt.y);
            if let Ok(guard) = SIGNAL_TX.lock() {
                if let Some(tx) = guard.as_ref() {
                    let _ = tx.send((x, y));
                }
            }
        }
    }
    CallNextHookEx(HHOOK(std::ptr::null_mut()), n_code, w_param, l_param)
}

// 安装全局鼠标钩子（在独立线程里跑消息循环）
pub fn install_mouse_hook(app_handle: AppHandle) -> anyhow::Result<()> {
    let (tx, rx) = mpsc::channel::<(i32, i32)>();
    *SIGNAL_TX.lock().unwrap() = Some(tx);

    // 工作线程：接收坐标 → 防抖 → 模拟复制 → 通知前端
    let worker_app = app_handle.clone();
    std::thread::spawn(move || {
        while let Ok((x, y)) = rx.recv() {
            // 丢弃在等待期间堆积的多余信号，避免重复触发
            while rx.try_recv().is_ok() {}
            // 等待目标应用完成选中
            std::thread::sleep(std::time::Duration::from_millis(180));
            // 把鼠标坐标一并发给前端，用于定位悬浮窗
            let _ = worker_app.emit("text-selected", (x, y));
        }
    });

    // 钩子线程：低级钩子必须有消息循环才能收到事件
    std::thread::spawn(move || unsafe {
        let hook = match SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), HINSTANCE::default(), 0) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("安装鼠标钩子失败: {}", e);
                return;
            }
        };
        println!("✓ 全局鼠标钩子已安装");

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = UnhookWindowsHookEx(hook);
    });

    Ok(())
}

// 启用/禁用捕获
pub fn set_capture_enabled(enabled: bool) {
    CAPTURE_ENABLED.store(enabled, Ordering::Relaxed);
    println!("✓ 划词捕获已{}", if enabled { "启用" } else { "禁用" });
}

// 获取捕获状态
pub fn is_capture_enabled() -> bool {
    CAPTURE_ENABLED.load(Ordering::Relaxed)
}
