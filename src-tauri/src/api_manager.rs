use crate::config::{ApiEndpoint, ProxyConfig};
use crate::translator;
use std::time::Duration;

// 使用故障转移机制调用多个 API，返回 (结果文本, 成功的接口)
// on_progress(当前序号_从1起, 总数, 接口名)：每尝试一个接口前回调一次，供前端显示进度
pub async fn translate_with_failover(
    text: &str,
    mut endpoints: Vec<ApiEndpoint>,
    prompt_template: &str,
    proxy: &ProxyConfig,
    timeout: Duration,
    on_progress: impl Fn(usize, usize, &str),
) -> anyhow::Result<(String, ApiEndpoint)> {
    // 过滤启用的接口并按优先级排序
    endpoints.retain(|e| e.enabled);
    endpoints.sort_by_key(|e| e.priority);

    if endpoints.is_empty() {
        return Err(anyhow::anyhow!("没有可用的 API 接口"));
    }

    let total = endpoints.len();
    let mut last_error = None;

    for (idx, endpoint) in endpoints.iter().enumerate() {
        on_progress(idx + 1, total, &endpoint.name);
        // 双重超时：reqwest 自身设了 timeout，外层再套一个硬上限 tokio::time::timeout，
        // 即使底层请求因某些原因（代理挂起、连接半开等）没在 reqwest 超时内返回，
        // 也会被这里强制中断并转移到下一个接口，避免永远卡在「正在请求 …」。
        let fut = translator::translate_text(text, endpoint, prompt_template, proxy, timeout);
        match tokio::time::timeout(timeout, fut).await {
            Ok(Ok(result)) => {
                println!("✓ 接口 {} 调用成功", endpoint.name);
                return Ok((result, endpoint.clone()));
            }
            Ok(Err(e)) => {
                eprintln!("✗ 接口 {} 失败: {}", endpoint.name, e);
                last_error = Some(e);
            }
            Err(_) => {
                let e = anyhow::anyhow!("接口 {} 请求超时（{} 秒）", endpoint.name, timeout.as_secs());
                eprintln!("✗ {}", e);
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("所有 API 接口均不可用")))
}
