use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

// 获取选中文本
pub fn get_selected_text(app: &AppHandle) -> anyhow::Result<String> {
    // 保存当前剪贴板内容
    let old_clipboard = app.clipboard().read_text().ok();

    // 直接模拟 Ctrl+C(不清空,避免触发终端副作用)
    simulate_ctrl_c()?;

    // 等待剪贴板更新(稍长一点确保复制完成)
    std::thread::sleep(std::time::Duration::from_millis(120));

    // 读取新内容
    let new_clipboard = app.clipboard().read_text().ok();

    // 严格判断:新内容非空 且 与旧内容不同 且 长度≥2(排除单字符误触发)
    let selected_text = match (&old_clipboard, &new_clipboard) {
        (Some(old), Some(new)) if new.trim() != old.trim() && new.trim().len() >= 2 => {
            new.trim().to_string()
        }
        (None, Some(new)) if new.trim().len() >= 2 => {
            new.trim().to_string()
        }
        _ => String::new(),
    };

    // 如果没检测到新选中,不动剪贴板(避免频繁写入);
    // 如果检测到新选中,也暂时保留在剪贴板(用户可能想继续用)
    // 只在明确有新选中且与旧不同时,才考虑恢复旧内容
    if selected_text.is_empty() {
        // 没选中,保持剪贴板不变
        Ok(String::new())
    } else {
        // 有选中,立即恢复旧剪贴板(让用户剪贴板不受影响)
        if let Some(old_text) = old_clipboard {
            let _ = app.clipboard().write_text(old_text);
        }
        Ok(selected_text)
    }
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
