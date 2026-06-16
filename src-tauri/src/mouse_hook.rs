use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// 是否启用划词捕获（原子量，可跨线程安全读写）
static CAPTURE_ENABLED: AtomicBool = AtomicBool::new(true);
// 钩子线程向工作线程发送鼠标信号的通道
static SIGNAL_TX: Mutex<Option<Sender<MouseSignal>>> = Mutex::new(None);
// 记录左键按下坐标，用于在抬起时区分“单击”与“拖动选择”
static DOWN_POS: Mutex<Option<(i32, i32)>> = Mutex::new(None);

// 悬浮窗句柄与可见状态：用于“点到窗口之外即关闭”的判断。
// 弹窗不抢焦点，无法依赖失焦事件，这里用纯 Win32 判断点击位置来驱动关闭。
static POPUP_HWND: AtomicIsize = AtomicIsize::new(0);
static POPUP_VISIBLE: AtomicBool = AtomicBool::new(false);

// 拖动判定阈值（物理像素）：任一轴位移达到它才算“拖选”，否则视为单击
const DRAG_THRESHOLD: i32 = 5;
// 双击判定：两次抬起的间隔与位移阈值（用于支持“双击选词”）
const DBLCLICK_MS: u128 = 400;
const DBLCLICK_DIST: i32 = 6;

// 鼠标信号：按下 / 抬起（抬起带上对应的按下点，便于计算拖动距离）
enum MouseSignal {
    Down(i32, i32),
    Up {
        down: Option<(i32, i32)>,
        up: (i32, i32),
    },
}

// 鼠标钩子回调：必须立刻返回，绝不能 sleep，否则系统会卸载钩子
unsafe extern "system" fn mouse_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if n_code == HC_ACTION as i32 {
        let msg = w_param.0 as u32;
        if msg == WM_LBUTTONDOWN {
            let info = &*(l_param.0 as *const MSLLHOOKSTRUCT);
            let (x, y) = (info.pt.x, info.pt.y);
            if let Ok(mut g) = DOWN_POS.lock() {
                *g = Some((x, y));
            }
            send_signal(MouseSignal::Down(x, y));
        } else if msg == WM_LBUTTONUP {
            let info = &*(l_param.0 as *const MSLLHOOKSTRUCT);
            let up = (info.pt.x, info.pt.y);
            let down = DOWN_POS.lock().ok().and_then(|mut g| g.take());
            send_signal(MouseSignal::Up { down, up });
        }
    }
    CallNextHookEx(HHOOK(std::ptr::null_mut()), n_code, w_param, l_param)
}

fn send_signal(sig: MouseSignal) {
    if let Ok(guard) = SIGNAL_TX.lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(sig);
        }
    }
}

// 安装全局鼠标钩子（在独立线程里跑消息循环）
pub fn install_mouse_hook(app_handle: AppHandle) -> anyhow::Result<()> {
    let (tx, rx) = mpsc::channel::<MouseSignal>();
    *SIGNAL_TX.lock().unwrap() = Some(tx);

    // 工作线程：判定动作（拖选/双击才取词；点窗口外则关闭弹窗）
    let worker_app = app_handle.clone();
    std::thread::spawn(move || {
        // 上一次“抬起”的时间与位置，用于识别双击选词
        let mut last_up: Option<(Instant, i32, i32)> = None;

        while let Ok(sig) = rx.recv() {
            match sig {
                // 按下：弹窗可见且点到窗口之外 → 通知前端关闭（替代失焦关闭）
                MouseSignal::Down(x, y) => {
                    if popup_outside(x, y) {
                        let _ = worker_app.emit("close-popup", ());
                    }
                }
                // 抬起：仅“拖选”或“双击”才视为有效选词动作，普通单击不取词
                MouseSignal::Up { down, up } => {
                    if !CAPTURE_ENABLED.load(Ordering::Relaxed) {
                        continue;
                    }

                    // 弹窗内的点击、拖动标题栏、双击按钮都属于弹窗交互，不能再进入
                    // 划词取词流程；否则会清空当前结果并可能触发 Ctrl+C 回退。
                    if point_in_popup(up.0, up.1)
                        || down
                            .map(|(x, y)| point_in_popup(x, y))
                            .unwrap_or(false)
                    {
                        continue;
                    }

                    let is_drag = match down {
                        Some((dx, dy)) => {
                            (up.0 - dx).abs() >= DRAG_THRESHOLD
                                || (up.1 - dy).abs() >= DRAG_THRESHOLD
                        }
                        None => false,
                    };

                    let now = Instant::now();
                    let is_double = match last_up {
                        Some((t, lx, ly)) => {
                            now.duration_since(t).as_millis() < DBLCLICK_MS
                                && (up.0 - lx).abs() < DBLCLICK_DIST
                                && (up.1 - ly).abs() < DBLCLICK_DIST
                        }
                        None => false,
                    };
                    last_up = Some((now, up.0, up.1));

                    // 普通单击（既非拖选也非双击）不取词：
                    // 终端里点击定位光标、桌面单击图标等都不再误触发
                    if !is_drag && !is_double {
                        continue;
                    }

                    // 终端/控制台 UIA 支持差，且 Ctrl+C 回退会中断进程，这里直接跳过取词
                    if is_terminal_foreground() {
                        continue;
                    }

                    // 等待目标应用完成选中，再把坐标交给前端定位悬浮窗
                    std::thread::sleep(Duration::from_millis(180));
                    let _ = worker_app.emit("text-selected", up);
                }
            }
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

// 供 commands::show_popup / hide_popup 更新悬浮窗状态
pub fn set_popup_hwnd(hwnd: isize) {
    POPUP_HWND.store(hwnd, Ordering::Relaxed);
}
pub fn set_popup_visible(visible: bool) {
    POPUP_VISIBLE.store(visible, Ordering::Relaxed);
}

// 悬浮窗当前是否可见，且点 (x,y) 落在窗口之外（纯 Win32 判断，不依赖 Tauri 跨线程方法）
fn popup_outside(x: i32, y: i32) -> bool {
    if !POPUP_VISIBLE.load(Ordering::Relaxed) {
        return false;
    }
    !point_in_popup(x, y)
}

fn point_in_popup(x: i32, y: i32) -> bool {
    if !POPUP_VISIBLE.load(Ordering::Relaxed) {
        return false;
    }
    let h = POPUP_HWND.load(Ordering::Relaxed);
    if h == 0 {
        return false;
    }
    unsafe {
        let hwnd = HWND(h as *mut _);
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok() {
            return x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom;
        }
    }
    false
}

// 前台窗口是否是终端/控制台（这类程序里 Ctrl+C = 中断，且多数不支持 UIA 取词）
fn is_terminal_foreground() -> bool {
    match foreground_class_name() {
        Some(cls) => {
            let c = cls.to_lowercase();
            c == "consolewindowclass"          // cmd / powershell / conhost 等传统控制台
                || c.contains("cascadia")      // Windows Terminal
                || c == "virtualconsoleclass"  // ConEmu
                || c.contains("mintty") // Git Bash / Cygwin / MSYS2
        }
        None => false,
    }
}

// 取前台窗口的窗口类名
fn foreground_class_name() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut buf = [0u16; 256];
        let len = GetClassNameW(hwnd, &mut buf);
        if len <= 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
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

// 当前鼠标光标的屏幕坐标（手动翻译快捷键用它定位弹窗）
pub fn cursor_pos() -> (i32, i32) {
    unsafe {
        let mut p = POINT::default();
        let _ = GetCursorPos(&mut p);
        (p.x, p.y)
    }
}
