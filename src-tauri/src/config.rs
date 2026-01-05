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
            model_name: "gemini-3-flash-preview".to_string(),
            api_protocol: "openai".to_string(),
            overlay_style: "stealth".to_string(), // "normal" or "stealth"
            font_size: 14,
            boss_key: "Ctrl+Shift+H".to_string(),
            send_key: "Ctrl+Alt+Enter".to_string(),
        }
    }
}

pub fn get_config_path(app: &AppHandle) -> PathBuf {
    app.path().app_config_dir().expect("failed to get app config dir").join("settings.json")
}

#[tauri::command]
pub fn get_config(app: AppHandle) -> AppConfig {
    load_config(&app)
}

#[tauri::command]
pub fn update_config(app: AppHandle, config: AppConfig) -> Result<(), String> {
    save_config(&app, &config)
}

pub fn load_config(app: &AppHandle) -> AppConfig {
    let path = get_config_path(app);
    if path.exists() {
        let content = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        AppConfig::default()
    }
}

pub fn save_config(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    let path = get_config_path(app);
    if let Some(parent) = path.parent() {
        if !parent.exists() {
             fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())?;
    Ok(())
}
