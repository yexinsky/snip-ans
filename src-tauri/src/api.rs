use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::config::AppConfig;
use crate::provider::{self, AuthKind};

use std::time::Duration;

/// 模型条目，返回给前端做下拉选择。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub owned_by: String,
}

fn extract_answer(body: &serde_json::Value) -> Option<String> {
    // 1. OpenAI 标准：content 为字符串
    if let Some(content) = body["choices"][0]["message"]["content"].as_str() {
        if !content.is_empty() {
            return Some(content.to_string());
        }
    }
    // 2. OpenAI 新形态：content 为分片数组
    if let Some(parts) = body["choices"][0]["message"]["content"].as_array() {
        let joined: String = parts
            .iter()
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join("");
        if !joined.is_empty() {
            return Some(joined);
        }
    }
    // 3. Gemini 标准：parts 可能有多段
    if let Some(parts) = body["candidates"][0]["content"]["parts"].as_array() {
        let joined: String = parts
            .iter()
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join("");
        if !joined.is_empty() {
            return Some(joined);
        }
    }
    // 4. 包装形态：{"response": {...}}
    let wrapped = &body["response"];
    if let Some(content) = wrapped["choices"][0]["message"]["content"].as_str() {
        if !content.is_empty() {
            return Some(content.to_string());
        }
    }
    if let Some(parts) = wrapped["candidates"][0]["content"]["parts"].as_array() {
        let joined: String = parts
            .iter()
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join("");
        if !joined.is_empty() {
            return Some(joined);
        }
    }

    None
}

fn apply_auth(req: reqwest::RequestBuilder, auth: AuthKind, key: &str) -> reqwest::RequestBuilder {
    match auth {
        AuthKind::Bearer => req.header("Authorization", format!("Bearer {}", key)),
        AuthKind::GoogleApiKey => req.header("x-goog-api-key", key),
        AuthKind::None => req,
    }
}

/// 当前配置对应的鉴权方式。
fn auth_for(config: &AppConfig) -> AuthKind {
    provider::find_preset(&config.provider)
        .map(|p| p.auth)
        .unwrap_or(AuthKind::Bearer)
}

fn build_client() -> Result<Client, String> {
    Client::builder()
        // 推理模型需要较长思考时间，超时放宽到 300 秒
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败：{}", e))
}

/// 从上游拉取可用模型列表。
pub async fn fetch_models(
    config: &AppConfig,
    endpoint_override: Option<String>,
    key_override: Option<String>,
) -> Result<Vec<ModelInfo>, String> {
    let endpoint = endpoint_override.unwrap_or_else(|| config.api_endpoint.clone());
    let key = key_override.unwrap_or_else(|| config.api_key.clone());

    let models_url = provider::resolve_models_endpoint(&config.provider, &endpoint)
        .ok_or_else(|| "无法推导模型列表地址，请先填写 API 端点".to_string())?;

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败：{}", e))?;

    let auth = auth_for(config);
    let req = client
        .get(&models_url)
        .header("Accept", "application/json")
        .header("User-Agent", "SnipAns/0.1.5");
    let req = apply_auth(req, auth, &key);

    let res = req.send().await.map_err(|e| {
        format!(
            "请求模型列表失败：{}。请检查网络、端点地址；本地服务可尝试用 127.0.0.1 代替 localhost。",
            e
        )
    })?;

    let status = res.status();
    let body_text = res.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(format!(
            "获取模型失败：HTTP {} — {}",
            status.as_u16(),
            truncate(&body_text, 300)
        ));
    }

    let body: Value = serde_json::from_str(&body_text)
        .map_err(|e| format!("模型列表不是合法 JSON：{}", e))?;

    let models = parse_models(&body)?;
    if models.is_empty() {
        return Err("上游未返回任何可用模型（列表为空）".to_string());
    }
    Ok(models)
}

