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
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Deserialize)]
struct Message {
    content: String,
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
    let mut client_builder = Client::builder().timeout(timeout);

    if proxy.enabled {
        let proxy_url = format!("http://{}:{}", proxy.host, proxy.port);
        client_builder = client_builder.proxy(reqwest::Proxy::all(&proxy_url)?);
    }

    let client = client_builder.build()?;

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
    text: &str,
    endpoint: &ApiEndpoint,
    prompt_template: &str,
    proxy: &ProxyConfig,
    timeout: Duration,
) -> anyhow::Result<String> {
    // 构建 HTTP 客户端
    let mut client_builder = Client::builder().timeout(timeout);

    if proxy.enabled {
        let proxy_url = format!("http://{}:{}", proxy.host, proxy.port);
        client_builder = client_builder.proxy(reqwest::Proxy::all(&proxy_url)?);
    }

    let client = client_builder.build()?;

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
        max_tokens: 1000,
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

    let chat_response: ChatResponse = response.json().await?;

    if let Some(choice) = chat_response.choices.first() {
        Ok(choice.message.content.trim().to_string())
    } else {
        Err(anyhow::anyhow!("API 返回空结果"))
    }
}
