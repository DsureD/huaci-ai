use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub app: AppConfig,
    pub hotkeys: HotkeyConfig,
    pub api: ApiConfig,
    pub proxy: ProxyConfig,
    pub ui: UiConfig,
    pub prompts: PromptConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub auto_translate: bool,
    pub min_text_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub toggle_capture: String,
    pub manual_translate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub timeout_seconds: u64,
    pub endpoints: Vec<ApiEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEndpoint {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub enabled: bool,
    pub priority: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub opacity: f32,
    pub font_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptConfig {
    pub translate: String,
    pub explain: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app: AppConfig {
                auto_translate: true,
                min_text_length: 1,
            },
            hotkeys: HotkeyConfig {
                toggle_capture: "Ctrl+Shift+H".to_string(),
                manual_translate: "Ctrl+Q".to_string(),
            },
            api: ApiConfig {
                timeout_seconds: 30,
                endpoints: vec![
                    ApiEndpoint {
                        name: "OpenAI".to_string(),
                        base_url: "https://api.openai.com/v1".to_string(),
                        api_key: "sk-your-api-key-here".to_string(),
                        model: "gpt-4o-mini".to_string(),
                        enabled: true,
                        priority: 1,
                    },
                ],
            },
            proxy: ProxyConfig {
                enabled: false,
                host: "127.0.0.1".to_string(),
                port: 7890,
            },
            ui: UiConfig {
                opacity: 0.95,
                font_size: 14,
            },
            prompts: PromptConfig {
                translate: "你是专业的翻译助手。将以下文本翻译成中文，只返回翻译结果，不要解释：\n\n{text}".to_string(),
                explain: "请用简洁的语言解释以下内容的含义：\n\n{text}".to_string(),
            },
        }
    }
}

// 获取配置文件路径
pub fn get_config_path() -> anyhow::Result<PathBuf> {
    // 优先使用程序目录（便携模式）
    let exe_dir = std::env::current_exe()?
        .parent()
        .ok_or_else(|| anyhow::anyhow!("无法获取程序目录"))?
        .to_path_buf();

    let portable_config = exe_dir.join("config.json");

    if portable_config.exists() {
        return Ok(portable_config);
    }

    // 使用 AppData 目录
    let app_data = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?
        .join("huaci-ai");

    fs::create_dir_all(&app_data)?;
    Ok(app_data.join("config.json"))
}

// 加载配置
pub fn load_config() -> anyhow::Result<Config> {
    let config_path = get_config_path()?;

    if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    } else {
        let default_config = Config::default();
        save_config(&default_config)?;
        Ok(default_config)
    }
}

// 保存配置
pub fn save_config(config: &Config) -> anyhow::Result<()> {
    let config_path = get_config_path()?;
    let content = serde_json::to_string_pretty(config)?;
    fs::write(&config_path, content)?;
    Ok(())
}
