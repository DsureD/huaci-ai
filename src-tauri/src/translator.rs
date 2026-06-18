use crate::config::{ApiEndpoint, ProxyConfig};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
    stream: bool,  // 明确禁用流式
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    object: Option<String>,
    #[serde(default)]
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
    #[serde(default)]
    finish_reason: Option<String>,
    #[serde(default)]
    index: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct Message {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    role: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
struct ModelInfo {
    id: String,
}

// 查询某个接口下可用的模型列表
pub async fn list_models(
    base_url: &str,
    api_key: &str,
    proxy: &ProxyConfig,
    timeout: Duration,
) -> anyhow::Result<Vec<String>> {
    let client = build_client(proxy, timeout)?;

    let url = format!("{}/models", base_url.trim_end_matches('/'));

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("获取模型失败 ({}): {}", status, error_text));
    }

    let parsed: ModelsResponse = response.json().await?;
    let mut ids: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
    ids.sort();
    Ok(ids)
}

// 翻译文本
pub async fn translate_text(
    client: &Client,
    text: &str,
    endpoint: &ApiEndpoint,
    prompt_template: &str,
) -> anyhow::Result<String> {
    // 构建提示词
    let prompt = prompt_template.replace("{text}", text);

    // 构建请求
    let request = ChatRequest {
        model: endpoint.model.clone(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }],
        temperature: 0.3,
        max_tokens: normalize_max_tokens(endpoint.max_tokens),
        stream: false,  // 明确禁用流式响应
    };

    // 发送请求
    let url = format!("{}/chat/completions", endpoint.base_url.trim_end_matches('/'));

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", endpoint.api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "API 请求失败 ({}): {}",
            status,
            error_text
        ));
    }

    // 先取响应文本,解析失败时能看到原始内容
    // 限制响应体最大 2MB,防止异常响应导致内存爆炸
    let response_bytes = response.bytes().await?;
    if response_bytes.len() > 2 * 1024 * 1024 {
        return Err(anyhow::anyhow!(
            "API 响应体过大({} bytes),可能是流式响应未正确关闭或服务端异常",
            response_bytes.len()
        ));
    }
    let response_text = String::from_utf8_lossy(&response_bytes).to_string();

    let chat_response: ChatResponse = serde_json::from_str(&response_text)
        .map_err(|e| anyhow::anyhow!("解析 API 响应失败: {}", e))?;

    if let Some(choice) = chat_response.choices.first() {
        if let Some(content) = &choice.message.content {
            let trimmed = content.trim();
            if trimmed.is_empty() {
                Err(anyhow::anyhow!(
                    "API 返回空内容{}",
                    choice
                        .finish_reason
                        .as_ref()
                        .map(|r| format!("，finish_reason={}", r))
                        .unwrap_or_default()
                ))
            } else {
                Ok(trimmed.to_string())
            }
        } else {
            Err(anyhow::anyhow!(
                "API 返回的 message.content 为空{}",
                choice
                    .finish_reason
                    .as_ref()
                    .map(|r| format!("，finish_reason={}", r))
                    .unwrap_or_default()
            ))
        }
    } else {
        Err(anyhow::anyhow!("API 返回空 choices 数组"))
    }
}

pub fn build_client(proxy: &ProxyConfig, timeout: Duration) -> anyhow::Result<Client> {
    let timeout = normalize_timeout(timeout);
    let connect_timeout = timeout.min(Duration::from_secs(10));
    let mut client_builder = Client::builder()
        .timeout(timeout)
        .connect_timeout(connect_timeout);

    if proxy.enabled {
        let proxy_url = format!("http://{}:{}", proxy.host, proxy.port);
        client_builder = client_builder.proxy(reqwest::Proxy::all(&proxy_url)?);
    }

    Ok(client_builder.build()?)
}

pub fn normalize_timeout(timeout: Duration) -> Duration {
    if timeout.is_zero() {
        Duration::from_secs(1)
    } else {
        timeout
    }
}

fn normalize_max_tokens(max_tokens: u32) -> u32 {
    max_tokens.clamp(1, 200_000)
}
