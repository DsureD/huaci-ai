use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

// 获取选中文本
pub fn get_selected_text(app: &AppHandle) -> anyhow::Result<String> {
    // 保存当前剪贴板内容
    let old_clipboard = app.clipboard().read_text().ok();

    // 不再清空剪贴板(避免触发某些终端清空选区的副作用)
    // 直接模拟 Ctrl+C,然后比对内容差异判断是否有新选中

    // 模拟 Ctrl+C
    simulate_ctrl_c()?;

    // 等待剪贴板更新
    std::thread::sleep(std::time::Duration::from_millis(150));

    // 读取新内容
    let new_clipboard = app
        .clipboard()
        .read_text()
        .unwrap_or_default()
        .trim()
        .to_string();

    // 判断是否有新选中:新内容非空 且 与旧内容不同
    let selected_text = if !new_clipboard.is_empty()
        && old_clipboard.as_deref() != Some(new_clipboard.as_str()) {
        new_clipboard.clone()
    } else {
        String::new()
    };

    // 恢复原剪贴板内容
    if let Some(old_text) = old_clipboard {
        let _ = app.clipboard().write_text(old_text);
    } else if selected_text.is_empty() {
        // 如果原剪贴板为空且没选中新内容,保持空
        let _ = app.clipboard().write_text("");
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