/// 兼容多种模型列表响应形态。
///
/// 容器存在但为空时返回空 Vec（由调用方给出「上游没有可用模型」的提示），
/// 只有形状完全对不上才报解析失败 —— 两者的排查方向不同，不该混为一谈。
fn parse_models(body: &Value) -> Result<Vec<ModelInfo>, String> {
    let mut saw_container = false;

    // OpenAI 形态：{ "data": [{ "id": "...", "owned_by": "..." }] }
    if let Some(arr) = body["data"].as_array() {
        saw_container = true;
        let mut out: Vec<ModelInfo> = arr
            .iter()
            .filter_map(|m| {
                m["id"].as_str().map(|id| ModelInfo {
                    id: id.to_string(),
                    owned_by: m["owned_by"].as_str().unwrap_or("").to_string(),
                })
            })
            .collect();
        if !out.is_empty() {
            out.sort_by(|a, b| a.id.cmp(&b.id));
            return Ok(out);
        }
    }
    // Gemini 形态：{ "models": [{ "name": "models/gemini-2.0-flash", ... }] }
    if let Some(arr) = body["models"].as_array() {
        saw_container = true;
        let mut out: Vec<ModelInfo> = arr
            .iter()
            .filter_map(|m| {
                let name = m["name"].as_str()?;
                let id = name.strip_prefix("models/").unwrap_or(name).to_string();
                Some(ModelInfo {
                    id,
                    owned_by: m["displayName"].as_str().unwrap_or("").to_string(),
                })
            })
            .collect();
        if !out.is_empty() {
            out.sort_by(|a, b| a.id.cmp(&b.id));
            return Ok(out);
        }
    }

    if saw_container {
        return Ok(Vec::new());
    }

    Err(format!(
        "无法解析模型列表，响应片段：{}",
        truncate(&body.to_string(), 300)
    ))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let t: String = s.chars().take(max).collect();
    format!("{}…", t)
}

/// 查询 AI，支持多张图片
pub async fn query_ai_multi(config: &AppConfig, base64_images: &[String]) -> Result<String, String> {
    let client = build_client()?;

    let res = if config.api_protocol == "gemini" {
        // Gemini 原生协议 - 支持多图
        let mut parts: Vec<Value> = vec![json!({ "text": &config.prompt })];

        for (i, img) in base64_images.iter().enumerate() {
            parts.push(json!({ "text": format!("\n[图片 {}]", i + 1) }));
            parts.push(json!({
                "inline_data": {
                    "mime_type": "image/jpeg",
                    "data": img
                }
            }));
        }

        let payload = json!({
            "contents": [
                {
                    "parts": parts
                }
            ]
        });

        client
            .post(&config.api_endpoint)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("User-Agent", "SnipAns/0.1.5")
            .header("x-goog-api-key", &config.api_key)
            .json(&payload)
            .send()
            .await
    } else {
        // OpenAI 兼容协议 - 支持多图
        let mut content: Vec<Value> = vec![json!({
            "type": "text",
            "text": &config.prompt
        })];

        for (i, img) in base64_images.iter().enumerate() {
            content.push(json!({
                "type": "text",
                "text": format!("\n[图片 {}]", i + 1)
            }));
            content.push(json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:image/jpeg;base64,{}", img)
                }
            }));
        }

        let payload = json!({
            "messages": [
                {
                    "role": "user",
                    "content": content
                }
            ],
            "model": config.model_name,
            "max_tokens": 8192
        });

        client
            .post(&config.api_endpoint)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("User-Agent", "SnipAns/0.1.5")
            .header("Authorization", format!("Bearer {}", config.api_key))
            .json(&payload)
            .send()
            .await
    };

    let res = res.map_err(|e| {
        format!(
            "请求失败：{}。请检查网络与端点地址；若为本地服务，可尝试用 127.0.0.1 代替 localhost。",
            e
        )
    })?;

    if !res.status().is_success() {
        let status = res.status();
        let error_text = res.text().await.unwrap_or_default();
        return Err(format!(
            "接口返回错误：HTTP {} — {}",
            status.as_u16(),
            truncate(&error_text, 500)
        ));
    }

    let body: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("响应不是合法 JSON：{}", e))?;

    extract_answer(&body).ok_or_else(|| truncated_by_token_limit(&body).unwrap_or_else(|| {
        format!(
            "无法从响应中解析答案，响应片段：{}",
            truncate(&body.to_string(), 500)
        )
    }))
}

