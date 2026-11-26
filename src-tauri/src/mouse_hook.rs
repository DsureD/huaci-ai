use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// 是否启用划词捕获（原子量，可跨线程安全读写）
static CAPTURE_ENABLED: AtomicBool = AtomicBool::new(true);
// 钩子线程向工作线程发送鼠标信号的通道
static SIGNAL_TX: Mutex<Option<Sender<MouseSignal>>> = Mutex::new(None);
// 记录左键按下坐标，用于在抬起时区分“单击”与“拖动选择”
static DOWN_POS: Mutex<Option<(i32, i32)>> = Mutex::new(None);
// 确保全局钩子与工作线程只安装一次
static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);
// 最近一次钩子收到任意鼠标事件的时间戳（毫秒），用于判断系统是否静默卸载了低级钩子
static LAST_HOOK_EVENT_MS: AtomicU64 = AtomicU64::new(0);
// 钩子是否处于需要重装的状态（例如重装命令触发或看门狗触发）
static HOOK_REINSTALL_GEN: AtomicU32 = AtomicU32::new(0);

// 悬浮窗句柄与可见状态：用于“点到窗口之外即关闭”的判断。
// 弹窗不抢焦点，无法依赖失焦事件，这里用纯 Win32 判断点击位置来驱动关闭。
static POPUP_HWND: AtomicIsize = AtomicIsize::new(0);
static POPUP_VISIBLE: AtomicBool = AtomicBool::new(false);

// 拖动判定阈值（物理像素）：任一轴位移达到它才算“拖选”，否则视为单击
const DRAG_THRESHOLD: i32 = 5;
// 近窗口右/下边缘的明显单轴拖动大概率是拖滚条，不应触发划词。
const SCROLLBAR_EDGE_PX: i32 = 24;
// 双击判定：两次抬起的间隔与位移阈值（用于支持“双击选词”）
const DBLCLICK_MS: u128 = 400;
const DBLCLICK_DIST: i32 = 6;
// 周期刷新低级鼠标钩子：防止系统休眠/锁屏/安全软件或系统超时后 WH_MOUSE_LL 被静默摘除
const HOOK_HEALTH_CHECK_MS: u32 = 30_000;
const HOOK_IDLE_REINSTALL_MS: u64 = 120_000;
const HOOK_TIMER_ID: usize = 1;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// 鼠标信号：按下 / 抬起（抬起带上对应的按下点，便于计算拖动距离）
enum MouseSignal {
    Down(i32, i32),
    Up {
        down: Option<(i32, i32)>,
        up: (i32, i32),
    },
}

// 鼠标钩子回调：必须立刻返回，绝不能 sleep 或阻塞，否则系统会静默卸载钩子
unsafe extern "system" fn mouse_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if n_code == HC_ACTION as i32 {
        LAST_HOOK_EVENT_MS.store(now_ms(), Ordering::Relaxed);
        let msg = w_param.0 as u32;
        if msg == WM_LBUTTONDOWN {
            let info = &*(l_param.0 as *const MSLLHOOKSTRUCT);
            let (x, y) = (info.pt.x, info.pt.y);
            if let Ok(mut g) = DOWN_POS.try_lock() {
                *g = Some((x, y));
            }
            send_signal(MouseSignal::Down(x, y));
        } else if msg == WM_LBUTTONUP {
            let info = &*(l_param.0 as *const MSLLHOOKSTRUCT);
            let up = (info.pt.x, info.pt.y);
            let down = DOWN_POS.try_lock().ok().and_then(|mut g| g.take());
            send_signal(MouseSignal::Up { down, up });
        }
    }
    CallNextHookEx(HHOOK(std::ptr::null_mut()), n_code, w_param, l_param)
}

fn send_signal(sig: MouseSignal) {
    // 回调里只用 try_lock，避免锁竞争拖慢 WH_MOUSE_LL 触发 Windows 低级钩子超时卸载
    if let Ok(guard) = SIGNAL_TX.try_lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(sig);
        }
    }
}

// 请求钩子线程在下一次心跳周期重装低级鼠标钩子（外部可触发）
pub fn request_hook_reinstall() {
    HOOK_REINSTALL_GEN.fetch_add(1, Ordering::Relaxed);
}

