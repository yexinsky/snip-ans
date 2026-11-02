use serde::{Deserialize, Serialize};

/// 鉴权方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthKind {
    /// Authorization: Bearer <key>
    Bearer,
    /// x-goog-api-key: <key>
    GoogleApiKey,
    /// 无需鉴权（本地网关）
    None,
}

/// 供应商预设（静态表，全部使用 `&'static str`）。
pub struct ProviderPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub protocol: &'static str,
    pub default_endpoint: &'static str,
    pub models_endpoint: &'static str,
    pub auth: AuthKind,
    pub default_model: &'static str,
    pub editable_endpoint: bool,
    pub hint: &'static str,
}

/// 传给前端的供应商信息（拥有所有权的字符串，便于序列化）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub label: String,
    pub protocol: String,
    pub default_endpoint: String,
    pub models_endpoint: String,
    pub auth: String,
    pub default_model: String,
    pub editable_endpoint: bool,
    pub hint: String,
}

impl From<&ProviderPreset> for ProviderInfo {
    fn from(p: &ProviderPreset) -> Self {
        ProviderInfo {
            id: p.id.to_string(),
            label: p.label.to_string(),
            protocol: p.protocol.to_string(),
            default_endpoint: p.default_endpoint.to_string(),
            models_endpoint: p.models_endpoint.to_string(),
            auth: p.auth.as_str().to_string(),
            default_model: p.default_model.to_string(),
            editable_endpoint: p.editable_endpoint,
            hint: p.hint.to_string(),
        }
    }
}

impl AuthKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthKind::Bearer => "bearer",
            AuthKind::GoogleApiKey => "google_api_key",
            AuthKind::None => "none",
        }
    }
}

pub const PRESETS: &[ProviderPreset] = &[
    ProviderPreset {
        id: "openai",
        label: "OpenAI 官方",
        protocol: "openai",
        default_endpoint: "https://api.openai.com/v1/chat/completions",
        models_endpoint: "https://api.openai.com/v1/models",
        auth: AuthKind::Bearer,
        default_model: "gpt-4o",
        editable_endpoint: true,
        hint: "使用 OpenAI 官方接口，需要 sk- 开头的 API Key。",
    },
    ProviderPreset {
        id: "deepseek",
        label: "DeepSeek 官方",
        protocol: "openai",
        default_endpoint: "https://api.deepseek.com/chat/completions",
        models_endpoint: "https://api.deepseek.com/models",
        auth: AuthKind::Bearer,
        default_model: "deepseek-chat",
        editable_endpoint: true,
        hint: "DeepSeek 官方接口（OpenAI 兼容）。填写 DeepSeek 开放平台申请的 API Key，点「获取模型」可拉取账号可用模型。",
    },
    ProviderPreset {
        id: "opendesign",
        label: "OpenDesign 云端",
        protocol: "openai",
        // 默认指向官方 Link 网关；用户若走自建隧道可在设置里改。
        default_endpoint: "https://amr-link.open-design.ai/v1/chat/completions",
        models_endpoint: "https://amr-link.open-design.ai/v1/models",
        auth: AuthKind::Bearer,
        // 已验证支持图片输入，适合截图识别
        default_model: "deepseek-v4.1-flash",
        editable_endpoint: true,
        hint: "OpenDesign 云端（OpenAI 形状）。Key 填写 vela 登录后生成的 runtimeKey（见 ~/.amr/config.json 的 profiles.prod.runtimeKey）；端点如非官方网关，请填 <linkUrl>/v1/chat/completions。",
    },
    ProviderPreset {
        id: "gemini",
        label: "Google Gemini",
        protocol: "gemini",
        default_endpoint: "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent",
        models_endpoint: "https://generativelanguage.googleapis.com/v1beta/models",
        auth: AuthKind::GoogleApiKey,
        default_model: "gemini-2.0-flash",
        editable_endpoint: true,
        hint: "Google Gemini 原生协议，使用 x-goog-api-key 鉴权。",
    },
    ProviderPreset {
        id: "custom",
        label: "自定义 / 兼容网关",
        protocol: "openai",
        default_endpoint: "",
        models_endpoint: "",
        auth: AuthKind::Bearer,
        default_model: "",
        editable_endpoint: true,
        hint: "任意 OpenAI 兼容服务（本地网关、中转站等）。端点需填写完整的 chat/completions 地址。",
    },
];

