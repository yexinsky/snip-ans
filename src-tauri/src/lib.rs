pub mod config;
pub mod screenshot;
pub mod api;
use config::*;
use arboard::Clipboard;
use tauri::{Emitter, Listener}; // Listener not strictly needed unless we listen in rust

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn notify_overlay_style(app: tauri::AppHandle, style: String) -> Result<(), String> {
    app.emit_to("overlay", "style-change", style).map_err(|e| e.to_string())
}

#[tauri::command]
fn notify_font_size(app: tauri::AppHandle, size: u32) -> Result<(), String> {
    app.emit_to("overlay", "font-size-change", size).map_err(|e| e.to_string())
}

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tauri::image::Image;
use global_hotkey::{
    hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

// 使用全局静态变量来跟踪热键 ID
static SCREENSHOT_HOTKEY_ID: AtomicU32 = AtomicU32::new(0);
static BOSS_HOTKEY_ID: AtomicU32 = AtomicU32::new(0);
static SEND_HOTKEY_ID: AtomicU32 = AtomicU32::new(0);

// 图片缓存
struct ImageCache {
    images: Vec<String>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化图片缓存
    let image_cache = Arc::new(Mutex::new(ImageCache { images: Vec::new() }));
    let image_cache_clone = image_cache.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, get_config, update_config, notify_overlay_style, notify_font_size])
        .setup(move |app| {
            // --- System Tray Setup ---
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>).unwrap();
            let settings_i = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>).unwrap();
            let menu = Menu::with_items(app, &[&settings_i, &quit_i]).unwrap();

            // Load icon explicitly to avoid transparency/missing icon issues
            let icon_bytes = include_bytes!("../icons/icon.png"); 
            let image_buffer = image::load_from_memory(icon_bytes).expect("Failed to load icon").to_rgba8();
            let (width, height) = image_buffer.dimensions();
            let icon = Image::new(image_buffer.as_raw(), width, height);

            let _tray = TrayIconBuilder::new()
                .icon(icon) 
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "quit" => app.exit(0),
                        "settings" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // --- Global Hotkey Setup ---
            let app_handle = app.handle().clone();
            
            // 加载配置并获取热键 ID
            let config = load_config(app.handle());
            let manager = GlobalHotKeyManager::new().unwrap();
            
            // 注册截图热键
            if let Ok(hotkey) = HotKey::from_str(&config.hotkey) {
                SCREENSHOT_HOTKEY_ID.store(hotkey.id(), Ordering::SeqCst);
                match manager.register(hotkey) {
                    Ok(_) => println!("Registered screenshot hotkey: {} (id: {})", config.hotkey, SCREENSHOT_HOTKEY_ID.load(Ordering::SeqCst)),
                    Err(e) => eprintln!("Warning: Failed to register screenshot hotkey '{}': {:?}", config.hotkey, e),
                }
            } else {
                eprintln!("Warning: Invalid screenshot hotkey format: {}", config.hotkey);
            }
            
            // 注册老板键
            if let Ok(boss_hotkey) = HotKey::from_str(&config.boss_key) {
                BOSS_HOTKEY_ID.store(boss_hotkey.id(), Ordering::SeqCst);
                match manager.register(boss_hotkey) {
                    Ok(_) => println!("Registered boss key: {} (id: {})", config.boss_key, BOSS_HOTKEY_ID.load(Ordering::SeqCst)),
                    Err(e) => eprintln!("Warning: Failed to register boss key '{}': {:?}. Try a different key combination.", config.boss_key, e),
                }
            } else {
                eprintln!("Warning: Invalid boss key format: {}", config.boss_key);
            }

            // 注册发送键
            if let Ok(send_hotkey) = HotKey::from_str(&config.send_key) {
                SEND_HOTKEY_ID.store(send_hotkey.id(), Ordering::SeqCst);
                match manager.register(send_hotkey) {
                    Ok(_) => println!("Registered send key: {} (id: {})", config.send_key, SEND_HOTKEY_ID.load(Ordering::SeqCst)),
                    Err(e) => eprintln!("Warning: Failed to register send key '{}': {:?}. Try a different key combination.", config.send_key, e),
                }
            } else {
                eprintln!("Warning: Invalid send key format: {}", config.send_key);
            }
            
            // Keep manager alive by leaking it
            Box::leak(Box::new(manager));
            
            let image_cache = image_cache_clone.clone();

            tauri::async_runtime::spawn(async move {
                let receiver = GlobalHotKeyEvent::receiver();
                while let Ok(event) = receiver.recv() {
                    if event.state == HotKeyState::Released {
                        let app = app_handle.clone();
                        let screenshot_id = SCREENSHOT_HOTKEY_ID.load(Ordering::SeqCst);
                        let boss_id = BOSS_HOTKEY_ID.load(Ordering::SeqCst);
                        let send_id = SEND_HOTKEY_ID.load(Ordering::SeqCst);
                        
                        if event.id == screenshot_id {
                            // 截图热键触发 - 只负责截图并缓存
                            println!("Screenshot hotkey pressed: {:?}", event);
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.emit("log-message", format!("Screenshot hotkey pressed"));
                            }
                            
                            match screenshot::capture_primary_screen() {
                                Ok(base64_img) => {
                                    let mut cache = image_cache.lock().unwrap();
                                    
                                    // 检查是否达到上限 (8张)
                                    if cache.images.len() >= 8 {
                                        println!("Cache full (8/8). Ignoring new screenshot.");
                                        if let Some(overlay) = app.get_webview_window("overlay") {
                                            let _ = overlay.unminimize();
                                            let _ = overlay.show();
                                            let _ = app.emit_to("overlay", "new-answer", "已达到最大图片数量 (8/8)，请按发送键开始识别。");
                                        }
                                        continue;
                                    }
                                    
                                    cache.images.push(base64_img);
                                    let count = cache.images.len();
                                    println!("Screenshot cached. Total: {}/8", count);
                                    
                                    if let Some(w) = app.get_webview_window("main") {
                                        let _ = w.emit("log-message", format!("Screenshot cached. Total: {}/8", count));
                                    }
                                    
                                    // 在 overlay 显示缓存状态
                                    if let Some(overlay) = app.get_webview_window("overlay") {
                                        let _ = overlay.unminimize();
                                        let _ = overlay.show();
                                        let _ = app.emit_to("overlay", "new-answer", format!("已缓存【{}/8】", count));
                                    }
                                },
                                Err(e) => {
                                    println!("Screenshot Error: {:?}", e);
                                    if let Some(w) = app.get_webview_window("main") {
                                        let _ = w.emit("log-message", format!("Screenshot Error: {:?}", e));
                                    }
                                }
                            }
                        } else if event.id == send_id {
                            // 发送键触发 - 发送所有缓存图片
                            let app_clone = app.clone();
                            let cache_clone = image_cache.clone();
                            
                            // 在独立的spawn中处理，避免阻塞主循环
                            tauri::async_runtime::spawn(async move {
                                println!("Send hotkey pressed");
                                if let Some(w) = app_clone.get_webview_window("main") {
                                    let _ = w.emit("log-message", format!("Send hotkey pressed"));
                                }
                                
                                // 获取并清空缓存
                                let images_to_send = {
                                    let mut cache = cache_clone.lock().unwrap();
                                    let images = cache.images.clone();
                                    let count = images.len();
                                    
                                    println!("DEBUG: Cache has {} images before sending", count);
                                    
                                    if count == 0 {
                                        println!("No images in cache.");
                                        if let Some(overlay) = app_clone.get_webview_window("overlay") {
                                            let _ = overlay.unminimize();
                                            let _ = overlay.show();
                                            let _ = app_clone.emit_to("overlay", "new-answer", "没有缓存的图片，请先截图。");
                                        }
                                        return;
                                    }
                                    
                                    cache.images.clear();
                                    println!("DEBUG: Cache cleared, sending {} images", count);
                                    images
                                }; // 锁在这里被释放
                                
                                let config = load_config(&app_clone);
                                
                                println!("Sending {} images to API...", images_to_send.len());
                                if let Some(w) = app_clone.get_webview_window("main") {
                                    let _ = w.emit("log-message", format!("Sending {} images to API...", images_to_send.len()));
                                }
                                
                                // 发送加载开始事件到 overlay
                                let use_overlay = config.output_mode == "overlay" || config.output_mode == "both";
                                if use_overlay {
                                    if let Some(overlay) = app_clone.get_webview_window("overlay") {
                                        let _ = overlay.unminimize();
                                        let _ = overlay.show();
                                        let _ = app_clone.emit_to("overlay", "loading-start", ());
                                    }
                                }
                                
                                match api::query_ai_multi(&config, &images_to_send).await {
                                    Ok(answer) => {
                                        println!("Answer received (length: {} chars)", answer.len());
                                        if let Some(w) = app_clone.get_webview_window("main") {
                                            let _ = w.emit("log-message", format!("Answer received."));
                                        }
                                        
                                        // Handle output based on mode
                                        let use_clipboard = config.output_mode == "clipboard" || config.output_mode == "both";
                                        
                                        if use_clipboard {
                                            if let Ok(mut clipboard) = Clipboard::new() {
                                                let _ = clipboard.set_text(answer.clone());
                                                println!("Copied to clipboard.");
                                            }
                                        }
                                        
                                        if use_overlay {
                                            if let Some(_overlay) = app_clone.get_webview_window("overlay") {
                                                let _ = app_clone.emit_to("overlay", "loading-end", ());
                                                let _ = app_clone.emit_to("overlay", "new-answer", &answer);
                                                println!("Sent answer to overlay");
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        println!("API/Processing Error: {:?}", e);
                                        if let Some(w) = app_clone.get_webview_window("main") {
                                            let _ = w.emit("log-message", format!("API/Processing Error: {:?}", e));
                                        }
                                        if use_overlay {
                                            let _ = app_clone.emit_to("overlay", "loading-end", ());
                                            let _ = app_clone.emit_to("overlay", "new-answer", format!("Error: {}", e));
                                        }
                                    }
                                }
                            });
                        } else if event.id == boss_id {
                            // 老板键触发 - 切换 overlay 显示/隐藏
                            println!("Boss key pressed: {:?}", event);
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.emit("log-message", "Boss key pressed - toggling overlay".to_string());
                            }
                            
                            if let Some(overlay) = app.get_webview_window("overlay") {
                                if overlay.is_visible().unwrap_or(false) {
                                    let _ = overlay.hide();
                                    println!("Overlay hidden");
                                } else {
                                    let _ = overlay.show();
                                    let _ = overlay.set_focus(); // 尝试获取焦点
                                    println!("Overlay shown");
                                }
                            }
                        }
                    }
                }
            });

            // Show overlay on startup if configured to use overlay
            let config = load_config(app.handle());
            if config.output_mode == "overlay" || config.output_mode == "both" {
                if let Some(overlay) = app.get_webview_window("overlay") {
                    let _ = overlay.show();
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
