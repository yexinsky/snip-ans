use reqwest::Client;
use serde_json::{json, Value};
use crate::config::AppConfig;

use std::time::Duration;

fn extract_answer(body: &serde_json::Value) -> Option<String> {
    // 1. OpenAI Standard
    if let Some(content) = body["choices"][0]["message"]["content"].as_str() {
        return Some(content.to_string());
    }
    // 2. Gemini Standard
    if let Some(text) = body["candidates"][0]["content"]["parts"][0]["text"].as_str() {
        return Some(text.to_string());
    }
    // 3. Wrapped Gemini (e.g. {"response": {"candidates": ...}})
    if let Some(text) = body["response"]["candidates"][0]["content"]["parts"][0]["text"].as_str() {
        return Some(text.to_string());
    }
    // 4. Wrapped OpenAI
    if let Some(content) = body["response"]["choices"][0]["message"]["content"].as_str() {
        return Some(content.to_string());
    }
    
    None
}

/// 查询 AI，支持多张图片
pub async fn query_ai_multi(config: &AppConfig, base64_images: &[String]) -> Result<String, String> {
    // 增加超时时间到 300 秒以支持推理模型（如 DeepSeek 等需要较长思考时间）
    let client = Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?;
    
    let res = if config.api_protocol == "gemini" {
        // Gemini Native Protocol Request - 支持多图
        let mut parts: Vec<Value> = vec![json!({ "text": &config.prompt })];
        
        for (i, img) in base64_images.iter().enumerate() {
            // 添加图片标签
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
        
        let payload_str = serde_json::to_string(&payload).unwrap_or_default();
        println!("DEBUG: Gemini request payload size: {} bytes ({} KB)", payload_str.len(), payload_str.len() / 1024);

        client.post(&config.api_endpoint)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("User-Agent", "SnipAns/0.1.0")
            .header("x-goog-api-key", &config.api_key)
            .json(&payload)
            .send()
            .await

    } else {
        // OpenAI Compatible Protocol Request - 支持多图
        let mut content: Vec<Value> = vec![json!({
            "type": "text",
            "text": &config.prompt
        })];
        
        for (i, img) in base64_images.iter().enumerate() {
            // 添加图片标签
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
            "max_tokens": 20000
        });
        
        let payload_str = serde_json::to_string(&payload).unwrap_or_default();
        println!("DEBUG: OpenAI request payload size: {} bytes ({} KB)", payload_str.len(), payload_str.len() / 1024);
        
        client.post(&config.api_endpoint)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("User-Agent", "SnipAns/0.1.0")
            .header("Authorization", format!("Bearer {}", config.api_key))
            .json(&payload)
            .send()
            .await
    };

    let res = res.map_err(|e| {
        format!("Request failed: {}. (Hint: Try 127.0.0.1 instead of localhost if local)", e)
    })?;

    if !res.status().is_success() {
        let status = res.status();
        let error_text = res.text().await.unwrap_or_default();
        return Err(format!("API Error: {} - {}", status, error_text));
    }

    let body: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
    
    // Debug logging
    println!("Response Body: {:?}", body); 

    let answer = extract_answer(&body)
        .ok_or_else(|| format!("Failed to parse answer from any known format. JSON: {}", body))?;

    Ok(answer)
}

/// 兼容旧的单图接口
pub async fn query_ai(config: &AppConfig, base64_image: &str) -> Result<String, String> {
    query_ai_multi(config, &[base64_image.to_string()]).await
}