// 安装全局鼠标钩子（在独立线程里跑消息循环）
pub fn install_mouse_hook(app_handle: AppHandle) -> anyhow::Result<()> {
    if HOOK_INSTALLED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let (tx, rx) = mpsc::channel::<MouseSignal>();
    *SIGNAL_TX.lock().unwrap() = Some(tx);
    LAST_HOOK_EVENT_MS.store(now_ms(), Ordering::Relaxed);

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

                    // 设置页/悬浮窗等本应用窗口内的划选不应触发全局划词。
                    // 否则会对自己的 WebView 做 UIA/剪贴板取词，容易造成重入和卡顿。
                    if is_own_process_foreground() {
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

                    if is_drag && down.map(|p| is_scrollbar_like_drag(p, up)).unwrap_or(false) {
                        continue;
                    }

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
                    // 触发划词前确保悬浮翻译窗口存活（长时间后台运行后 WebView2 崩溃时自动重建）
                    crate::commands::trigger_text_selected(&worker_app, up);
                }
            }
        }
    });

    // 钩子线程：低级钩子必须有消息循环才能收到事件，
    // 同时借助线程定时器定期检测：若长时间未收到鼠标事件或收到重装信号（如系统休眠唤醒），自动重装钩子。
    std::thread::spawn(move || unsafe {
        let mut hook = match SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), HINSTANCE::default(), 0) {
            Ok(h) => {
                crate::logger::log_info("✓ 全局鼠标钩子已安装");
                Some(h)
            }
            Err(e) => {
                crate::logger::log_error(&format!("安装鼠标钩子失败: {}", e));
                None
            }
        };

        let mut last_cursor = cursor_pos();
        let mut last_gen = HOOK_REINSTALL_GEN.load(Ordering::Relaxed);
        let timer_id = SetTimer(HWND::default(), HOOK_TIMER_ID, HOOK_HEALTH_CHECK_MS, None);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_TIMER {
                let current_cursor = cursor_pos();
                let cursor_moved = current_cursor != last_cursor;
                last_cursor = current_cursor;

                let gen = HOOK_REINSTALL_GEN.load(Ordering::Relaxed);
                let idle_ms = now_ms().saturating_sub(LAST_HOOK_EVENT_MS.load(Ordering::Relaxed));
                // 光标确实在移动但钩子已长时间没收到任何回调，说明 Windows 静默摘除了 WH_MOUSE_LL
                let need_reinstall = hook.is_none()
                    || gen != last_gen
                    || (cursor_moved && idle_ms >= HOOK_IDLE_REINSTALL_MS);

                if need_reinstall {
                    last_gen = gen;
                    if let Some(old_hook) = hook.take() {
                        let _ = UnhookWindowsHookEx(old_hook);
                    }
                    match SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), HINSTANCE::default(), 0) {
                        Ok(new_hook) => {
                            hook = Some(new_hook);
                            LAST_HOOK_EVENT_MS.store(now_ms(), Ordering::Relaxed);
                            crate::logger::log_info("✓ 全局鼠标钩子已自动重装恢复");
                        }
                        Err(e) => {
                            crate::logger::log_error(&format!("重装鼠标钩子失败: {}", e));
                        }
                    }
                }
                continue;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        if timer_id != 0 {
            let _ = KillTimer(HWND::default(), timer_id);
        }
        if let Some(h) = hook {
            let _ = UnhookWindowsHookEx(h);
        }
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

// 供关闭设置页时检测前台是否是本应用窗口、防止误判；同时供 show_popup / hide_popup 外使用
fn is_popup_window_valid(hwnd: HWND) -> bool {
    unsafe { IsWindow(hwnd).as_bool() }
}

// 悬浮窗当前是否可见，且点 (x,y) 落在窗口之外（纯 Win32 判断，不依赖 Tauri 跨线程方法）
fn popup_outside(x: i32, y: i32) -> bool {
    if !POPUP_VISIBLE.load(Ordering::Relaxed) {
        return false;
    }
    let inside = point_in_popup(x, y);
    // point_in_popup 若检测到窗口已无效/隐藏，会顺手把 POPUP_VISIBLE 清为 false
    POPUP_VISIBLE.load(Ordering::Relaxed) && !inside
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
        if !is_popup_window_valid(hwnd) || !IsWindowVisible(hwnd).as_bool() {
            // 句柄已失效或实际已隐藏时及时重置状态，避免永久停留在 visible=true
            POPUP_VISIBLE.store(false, Ordering::Relaxed);
            if !is_popup_window_valid(hwnd) {
                POPUP_HWND.store(0, Ordering::Relaxed);
            }
            return false;
        }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok() {
            return x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom;
        }
    }
    false
}

fn is_scrollbar_like_drag(down: (i32, i32), up: (i32, i32)) -> bool {
    let dx = (up.0 - down.0).abs();
    let dy = (up.1 - down.1).abs();

    if dy >= dx.saturating_mul(2)
        && (point_near_window_edge(down.0, down.1).0
            || point_near_window_edge(up.0, up.1).0)
    {
        return true;
    }

    if dx >= dy.saturating_mul(2)
        && (point_near_window_edge(down.0, down.1).1
            || point_near_window_edge(up.0, up.1).1)
    {
        return true;
    }

    false
}

fn point_near_window_edge(x: i32, y: i32) -> (bool, bool) {
    unsafe {
        let hwnd = WindowFromPoint(POINT { x, y });
        if hwnd.0.is_null() {
            return (false, false);
        }

        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return (false, false);
        }

        let near_right = x >= rect.right - SCROLLBAR_EDGE_PX && x < rect.right;
        let near_bottom = y >= rect.bottom - SCROLLBAR_EDGE_PX && y < rect.bottom;
        (near_right, near_bottom)
    }
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

fn is_own_process_foreground() -> bool {
    foreground_process_id()
        .map(|pid| pid == std::process::id())
        .unwrap_or(false)
}

fn foreground_process_id() -> Option<u32> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }

        let mut pid = 0u32;
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32));
        if pid == 0 {
            None
        } else {
            Some(pid)
        }
    }
}

// 启用/禁用捕获
pub fn set_capture_enabled(enabled: bool) {
    CAPTURE_ENABLED.store(enabled, Ordering::Relaxed);
    crate::logger::log_info(&format!("✓ 划词捕获已{}", if enabled { "启用" } else { "禁用" }));
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
