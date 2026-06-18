use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::DataExchange::{
    CloseClipboard, CountClipboardFormats, GetClipboardSequenceNumber,
    IsClipboardFormatAvailable, OpenClipboard,
};
use windows::Win32::UI::Input::KeyboardAndMouse::*;

const CF_TEXT_FORMAT: u32 = 1;
const CF_UNICODETEXT_FORMAT: u32 = 13;
const CF_HDROP_FORMAT: u32 = 15;
const COPY_WAIT_ATTEMPTS: usize = 20;
const COPY_WAIT_INTERVAL_MS: u64 = 20;

// 获取选中文本,返回 (选中的文本, 需要恢复的旧剪贴板内容, 取词后的剪贴板序号)
pub fn get_selected_text(app: &AppHandle) -> anyhow::Result<(String, Option<String>, Option<u32>)> {
    // 首选 UI Automation：直接读取焦点控件中的选中文本，不触碰剪贴板、不模拟按键，
    // 因此既不污染剪贴板，也不会在终端里触发中断。成功时无需恢复，旧剪贴板返回 None。
    if let Some(text) = crate::uia::get_selected_text_uia() {
        return Ok((text, None, None));
    }

    // 回退：少数不支持 UIA TextPattern 的控件，才退回到模拟 Ctrl+C 取词
    // 保存当前剪贴板内容
    let old_clipboard = app.clipboard().read_text().ok();
    // 如果当前剪贴板有文件、图片等非文本内容，不能用文本哨兵覆盖它。
    // 例如资源管理器里 Ctrl+C 复制文件后，双击会触发取词；此时写入任何文本都会让“粘贴文件”失效。
    if clipboard_has_file_drop_data()
        || (old_clipboard.is_none() && clipboard_has_non_text_data())
    {
        return Ok((String::new(), None, None));
    }

    let sentinel = format!(
        "__HUACI_AI_COPY_SENTINEL_{}__",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    );

    // 先写入哨兵值。Ctrl+C 后如果剪贴板仍是哨兵，说明这次操作并没有复制出选中文本，
    // 不能拿旧剪贴板内容当作“本次选中”来弹窗。
    app.clipboard()
        .write_text(sentinel.clone())
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    simulate_ctrl_c()?;

    // 等待剪贴板从哨兵值变为复制结果；多数应用很快完成，少数应用稍慢。
    let mut new_clipboard = None;
    for _ in 0..COPY_WAIT_ATTEMPTS {
        std::thread::sleep(std::time::Duration::from_millis(COPY_WAIT_INTERVAL_MS));
        new_clipboard = app.clipboard().read_text().ok();
        if new_clipboard.as_deref() != Some(sentinel.as_str()) {
            break;
        }
    }

    let selected_text = match &new_clipboard {
        Some(new) if new != &sentinel && new.trim().chars().count() >= 2 => {
            new.trim().to_string()
        }
        _ => String::new(),
    };

    // 返回选中文本和旧剪贴板内容,让调用方决定何时恢复
    // (立即恢复会触发终端清空选区,需延迟到弹窗显示后)
    if selected_text.is_empty() {
        restore_clipboard_now(app, old_clipboard.as_deref());
        Ok((String::new(), None, None))
    } else {
        Ok((selected_text, old_clipboard, Some(clipboard_sequence_number())))
    }
}

pub fn clipboard_sequence_number() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

fn clipboard_has_file_drop_data() -> bool {
    unsafe {
        if OpenClipboard(HWND(std::ptr::null_mut())).is_err() {
            return false;
        }

        let has_file_drop = IsClipboardFormatAvailable(CF_HDROP_FORMAT).is_ok();
        let _ = CloseClipboard();

        has_file_drop
    }
}

fn clipboard_has_non_text_data() -> bool {
    unsafe {
        if OpenClipboard(HWND(std::ptr::null_mut())).is_err() {
            return false;
        }

        let has_formats = CountClipboardFormats() > 0;
        let has_text = IsClipboardFormatAvailable(CF_UNICODETEXT_FORMAT).is_ok()
            || IsClipboardFormatAvailable(CF_TEXT_FORMAT).is_ok();
        let _ = CloseClipboard();

        has_formats && !has_text
    }
}

fn restore_clipboard_now(app: &AppHandle, old_text: Option<&str>) {
    let _ = app
        .clipboard()
        .write_text(old_text.unwrap_or_default().to_string());
}

// 模拟 Ctrl+C 按键
fn simulate_ctrl_c() -> anyhow::Result<()> {
    unsafe {
        let mut inputs = vec![];

        // Ctrl 按下
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_CONTROL,
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });

        // C 按下
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY('C' as u16),
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });

        // C 释放
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY('C' as u16),
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });

        // Ctrl 释放
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_CONTROL,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });

        let sent = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        if sent != inputs.len() as u32 {
            return Err(anyhow::anyhow!(
                "模拟 Ctrl+C 失败，仅发送 {sent}/{} 个输入事件",
                inputs.len()
            ));
        }
    }

    Ok(())
}
