use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

// 获取选中文本
pub fn get_selected_text(app: &AppHandle) -> anyhow::Result<String> {
    // 保存当前剪贴板内容
    let old_clipboard = app.clipboard().read_text().ok();

    // 清空剪贴板
    let _ = app.clipboard().write_text("");

    // 模拟 Ctrl+C
    simulate_ctrl_c()?;

    // 等待剪贴板更新
    std::thread::sleep(std::time::Duration::from_millis(100));

    // 读取新内容
    let selected_text = app
        .clipboard()
        .read_text()
        .unwrap_or_default()
        .trim()
        .to_string();

    // 恢复原剪贴板内容
    if let Some(old_text) = old_clipboard {
        let _ = app.clipboard().write_text(old_text);
    }

    Ok(selected_text)
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

        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }

    Ok(())
}
