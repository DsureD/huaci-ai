#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "HuaciAI";
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn set_auto_start(enabled: bool) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        if enabled {
            enable_auto_start()
        } else {
            disable_auto_start()
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn enable_auto_start() -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let command = format!("\"{}\"", exe.display());

    let status = std::process::Command::new("reg")
        .args([
            "add", RUN_KEY, "/v", RUN_VALUE, "/t", "REG_SZ", "/d", &command, "/f",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("写入开机自启注册表失败"))
    }
}

#[cfg(target_os = "windows")]
fn disable_auto_start() -> anyhow::Result<()> {
    let status = std::process::Command::new("reg")
        .args(["delete", RUN_KEY, "/v", RUN_VALUE, "/f"])
        .creation_flags(CREATE_NO_WINDOW)
        .status()?;

    if status.success() || status.code() == Some(1) {
        Ok(())
    } else {
        Err(anyhow::anyhow!("删除开机自启注册表失败"))
    }
}
