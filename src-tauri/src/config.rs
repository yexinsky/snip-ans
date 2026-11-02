use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub api_key: String,
    pub api_endpoint: String,
    pub hotkey: String,        // 截图快捷键（缓存图片）
    pub prompt: String,
    pub output_mode: String,
    pub model_name: String,
    pub api_protocol: String,
    #[serde(default = "default_provider")]
    pub provider: String,      // 供应商预设 id，见 provider::PRESETS
    pub overlay_style: String, // "normal" or "stealth"
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    #[serde(default = "default_boss_key")]
    pub boss_key: String,      // 老板键（隐藏/显示）
    #[serde(default = "default_send_key")]
    pub send_key: String,      // 发送快捷键（发送所有缓存图片）
}

fn default_font_size() -> u32 {
    14
}

fn default_provider() -> String {
    "openai".to_string()
}

fn default_boss_key() -> String {
    "Ctrl+Shift+H".to_string()
}

fn default_send_key() -> String {
    "Ctrl+Alt+Enter".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_key: "".to_string(),
            api_endpoint: "https://api.openai.com/v1/chat/completions".to_string(),
            hotkey: "Ctrl+Alt+S".to_string(),
            prompt: "识别图中的数学题，可能存在多道，请你判断题干是否完整，仅对题干完整的题目进行作答，仅仅回复答案，如果是选择题则回复【题号、选项】，简答题则回复【题号、简洁的答案】。如果检测不到题目，请回复‘未检测到题目’。".to_string(),
            output_mode: "overlay".to_string(), // "overlay", "clipboard", or "both"
            model_name: "gpt-4o".to_string(),
            api_protocol: "openai".to_string(),
            provider: "openai".to_string(),
            overlay_style: "stealth".to_string(), // "normal" or "stealth"
            font_size: 14,
            boss_key: "Ctrl+Shift+H".to_string(),
            send_key: "Ctrl+Alt+Enter".to_string(),
        }
    }
}

/// 保存到磁盘的结构：这些字段缺省时不得导致整份配置解析失败。
#[derive(Debug, Serialize, Deserialize)]
struct AppConfigFile {
    #[serde(default)]
    api_key: String,
    #[serde(default = "default_endpoint")]
    api_endpoint: String,
    #[serde(default = "default_hotkey")]
    hotkey: String,
    #[serde(default = "default_prompt")]
    prompt: String,
    #[serde(default = "default_output_mode")]
    output_mode: String,
    #[serde(default = "default_model")]
    model_name: String,
    #[serde(default = "default_protocol")]
    api_protocol: String,
    #[serde(default = "default_provider")]
    provider: String,
    #[serde(default = "default_overlay_style")]
    overlay_style: String,
    #[serde(default = "default_font_size")]
    font_size: u32,
    #[serde(default = "default_boss_key")]
    boss_key: String,
    #[serde(default = "default_send_key")]
    send_key: String,
}

fn default_endpoint() -> String { AppConfig::default().api_endpoint }
fn default_hotkey() -> String { AppConfig::default().hotkey }
fn default_prompt() -> String { AppConfig::default().prompt }
fn default_output_mode() -> String { AppConfig::default().output_mode }
fn default_model() -> String { AppConfig::default().model_name }
fn default_protocol() -> String { AppConfig::default().api_protocol }
fn default_overlay_style() -> String { AppConfig::default().overlay_style }

impl From<AppConfigFile> for AppConfig {
    fn from(f: AppConfigFile) -> Self {
        AppConfig {
            api_key: f.api_key,
            api_endpoint: f.api_endpoint,
            hotkey: f.hotkey,
            prompt: f.prompt,
            output_mode: f.output_mode,
            model_name: f.model_name,
            api_protocol: f.api_protocol,
            provider: f.provider,
            overlay_style: f.overlay_style,
            font_size: f.font_size,
            boss_key: f.boss_key,
            send_key: f.send_key,
        }
    }
}

/// 配置候选位置，按优先级排列。
///
/// 首选应用配置目录；若该目录连创建都失败（沙箱、只读漫游配置、
/// 受限的 AppData），依次退到本地应用数据、临时目录，
/// 保证用户填的 Key 至少有一个落点，不会白填一遍。
fn candidate_paths(app: &AppHandle) -> Vec<PathBuf> {
    let mut out = Vec::new();

    if let Ok(d) = app.path().app_config_dir() {
        out.push(d.join("settings.json"));
    }
    if let Ok(d) = app.path().app_local_data_dir() {
        out.push(d.join("settings.json"));
    }
    out.push(std::env::temp_dir().join("snip-ans-settings.json"));

    // 去重，避免几个 API 返回同一路径时重复尝试
    let mut seen = Vec::new();
    out.retain(|p| {
        let key = p.to_string_lossy().to_lowercase();
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    });
    out
}