/// 推理模型会把 token 先花在思考上，`max_tokens` 不够时 `content` 为空、
/// `finish_reason` 为 `length`。这种情况要给出可操作的提示，
/// 而不是让用户对着「无法解析响应」猜。
fn truncated_by_token_limit(body: &serde_json::Value) -> Option<String> {
    let choice = &body["choices"][0];
    if choice["finish_reason"].as_str() != Some("length") {
        return None;
    }
    let has_reasoning = choice["message"]["reasoning"]
        .as_str()
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    Some(if has_reasoning {
        "模型把输出额度用在了思考过程上，答案被截断。请在设置里提高「最大输出 Token」，或改用非推理模型。".to_string()
    } else {
        "模型输出被截断（达到最大 Token 上限），请提高「最大输出 Token」后重试。".to_string()
    })
}

/// 兼容旧的单图接口
#[allow(dead_code)]
pub async fn query_ai(config: &AppConfig, base64_image: &str) -> Result<String, String> {
    query_ai_multi(config, &[base64_image.to_string()]).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_openai_string_content() {
        let body = json!({"choices":[{"message":{"content":"答案 A"}}]});
        assert_eq!(extract_answer(&body).as_deref(), Some("答案 A"));
    }

    #[test]
    fn extracts_openai_array_content() {
        let body = json!({"choices":[{"message":{"content":[{"type":"text","text":"答案"},{"type":"text","text":" B"}]}}]});
        assert_eq!(extract_answer(&body).as_deref(), Some("答案 B"));
    }

    #[test]
    fn extracts_gemini_parts() {
        let body = json!({"candidates":[{"content":{"parts":[{"text":"第一段"},{"text":"第二段"}]}}]});
        assert_eq!(extract_answer(&body).as_deref(), Some("第一段第二段"));
    }

    #[test]
    fn extracts_wrapped_response() {
        let body = json!({"response":{"choices":[{"message":{"content":"包装答案"}}]}});
        assert_eq!(extract_answer(&body).as_deref(), Some("包装答案"));
    }

    #[test]
    fn returns_none_on_unknown_shape() {
        let body = json!({"foo":"bar"});
        assert_eq!(extract_answer(&body), None);
    }

    #[test]
    fn parses_openai_model_list() {
        let body = json!({"data":[{"id":"deepseek-reasoner","owned_by":"deepseek"},{"id":"deepseek-chat","owned_by":"deepseek"}]});
        let models = parse_models(&body).unwrap();
        assert_eq!(models.len(), 2);
        // 结果应已排序
        assert_eq!(models[0].id, "deepseek-chat");
        assert_eq!(models[0].owned_by, "deepseek");
    }

    #[test]
    fn parses_gemini_model_list() {
        let body = json!({"models":[{"name":"models/gemini-2.0-flash","displayName":"Gemini 2.0 Flash"}]});
        let models = parse_models(&body).unwrap();
        assert_eq!(models[0].id, "gemini-2.0-flash");
    }

    #[test]
    fn rejects_unknown_model_list_shape() {
        let body = json!({"unexpected":true});
        assert!(parse_models(&body).is_err());
    }

    #[test]
    fn accepts_empty_model_list_as_empty_result() {
        let body = json!({"data":[]});
        assert_eq!(parse_models(&body).unwrap().len(), 0);
    }

    #[test]
    fn explains_truncation_when_reasoning_ate_the_budget() {
        let body = json!({
            "choices":[{"finish_reason":"length","message":{"content":"","reasoning":"思考了很久"}}]
        });
        let msg = truncated_by_token_limit(&body).expect("应识别为截断");
        assert!(msg.contains("思考过程"), "实际：{}", msg);
    }

    #[test]
    fn no_truncation_error_on_normal_stop() {
        let body = json!({"choices":[{"finish_reason":"stop","message":{"content":"答案"}}]});
        assert!(truncated_by_token_limit(&body).is_none());
    }

    /// 启动一个只回一次请求的本地服务，把收到的请求行与鉴权头回传给调用方，
    /// 用于验证 fetch_models 真正发出的 URL 与 Authorization。
    fn serve_once(
        status_line: &str,
        body: &str,
    ) -> (String, std::sync::mpsc::Receiver<String>) {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("绑定本地端口失败");
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let status_line = status_line.to_string();
        let body = body.to_string();

        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let _ = tx.send(req);
                let resp = format!(
                    "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    status_line,
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
            }
        });

        (format!("http://{}", addr), rx)
    }

    #[tokio::test]
    async fn fetches_models_over_real_http_with_bearer_auth() {
        let (base, rx) = serve_once(
            "200 OK",
            r#"{"data":[{"id":"deepseek-reasoner"},{"id":"deepseek-chat"}]}"#,
        );

        let config = AppConfig {
            provider: "deepseek".to_string(),
            api_endpoint: format!("{}/chat/completions", base),
            api_key: "sk-test-123".to_string(),
            ..Default::default()
        };

        let models = fetch_models(&config, None, None).await.expect("应成功取到模型");
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "deepseek-chat");

        let req = rx.recv().expect("服务端应收到请求");
        // 端点由 /chat/completions 推导为 /models
        assert!(
            req.starts_with("GET /models "),
            "实际请求行：{}",
            req.lines().next().unwrap_or("")
        );
        assert!(
            req.to_lowercase().contains("authorization: bearer sk-test-123"),
            "缺少正确的鉴权头：{}",
            req
        );
    }

    #[tokio::test]
    async fn surfaces_http_error_with_status_code() {
        let (base, _rx) = serve_once("401 Unauthorized", "Authentication Fails");
        let config = AppConfig {
            provider: "deepseek".to_string(),
            api_endpoint: format!("{}/chat/completions", base),
            api_key: "bad".to_string(),
            ..Default::default()
        };

        let err = fetch_models(&config, None, None).await.unwrap_err();
        assert!(err.contains("401"), "错误信息应包含状态码：{}", err);
    }

    #[tokio::test]
    async fn rejects_empty_model_list() {
        let (base, _rx) = serve_once("200 OK", r#"{"data":[]}"#);
        let config = AppConfig {
            provider: "deepseek".to_string(),
            api_endpoint: format!("{}/chat/completions", base),
            api_key: "sk".to_string(),
            ..Default::default()
        };

        let err = fetch_models(&config, None, None).await.unwrap_err();
        assert!(err.contains("未返回任何可用模型"), "实际：{}", err);
    }

    /// 对真实 OpenDesign 上游的联调测试。默认忽略，避免把凭据写进仓库。
    /// 手动执行：
    ///   SNIPANS_OPENDESIGN_KEY=<runtimeKey> cargo test live_opendesign -- --ignored
    #[tokio::test]
    #[ignore]
    async fn live_opendesign_lists_models() {
        let Ok(key) = std::env::var("SNIPANS_OPENDESIGN_KEY") else {
            eprintln!("未设置 SNIPANS_OPENDESIGN_KEY，跳过");
            return;
        };
        let config = AppConfig {
            provider: "opendesign".to_string(),
            api_endpoint: "https://amr-link.open-design.ai/v1/chat/completions".to_string(),
            api_key: key,
            ..Default::default()
        };

        let models = fetch_models(&config, None, None)
            .await
            .expect("应能从 OpenDesign 拉到模型");
        assert!(!models.is_empty(), "上游应返回非空模型列表");
        eprintln!("OpenDesign 返回 {} 个模型", models.len());
    }
}
