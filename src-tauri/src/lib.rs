pub mod api;
pub mod config;
pub mod provider;
pub mod screenshot;
pub mod stealth;

use arboard::Clipboard;
use tauri::Emitter;
use tauri::Manager;

#[tauri::command]
fn notify_font_size(app: tauri::AppHandle, size: u32) -> Result<(), String> {
    app.emit_to("overlay", "font-size-change", size)
        .map_err(|e| format!("通知悬浮窗字号失败：{}", e))
}

/// 读取配置（转发到 config 模块，保持命令在同一模块内以便宏展开）。
#[tauri::command]
fn get_config(app: tauri::AppHandle) -> config::AppConfig {
    config::load_config(&app)
}

/// 保存配置。
#[tauri::command]
fn update_config(app: tauri::AppHandle, config: config::AppConfig) -> Result<(), String> {
    config::save_config(&app, &config)
}

#[tauri::command]
fn notify_overlay_style(app: tauri::AppHandle, style: String) -> Result<(), String> {
    app.emit_to("overlay", "style-change", style)
        .map_err(|e| format!("通知悬浮窗样式失败：{}", e))
}

/// 返回供应商预设列表，供设置界面渲染下拉框。
#[tauri::command]
fn get_providers() -> Vec<provider::ProviderInfo> {
    provider::all_infos()
}

/// 从上游拉取模型列表。endpoint/key 为空时回落到已保存配置，
/// 这样用户改完输入框不必先保存就能「获取模型」。
#[tauri::command]
async fn fetch_models(
    app: tauri::AppHandle,
    provider: String,
    endpoint: Option<String>,
    api_key: Option<String>,
) -> Result<Vec<api::ModelInfo>, String> {
    let mut config = config::load_config(&app);
    config.provider = provider;

    let endpoint = endpoint.filter(|s| !s.trim().is_empty());
    let key = api_key.filter(|s| !s.trim().is_empty());

    if let Some(ep) = &endpoint {
        config.api_endpoint = ep.clone();
    }
    if let Some(k) = &key {
        config.api_key = k.clone();
    }

    if config.api_key.trim().is_empty() {
        return Err("请先填写 API Key，再获取模型列表".to_string());
    }
    if config.api_endpoint.trim().is_empty() {
        return Err("请先填写 API 端点，再获取模型列表".to_string());
    }

    api::fetch_models(&config, endpoint, key).await
}

use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tauri::image::Image;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
};

// 使用全局静态变量来跟踪热键 ID
static SCREENSHOT_HOTKEY_ID: AtomicU32 = AtomicU32::new(0);
static BOSS_HOTKEY_ID: AtomicU32 = AtomicU32::new(0);
static SEND_HOTKEY_ID: AtomicU32 = AtomicU32::new(0);

// 图片缓存
struct ImageCache {
    images: Vec<String>,
}

/// 截图缓存上限
const MAX_CACHED_IMAGES: usize = 8;

/// WebView2 默认把用户数据写到应用数据目录下，若该目录不可写，
/// 创建窗口会以 os error 5（拒绝访问）失败并直接 panic。
/// 这里在窗口创建前探测一次，不可写就退回一个确定可写的位置。
fn ensure_webview2_data_dir() {
    if std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_some() {
        return;
    }

    let candidate = dirs_app_data().join("EBWebView");

    match std::fs::create_dir_all(&candidate) {
        Ok(()) => {
            let probe = candidate.join(".write-probe");
            match std::fs::write(&probe, b"ok") {
                Ok(()) => {
                    let _ = std::fs::remove_file(&probe);
                    std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &candidate);
                    return;
                }
                Err(e) => eprintln!(
                    "应用数据目录 {} 写入探测失败：{}",
                    candidate.display(),
                    e
                ),
            }
        }
        Err(e) => eprintln!(
            "应用数据目录 {} 创建失败：{}",
            candidate.display(),
            e
        ),
    }

    // 应用数据目录不可写时退到临时目录，至少保证能启动
    let fallback = std::env::temp_dir().join("snip-ans-webview2");
    if std::fs::create_dir_all(&fallback).is_ok() {
        eprintln!(
            "应用数据目录不可写，WebView2 数据改存到 {}",
            fallback.display()
        );
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &fallback);
    }
}

