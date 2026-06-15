// 通过 Windows UI Automation 直接读取“当前焦点控件中已选中的文本”。
//
// 相比模拟 Ctrl+C 取词：不触碰剪贴板、不发送任何按键，因此既不会污染/覆盖
// 用户剪贴板，也不会在终端等程序里把 Ctrl+C 当成中断信号。
// 仅对支持 UIA TextPattern 的控件有效（现代浏览器、Office、记事本、多数原生/WPF/UWP
// 控件等）；不支持时返回 None，由调用方回退到 Ctrl+C 方案。

use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationTextPattern, UIA_TextPatternId,
};

// 读取当前选中文本；取不到（控件不支持 / 无选中）返回 None
pub fn get_selected_text_uia() -> Option<String> {
    // 放到独立的 MTA 线程执行：UIA 属于跨进程 COM 调用，MTA 套间下无需消息泵即可可靠返回，
    // 也避免干扰 Tauri 命令线程已有的 COM 套间设置。
    std::thread::spawn(|| unsafe {
        let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        let text = read_selection().ok().flatten();
        // 仅在本线程确实初始化成功时才反初始化，保持 init/uninit 配对
        if hr.is_ok() {
            CoUninitialize();
        }
        text
    })
    .join()
    .ok()
    .flatten()
}

unsafe fn read_selection() -> windows::core::Result<Option<String>> {
    let automation: IUIAutomation =
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
    let focused = automation.GetFocusedElement()?;

    // 焦点控件若不支持 TextPattern，这里会返回错误 → 交给调用方回退到 Ctrl+C
    let text_pattern: IUIAutomationTextPattern = focused.GetCurrentPatternAs(UIA_TextPatternId)?;
    let selection = text_pattern.GetSelection()?;
    let count = selection.Length()?;
    if count <= 0 {
        return Ok(None);
    }

    let mut out = String::new();
    for i in 0..count {
        let range = selection.GetElement(i)?;
        let bstr = range.GetText(-1)?;
        out.push_str(&String::from_utf16_lossy(bstr.as_wide()));
    }

    let trimmed = out.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}
