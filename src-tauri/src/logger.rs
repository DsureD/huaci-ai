use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LOG_BYTES: u64 = 512 * 1024;

static LOG_LOCK: Mutex<()> = Mutex::new(());

fn log_path() -> Option<PathBuf> {
    let config_path = crate::config::get_config_path().ok()?;
    let dir = config_path.parent()?;
    Some(dir.join("huaci-ai.log"))
}

pub fn log_info(msg: &str) {
    println!("{}", msg);
    write_line("INFO", msg);
}

pub fn log_warn(msg: &str) {
    eprintln!("[WARN] {}", msg);
    write_line("WARN", msg);
}

pub fn log_error(msg: &str) {
    eprintln!("[ERROR] {}", msg);
    write_line("ERROR", msg);
}

fn write_line(level: &str, msg: &str) {
    let Ok(_guard) = LOG_LOCK.lock() else {
        return;
    };
    let Some(path) = log_path() else {
        return;
    };

    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > MAX_LOG_BYTES {
            let backup = path.with_extension("log.old");
            let _ = fs::remove_file(&backup);
            let _ = fs::rename(&path, &backup);
        }
    }

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "[{}][{}] {}", ts, level, msg);
    }
}
