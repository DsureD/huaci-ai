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
    #[serde(default = "default_prompts")]
    pub prompts: PromptConfig,
    #[serde(default)]
    pub actions: Vec<ActionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub auto_translate: bool,
    pub min_text_length: usize,
    #[serde(default = "default_true")]
    pub show_copy_button: bool,
    #[serde(default = "default_true")]
    pub clipboard_fallback_enabled: bool,
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
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
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

// 划词弹窗的可自定义功能项（替代写死的 翻译/解释）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionItem {
    pub name: String,   // 显示名称
    pub prompt: String, // 提示词模板，用 {text} 占位
    pub icon: String,   // 图标名，对应前端 icons 集合的 key
    pub enabled: bool,  // 是否在弹窗显示
    #[serde(default)]
    pub auto: bool, // 划词后是否自动执行该功能（取代旧的全局 auto_translate）
}

// prompts 字段缺失时的兜底值（也是旧配置迁移的来源）
fn default_prompts() -> PromptConfig {
    PromptConfig {
        translate: "你是专业的翻译助手。将以下文本翻译成中文，只返回翻译结果，不要解释：\n\n{text}".to_string(),
        explain: "请用简洁的语言解释以下内容的含义：\n\n{text}".to_string(),
    }
}

fn default_max_tokens() -> u32 {
    1000
}

fn default_true() -> bool {
    true
}

// 全新安装时的默认功能项
fn default_actions() -> Vec<ActionItem> {
    vec![
        ActionItem {
            name: "翻译".to_string(),
            prompt: "你是专业的翻译助手。将以下文本翻译成中文，只返回翻译结果，不要解释：\n\n{text}".to_string(),
            icon: "translate".to_string(),
            enabled: true,
            auto: true,
        },
        ActionItem {
            name: "解释".to_string(),
            prompt: "请用简洁的语言解释以下内容的含义：\n\n{text}".to_string(),
            icon: "explain".to_string(),
            enabled: true,
            auto: false,
        },
    ]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app: AppConfig {
                auto_translate: true,
                min_text_length: 1,
                show_copy_button: true,
                clipboard_fallback_enabled: true,
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
                        max_tokens: default_max_tokens(),
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
            prompts: default_prompts(),
            actions: default_actions(),
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
        let mut config: Config = serde_json::from_str(&content)?;
        // 旧配置没有 actions：用 prompts 迁移成两个功能项，保留用户曾经自定义的提示词
        // 自动执行沿用旧的全局 auto_translate（仅给“翻译”项）
        if config.actions.is_empty() {
            config.actions = vec![
                ActionItem {
                    name: "翻译".to_string(),
                    prompt: config.prompts.translate.clone(),
                    icon: "translate".to_string(),
                    enabled: true,
                    auto: config.app.auto_translate,
                },
                ActionItem {
                    name: "解释".to_string(),
                    prompt: config.prompts.explain.clone(),
                    icon: "explain".to_string(),
                    enabled: true,
                    auto: false,
                },
            ];
        }
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