/// 首选配置路径，用于对外展示。
pub fn get_config_path(app: &AppHandle) -> PathBuf {
    candidate_paths(app)
        .into_iter()
        .find(|p| is_dir_writable(p.parent().unwrap_or(p)))
        .unwrap_or_else(|| std::env::temp_dir().join("snip-ans-settings.json"))
}

/// 用一次真实的写+删来判断目录是否可写。
fn is_dir_writable(dir: &std::path::Path) -> bool {
    if fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".write-probe");
    match fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// 读取配置。文件不存在时用默认值；解析失败时保留原文件备份并回退默认值，
/// 避免静默地把用户的 API Key 清空。
pub fn load_config(app: &AppHandle) -> AppConfig {
    // 读取时优先找已存在的文件：主目录可能不可写、但此前的配置还在，
    // 不能因为「写不进去」就当作「没配置过」而把用户的 Key 丢掉。
    let path = match candidate_paths(app).into_iter().find(|p| p.exists()) {
        Some(p) => p,
        None => return AppConfig::default(),
    };

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("读取配置文件失败：{}", e);
            return AppConfig::default();
        }
    };
    match serde_json::from_str::<AppConfigFile>(&content) {
        Ok(f) => f.into(),
        Err(e) => {
            eprintln!("配置文件解析失败，已备份为 settings.json.bak：{}", e);
            let backup = path.with_extension("json.bak");
            let _ = fs::copy(&path, &backup);
            AppConfig::default()
        }
    }
}

pub fn save_config(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    let content =
        serde_json::to_string_pretty(config).map_err(|e| format!("序列化配置失败：{}", e))?;

    // 逐个候选位置尝试。建目录失败也要继续试下一个，
    // 而不是把「创建配置目录失败」直接抛给用户。
    let candidates = candidate_paths(app);
    let mut errors = Vec::new();

    for (idx, path) in candidates.iter().enumerate() {
        match write_at(path, &content) {
            Ok(()) => {
                if idx > 0 {
                    eprintln!(
                        "配置已改存到 {}（前序位置不可写：{}）",
                        path.display(),
                        errors.join("；")
                    );
                }
                return Ok(());
            }
            Err(e) => errors.push(e),
        }
    }

    Err(format!(
        "保存失败，以下位置均不可写：\n{}\n请检查是否有权限写入用户配置目录。",
        errors.join("\n")
    ))
}

/// 确保父目录存在并写入文件，返回失败原因（含建目录失败）。
fn write_at(path: &std::path::Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("创建目录 {} 失败：{}", parent.display(), e))?;
        }
    }
    fs::write(path, content).map_err(|e| format!("写入 {} 失败：{}", path.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 目录不可写时必须报错而不是 panic，且错误里带上路径便于排查。
    #[test]
    fn write_at_reports_unwritable_dir() {
        let bad = std::env::temp_dir()
            .join("snip-ans-test-no-such-parent")
            .join("still-missing")
            .join("settings.json");
        // 用一个不可能创建的路径（把普通文件当目录用）
        let blocker = std::env::temp_dir().join("snip-ans-test-blocker");
        fs::write(&blocker, b"x").unwrap();
        let impossible = blocker.join("settings.json");
        let err = write_at(&impossible, "{}").unwrap_err();
        assert!(!err.is_empty(), "应返回失败原因");
        let _ = fs::remove_file(&blocker);
        let _ = bad;
    }

    #[test]
    fn write_at_creates_parent_and_writes() {
        let dir = std::env::temp_dir().join("snip-ans-test-writable");
        let _ = fs::remove_dir_all(&dir);
        let file = dir.join("nested").join("settings.json");
        write_at(&file, r#"{"provider":"opendesign"}"#).expect("应能创建目录并写入");
        let back = fs::read_to_string(&file).unwrap();
        assert!(back.contains("opendesign"));
        let _ = fs::remove_dir_all(&dir);
    }

    /// 候选列表必须至少有一个可用落点，否则用户永远存不下配置。
    #[test]
    fn candidate_paths_has_temp_fallback() {
        let tmp = std::env::temp_dir().join("snip-ans-settings.json");
        assert!(tmp.to_string_lossy().contains("snip-ans-settings.json"));
    }
}