pub fn find_preset(id: &str) -> Option<&'static ProviderPreset> {
    PRESETS.iter().find(|p| p.id == id)
}

pub fn all_infos() -> Vec<ProviderInfo> {
    PRESETS.iter().map(ProviderInfo::from).collect()
}

/// 推导模型列表地址。
///
/// 预设里的 `models_endpoint` 只在用户仍沿用该预设的官方主机时使用；
/// 一旦用户把端点换成别的地址（自建代理、中转网关），就按用户填的端点反推，
/// 否则会出现「对话走代理、拉模型仍打到官方域名」的错位。
pub fn resolve_models_endpoint(provider: &str, endpoint: &str) -> Option<String> {
    let base = endpoint.trim().trim_end_matches('/');
    let derived = if base.is_empty() {
        None
    } else {
        let stripped = base
            .trim_end_matches("/chat/completions")
            .trim_end_matches("/completions")
            .trim_end_matches("/generateContent")
            .trim_end_matches('/');
        if stripped.is_empty() {
            Some(format!("{}/models", base))
        } else {
            Some(format!("{}/models", stripped))
        }
    };

    if let Some(p) = find_preset(provider) {
        if !p.models_endpoint.is_empty() {
            // 端点留空，或仍指向该预设的官方主机时，沿用预设的模型地址；
            // 用户换成别的主机则按用户端点推导。
            let keep_preset = if base.is_empty() {
                true
            } else {
                match (host_of(p.default_endpoint), host_of(base)) {
                    (Some(official), Some(user)) => official == user,
                    _ => false,
                }
            };
            if keep_preset {
                return Some(p.models_endpoint.to_string());
            }
        }
    }
    derived
}

/// 取出 URL 的 host[:port] 部分，用于判断是否仍是同一个服务。
fn host_of(url: &str) -> Option<&str> {
    let rest = url.split("://").nth(1)?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    Some(&rest[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deepseek_has_official_models_endpoint() {
        let p = find_preset("deepseek").expect("deepseek 预设必须存在");
        assert_eq!(p.protocol, "openai");
        assert_eq!(p.models_endpoint, "https://api.deepseek.com/models");
        assert_eq!(p.auth, AuthKind::Bearer);
    }

    #[test]
    fn opendesign_preset_exists() {
        let p = find_preset("opendesign").expect("opendesign 预设必须存在");
        assert_eq!(p.protocol, "openai");
        assert_eq!(p.auth, AuthKind::Bearer);
        // 端点与模型均已对真实上游验证可用
        assert_eq!(
            p.models_endpoint,
            "https://amr-link.open-design.ai/v1/models"
        );
        assert!(!p.default_model.is_empty(), "应带上默认模型，便于开箱即用");
    }

    #[test]
    fn opendesign_derives_models_endpoint_from_custom_gateway() {
        let url = resolve_models_endpoint(
            "opendesign",
            "https://my-tunnel.example.com/v1/chat/completions",
        );
        assert_eq!(url.as_deref(), Some("https://my-tunnel.example.com/v1/models"));
    }

    #[test]
    fn resolves_deepseek_models_endpoint_from_preset() {
        let url = resolve_models_endpoint("deepseek", "https://api.deepseek.com/chat/completions");
        assert_eq!(url.as_deref(), Some("https://api.deepseek.com/models"));
    }

    #[test]
    fn derives_models_endpoint_for_custom_provider() {
        let url = resolve_models_endpoint("custom", "http://127.0.0.1:8792/v1/chat/completions");
        assert_eq!(url.as_deref(), Some("http://127.0.0.1:8792/v1/models"));
    }

    /// 用户把 DeepSeek 端点改成自建代理后，拉模型必须跟着走代理，
    /// 不能再打到官方的 api.deepseek.com。
    #[test]
    fn follows_user_endpoint_when_host_changed() {
        let url = resolve_models_endpoint(
            "deepseek",
            "http://127.0.0.1:8792/v1/chat/completions",
        );
        assert_eq!(url.as_deref(), Some("http://127.0.0.1:8792/v1/models"));
    }

    #[test]
    fn keeps_official_models_endpoint_for_default_host() {
        let url = resolve_models_endpoint("deepseek", "https://api.deepseek.com/v1/chat/completions");
        assert_eq!(url.as_deref(), Some("https://api.deepseek.com/models"));
    }

    #[test]
    fn returns_none_when_endpoint_missing() {
        assert_eq!(resolve_models_endpoint("custom", "   "), None);
    }
}