/// 返回 `%LOCALAPPDATA%\com.cccake.snip-ans`，失败时退回临时目录。
fn dirs_app_data() -> std::path::PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("com.cccake.snip-ans")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    ensure_webview2_data_dir();

    // 初始化图片缓存
    let image_cache = Arc::new(Mutex::new(ImageCache { images: Vec::new() }));
    let image_cache_clone = image_cache.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_config,
            update_config,
            notify_overlay_style,
            notify_font_size,
            get_providers,
            fetch_models
        ])
        .setup(move |app| {
            // --- 系统托盘 ---
            let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>).unwrap();
            let settings_i =
                MenuItem::with_id(app, "settings", "设置", true, None::<&str>).unwrap();
            let menu = Menu::with_items(app, &[&settings_i, &quit_i]).unwrap();

            // 显式加载图标，避免透明/缺图问题
            let icon_bytes = include_bytes!("../icons/icon.png");
            let image_buffer = image::load_from_memory(icon_bytes)
                .expect("加载托盘图标失败")
                .to_rgba8();
            let (width, height) = image_buffer.dimensions();
            let icon = Image::new(image_buffer.as_raw(), width, height);

            let tray_result = TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "settings" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
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
                .build(app);

            match tray_result {
                Ok(_) => println!("托盘图标已创建"),
                Err(e) => {
                    // 托盘失败不该导致应用整体起不来。此时用户没有托盘入口，
                    // 直接把设置窗口显示出来，否则第一次运行将无处可配置。
                    eprintln!("创建托盘图标失败：{}", e);
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
            }

            // --- 全局快捷键 ---
            let app_handle = app.handle().clone();
            let config = config::load_config(app.handle());
            let manager = GlobalHotKeyManager::new().expect("初始化全局快捷键管理器失败");

            // 三个动作的键位若互相重复，后注册的必然失败，先做一次提示
            if config.hotkey == config.boss_key
                || config.hotkey == config.send_key
                || config.boss_key == config.send_key
            {
                report_hotkey_failure(
                    app.handle(),
                    &format!(
                        "快捷键配置重复（截图 {} / 老板键 {} / 发送 {}），请到设置中改成不同的组合。",
                        config.hotkey, config.boss_key, config.send_key
                    ),
                );
            }

            // 注册截图热键
            match HotKey::from_str(&config.hotkey) {
                Ok(hotkey) => match manager.register(hotkey) {
                    Ok(_) => {
                        SCREENSHOT_HOTKEY_ID.store(hotkey.id(), Ordering::SeqCst);
                        println!("已注册截图快捷键：{}", config.hotkey);
                    }
                    Err(e) => report_hotkey_failure(
                        app.handle(),
                        &format!(
                            "截图快捷键 {} 注册失败（可能被其他程序占用）：{:?}",
                            config.hotkey, e
                        ),
                    ),
                },
                Err(e) => report_hotkey_failure(
                    app.handle(),
                    &format!("截图快捷键 {} 格式无效：{:?}", config.hotkey, e),
                ),
            }

            // 注册老板键
            match HotKey::from_str(&config.boss_key) {
                Ok(hotkey) => match manager.register(hotkey) {
                    Ok(_) => {
                        BOSS_HOTKEY_ID.store(hotkey.id(), Ordering::SeqCst);
                        println!("已注册老板键：{}", config.boss_key);
                    }
                    Err(e) => report_hotkey_failure(
                        app.handle(),
                        &format!(
                            "老板键 {} 注册失败（可能被其他程序占用）：{:?}",
                            config.boss_key, e
                        ),
                    ),
                },
                Err(e) => report_hotkey_failure(
                    app.handle(),
                    &format!("老板键 {} 格式无效：{:?}", config.boss_key, e),
                ),
            }

            // 注册发送键
            match HotKey::from_str(&config.send_key) {
                Ok(hotkey) => match manager.register(hotkey) {
                    Ok(_) => {
                        SEND_HOTKEY_ID.store(hotkey.id(), Ordering::SeqCst);
                        println!("已注册发送快捷键：{}", config.send_key);
                    }
                    Err(e) => report_hotkey_failure(
                        app.handle(),
                        &format!(
                            "发送快捷键 {} 注册失败（可能被其他程序占用）：{:?}",
                            config.send_key, e
                        ),
                    ),
                },
                Err(e) => report_hotkey_failure(
                    app.handle(),
                    &format!("发送快捷键 {} 格式无效：{:?}", config.send_key, e),
                ),
            }

            // 管理器的所有权必须活到进程结束
            Box::leak(Box::new(manager));

            let image_cache = image_cache_clone.clone();

            tauri::async_runtime::spawn(async move {
                let receiver = GlobalHotKeyEvent::receiver();
                while let Ok(event) = receiver.recv() {
                    if event.state != HotKeyState::Released {
                        continue;
                    }

                    let app = app_handle.clone();
                    let screenshot_id = SCREENSHOT_HOTKEY_ID.load(Ordering::SeqCst);
                    let boss_id = BOSS_HOTKEY_ID.load(Ordering::SeqCst);
                    let send_id = SEND_HOTKEY_ID.load(Ordering::SeqCst);

                    if event.id == screenshot_id {
                        // 截图热键触发 - 只负责截图并缓存
                        log_to_main(&app, "已触发截图快捷键");

                        // 截图前先把悬浮窗收起来，原因有两点：
                        //   1. 它固定在屏幕左上角，会挡住题干；
                        //   2. 它开了 WDA_EXCLUDEFROMCAPTURE，该区域在截图里
                        //      是黑的，会把题目内容挖掉一块。
                        // 截完再恢复原来的可见状态。
                        let hwnd = overlay_hwnd(&app);
                        let was_visible = stealth::is_really_visible(hwnd);
                        if was_visible {
                            stealth::hide_window(hwnd);
                            // 给 DWM 一点时间把窗口真正从画面上撤掉
                            std::thread::sleep(std::time::Duration::from_millis(120));
                        }

                        let shot = screenshot::capture_primary_screen();

                        if was_visible {
                            reveal_overlay(&app);
                        }

                        match shot {
                            Ok(base64_img) => {
                                let count = {
                                    let mut cache = image_cache.lock().unwrap();
                                    if cache.images.len() >= MAX_CACHED_IMAGES {
                                        None
                                    } else {
                                        cache.images.push(base64_img);
                                        Some(cache.images.len())
                                    }
                                };

                                match count {
                                    None => {
                                        println!("缓存已满，忽略本次截图。");
                                        show_overlay_message(
                                            &app,
                                            "已达到最大图片数量 (8/8)，请按发送键开始识别。",
                                        );
                                    }
                                    Some(count) => {
                                        println!(
                                            "截图已缓存。当前 {}/{}",
                                            count, MAX_CACHED_IMAGES
                                        );
                                        log_to_main(
                                            &app,
                                            &format!(
                                                "截图已缓存。当前 {}/{}",
                                                count, MAX_CACHED_IMAGES
                                            ),
                                        );
                                        show_overlay_message(
                                            &app,
                                            &format!("已缓存【{}/8】", count),
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                println!("截图失败：{}", e);
                                log_to_main(&app, &format!("截图失败：{}", e));
                            }
                        }
                    } else if event.id == send_id {
                        // 发送键触发 - 发送所有缓存图片
                        let app_clone = app.clone();
                        let cache_clone = image_cache.clone();

                        // 这里不能用 tauri::async_runtime::spawn：外层接收循环
                        // 用 crossbeam 的 recv() 同步阻塞，会长时间占住一个 worker
                        // 线程，导致这里排队的任务一直轮不到执行（表现为截图键
                        // 正常、发送键毫无反应）。改用独立的 OS 线程更稳妥。
                        std::thread::spawn(move || {
                            let images_to_send = {
                                let cache = cache_clone.lock().unwrap();
                                let images = cache.images.clone();
                                if images.is_empty() {
                                    drop(cache);
                                    show_overlay_message(&app_clone, "没有缓存的图片，请先截图。");
                                    return;
                                }
                                images
                            };

                            let config = config::load_config(&app_clone);

                            log_to_main(
                                &app_clone,
                                &format!("正在发送 {} 张图片到 API…", images_to_send.len()),
                            );

                            let use_overlay =
                                config.output_mode == "overlay" || config.output_mode == "both";
                            let use_clipboard = config.output_mode == "clipboard"
                                || config.output_mode == "both";

                            if use_overlay {
                                if let Some(overlay) = app_clone.get_webview_window("overlay") {
                                    let _ = overlay.unminimize();
                                    let _ = overlay.show();
                                    let _ = app_clone.emit_to("overlay", "loading-start", ());
                                }
                            }

                            match tauri::async_runtime::block_on(api::query_ai_multi(
                                &config,
                                &images_to_send,
                            )) {
                                Ok(answer) => {
                                    // 只有成功才清空缓存，失败时保留以便重试
                                    {
                                        let mut cache = cache_clone.lock().unwrap();
                                        cache.images.clear();
                                    }

                                    println!("已收到答案（长度 {} 字符）", answer.chars().count());
                                    log_to_main(&app_clone, "已收到答案。");

                                    if use_clipboard {
                                        match Clipboard::new() {
                                            Ok(mut clipboard) => {
                                                let _ = clipboard.set_text(answer.clone());
                                                println!("已复制到剪贴板。");
                                            }
                                            Err(e) => {
                                                eprintln!("访问剪贴板失败：{}", e);
                                                log_to_main(
                                                    &app_clone,
                                                    &format!("访问剪贴板失败：{}", e),
                                                );
                                            }
                                        }
                                    }

                                    if use_overlay {
                                        let _ = app_clone.emit_to("overlay", "loading-end", ());
                                        let _ =
                                            app_clone.emit_to("overlay", "new-answer", &answer);
                                    }
                                }
                                Err(e) => {
                                    // 失败时把图片放回缓存，用户可直接重试
                                    {
                                        let mut cache = cache_clone.lock().unwrap();
                                        let mut restored: Vec<String> = Vec::with_capacity(
                                            images_to_send.len() + cache.images.len(),
                                        );
                                        restored.extend(images_to_send.iter().cloned());
                                        restored.append(&mut cache.images);
                                        if restored.len() > MAX_CACHED_IMAGES {
                                            let dropped = restored.len() - MAX_CACHED_IMAGES;
                                            eprintln!("缓存已满，丢弃 {} 张较早的截图", dropped);
                                            restored.truncate(MAX_CACHED_IMAGES);
                                        }
                                        let total = restored.len();
                                        cache.images = restored;
                                        println!("已保留 {} 张截图供重试", total);
                                    }

                                    println!("接口或处理错误：{}", e);
                                    log_to_main(&app_clone, &format!("接口或处理错误：{}", e));

                                    if use_overlay {
                                        let _ = app_clone.emit_to("overlay", "loading-end", ());
                                        let _ = app_clone.emit_to(
                                            "overlay",
                                            "new-answer",
                                            format!("出错：{}\n\n截图已保留，可按发送键重试。", e),
                                        );
                                    }
                                }
                            }
                        });
                    } else if event.id == boss_id {
                        // 老板键触发 - 切换悬浮窗显示/隐藏
                        log_to_main(&app, "已触发老板键，正在切换悬浮窗");

                        // 必须问 Win32 的真实状态：我们用 SW_SHOWNA 自己显示
                        // 窗口，tao 内部缓存的 WindowFlags 不会更新，
                        // 用 overlay.is_visible() 会一直判成「隐藏」。
                        let hwnd = overlay_hwnd(&app);
                        let visible = stealth::is_really_visible(hwnd);

                        if visible {
                            // 同样不能调用 overlay.hide()：tao 内部还认为窗口
                            // 是隐藏的（我们的 SW_SHOWNA 它看不见），
                            // apply_diff 判定「无变化」就直接返回，收不起来。
                            stealth::hide_window(hwnd);
                            println!("悬浮窗已隐藏");
                        } else {
                            reveal_overlay(&app);
                            println!("悬浮窗已显示");
                        }
                    }
                }
            });

            // --- 悬浮窗隐身 ---
            // 「不抢焦点」由 tauri.conf.json 的 "focusable": false 在建窗时就
            // 设好了（运行时补会晚于 ShowWindow，实测无效）。这里只把启动时
            // 就要显示的窗口按同样的隐身流程亮出来。
            let config = config::load_config(app.handle());
            if config.output_mode == "overlay" || config.output_mode == "both" {
                reveal_overlay(app.handle());
                println!("悬浮窗已启用隐身：不抢焦点、不进 Alt+Tab、录屏/截图拍不到");
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用失败");
}

/// 向设置窗口写一条日志（窗口可能尚未创建，失败时忽略）。
fn log_to_main(app: &tauri::AppHandle, msg: &str) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.emit("log-message", msg.to_string());
    }
}

/// 取悬浮窗的原生句柄。拿不到就返回 0，调用方当作「不做隐身处理」。
fn overlay_hwnd(app: &tauri::AppHandle) -> isize {
    app.get_webview_window("overlay")
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as isize)
        .unwrap_or(0)
}

/// 让悬浮窗可见，且**不夺走前台焦点**、不进入 Alt+Tab、不被屏幕捕获。
///
/// 原先直接用 `overlay.show()` 会激活窗口 —— 监控软件只要轮询
/// `GetForegroundWindow()` 就能发现「前台突然变成了 snip-ans」。
///
/// 「不抢焦点」靠 tauri.conf.json 里的 `"focusable": false` 在**建窗时**就
/// 带上 `WS_EX_NOACTIVATE` 解决。不能靠运行时补：tao 的 apply_diff 是先
/// `ShowWindow(SW_SHOW)` 把窗口亮出来，之后才重建扩展样式，等我们发现时
/// 焦点早被抢走了。
///
/// 剩下两项有各自的时机要求：
///   * `SetWindowDisplayAffinity` 对**隐藏**窗口调用会失败，必须 show 之后；
///   * tao 每次 show/hide 都整份重建扩展样式，`WS_EX_TOOLWINDOW` 会被冲掉，
///     所以也要在 show 之后补。
fn reveal_overlay(app: &tauri::AppHandle) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        let hwnd = overlay_hwnd(app);

        // 注意：这里**不能**调用 overlay.show() 或 overlay.unminimize()。
        //   * show() 内部是 ShowWindow(SW_SHOW)，会激活窗口抢走前台；
        //   * unminimize() 走 SW_RESTORE，同样会激活窗口。
        // 两者都会在隐身样式生效前就把「前台进程」暴露成 snip-ans。
        // 这里直接用 Win32 完成「挂样式 → 不激活显示」。
        if !stealth::reveal_stealth(hwnd) {
            // 拿不到句柄（异常情况）就退回 tauri 的普通 show，
            // 至少保证窗口能显示出来，只是会抢一次焦点。
            let _ = overlay.show();
            return;
        }

        println!(
            "[隐身] ExStyle=0x{:X}  NOACTIVATE={}  TOOLWINDOW={}",
            stealth::ex_style(hwnd),
            stealth::has_noactivate(hwnd),
            stealth::has_toolwindow(hwnd)
        );
    }
}

/// 在悬浮窗上显示一条短消息（用于缓存进度、错误提示）。
fn show_overlay_message(app: &tauri::AppHandle, msg: &str) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        let hwnd = overlay_hwnd(app);
        if !stealth::reveal_stealth(hwnd) {
            let _ = overlay.show();
        }
        let _ = app.emit_to("overlay", "new-answer", msg.to_string());
    }
}

/// 热键注册失败时，同时写日志区并弹出设置窗口，避免用户完全无感知。
fn report_hotkey_failure(app: &tauri::AppHandle, msg: &str) {
    eprintln!("{}", msg);
    log_to_main(app, msg);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}
