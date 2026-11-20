#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod composition_probe;
mod hit_region;

use base64::Engine;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
};
use include_dir::{include_dir, Dir};
use serde_json::{json, Value};
use std::{
    borrow::Cow,
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};
use tao::platform::windows::{MonitorHandleExtWindows, WindowBuilderExtWindows, WindowExtWindows};
use tao::{
    dpi::{LogicalSize, PhysicalPosition},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::{Window, WindowBuilder},
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIconBuilder, TrayIconEvent,
};
use windows_sys::Win32::{
    Foundation::POINT,
    Graphics::Gdi::{CreateRectRgn, DeleteObject, GetMonitorInfoW, GetWindowRgn, MONITORINFO},
    UI::WindowsAndMessaging::{
        GetAncestor, GetWindowLongW, WindowFromPoint, GA_ROOT, GWL_EXSTYLE, WS_EX_TRANSPARENT,
    },
};
use wry::{
    http::{Request, Response},
    NewWindowFeatures, NewWindowOpener, NewWindowResponse, WebContext, WebView, WebViewBuilder,
    WebViewBuilderExtWindows, WebViewExtWindows,
};

static UI: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../dist");

enum UserEvent {
    Ipc(String),
    Popup(NewWindowFeatures, mpsc::SyncSender<Option<NewWindowOpener>>),
    ShowConsole,
    ToggleLock,
    Quit,
    AudioStarted {
        generation: u64,
        request: u64,
    },
    AudioFailed {
        generation: u64,
        request: Option<u64>,
        message: String,
    },
    Pcm {
        generation: u64,
        bytes: Vec<u8>,
        pending: Arc<AtomicUsize>,
    },
}

// Drop webviews before the windows they are attached to.
struct Host {
    view: WebView,
    window: Window,
}

fn log(message: &str) {
    if let Ok(path) = std::env::var("PET_LOG_FILE") {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{message}");
        }
    }
    eprintln!("{message}");
}

fn external_assets() -> PathBuf {
    std::env::current_exe()
        .unwrap_or_default()
        .parent()
        .unwrap_or(Path::new("."))
        .join("character_assets")
}

fn resource(request: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let decoded = percent_encoding::percent_decode_str(request.uri().path()).decode_utf8();
    let path = decoded.as_deref().unwrap_or("").trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    // Reject traversal, Windows drive paths, alternate streams and encoded separators.
    let safe = !path.contains(['\\', ':', '\0'])
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)));
    let mut body = None;
    if safe {
        if let Some(relative) = path.strip_prefix("character_assets/") {
            let root = external_assets();
            if let (Ok(root), Ok(target)) =
                (root.canonicalize(), root.join(relative).canonicalize())
            {
                if target.starts_with(&root) && target.is_file() {
                    body = std::fs::read(target).ok().map(Cow::Owned);
                }
            }
        }
        if body.is_none() {
            body = UI.get_file(path).map(|file| Cow::Borrowed(file.contents()));
        }
    }
    let status = if !safe {
        403
    } else if body.is_some() {
        200
    } else {
        404
    };
    Response::builder()
        .status(status)
        .header(
            "Content-Type",
            mime_guess::from_path(path).first_or_octet_stream().as_ref(),
        )
        .header(
            "Cache-Control",
            if path.starts_with("character_assets/") {
                "no-store"
            } else {
                "public, max-age=3600"
            },
        )
        .header("X-Content-Type-Options", "nosniff")
        .body(body.unwrap_or(Cow::Borrowed(b"Resource not found")))
        .unwrap()
}

fn trusted(url: String) -> bool {
    url == "about:blank"
        || url == "https://pet.localhost/"
        || url.starts_with("https://pet.localhost/")
}

fn reply(view: &WebView, id: u64, result: Result<Value, String>) {
    let (value, error) = match result {
        Ok(value) => (value, Value::Null),
        Err(error) => (Value::Null, json!(error)),
    };
    let _ = view.evaluate_script(&format!("window.__nativeReply?.({id},{value},{error})"));
}

fn show(console: &Option<Host>) {
    if let Some(host) = console {
        host.window.set_visible(true);
        host.window.set_minimized(false);
        host.window.set_focus();
        let _ = host.view.focus();
    }
}

fn lock(pet: &Host, value: bool, state: &mut bool, menu: &MenuItem) -> Result<Value, String> {
    pet.window
        .set_ignore_cursor_events(value)
        .map_err(|error| error.to_string())?;
    *state = value;
    menu.set_text(if value {
        "解除桌宠锁定"
    } else {
        "锁定桌宠（点击穿透）"
    });
    let _ = pet
        .view
        .evaluate_script(&format!("window.__nativeLocked?.({value})"));
    Ok(json!(value))
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // WebView2 reads this before creating its first controller, avoiding a white
    // startup canvas. It changes this process only, never the user's environment.
    std::env::set_var("WEBVIEW2_DEFAULT_BACKGROUND_COLOR", "00000000");
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let data_dir = std::env::var_os("PET_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default())
                .join("DesktopPetRhythmRust")
        });
    std::fs::create_dir_all(&data_dir)?;
    let mut context = WebContext::new(Some(data_dir));
    let window = WindowBuilder::new()
        .with_title("桌宠节奏师")
        .with_inner_size(LogicalSize::new(540., 590.))
        .with_resizable(false)
        .with_decorations(false)
        .with_undecorated_shadow(false)
        .with_transparent(true)
        .with_always_on_top(true)
        .build(&event_loop)?;
    let ipc = proxy.clone();
    let popup = proxy.clone();
    let mut builder = WebViewBuilder::new_with_web_context(&mut context)
        .with_transparent(true)
        .with_background_color((0, 0, 0, 0))
        .with_https_scheme(true)
        .with_custom_protocol("pet".into(), |_, request| resource(request))
        .with_initialization_script(include_str!("bridge.js"))
        .with_autoplay(true)
        .with_devtools(cfg!(debug_assertions) || std::env::var_os("PET_DEBUG_PORT").is_some())
        .with_navigation_handler(trusted)
        .with_ipc_handler(move |request| {
            if request.uri().host() == Some("pet.localhost") {
                let _ = ipc.send_event(UserEvent::Ipc(request.into_body()));
            }
        })
        .with_new_window_req_handler(move |url, features| {
            if url != "about:blank" {
                return NewWindowResponse::Deny;
            }
            let (tx, rx) = mpsc::sync_channel(1);
            if popup.send_event(UserEvent::Popup(features, tx)).is_err() {
                return NewWindowResponse::Deny;
            }
            match rx.recv_timeout(Duration::from_secs(12)) {
                Ok(Some(host)) => NewWindowResponse::Create {
                    webview: host.webview,
                },
                _ => NewWindowResponse::Deny,
            }
        })
        .with_url("https://pet.localhost/index.html");
    // Debugging is opt-in, used only by the local verification script.
    if let Ok(port) = std::env::var("PET_DEBUG_PORT") {
        let port: u16 = port.parse()?;
        builder = builder.with_additional_browser_args(&format!("--remote-debugging-port={port} --autoplay-policy=no-user-gesture-required --disable-background-timer-throttling --disable-renderer-backgrounding"));
    } else {
        builder = builder.with_additional_browser_args("--autoplay-policy=no-user-gesture-required --disable-background-timer-throttling --disable-renderer-backgrounding");
    }
    let pet = Host {
        view: builder.build(&window)?,
        window,
    };
    let mut console: Option<Host> = None;
    let mut composition_probe = composition_probe::CompositionProbe::default();
    let mut locked = false;
    let mut capture: Option<audio::Capture> = None;
    let mut generation = 0u64;
    let mut hit_mask = hit_region::HitRegion::default();
    let mut pointer_check_at = Instant::now();

    let menu = Menu::new();
    let open_item = MenuItem::new("打开控制台", true, None);
    let lock_item = MenuItem::new("锁定桌宠（点击穿透）", true, None);
    let quit_item = MenuItem::new("退出桌宠节奏师", true, None);
    menu.append_items(&[
        &open_item,
        &lock_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ])?;
    let icon = image::load_from_memory(
        UI.get_file("character_assets/head_base.png")
            .ok_or("缺少托盘图标")?
            .contents(),
    )?
    .resize(24, 24, image::imageops::FilterType::Triangle)
    .to_rgba8();
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("桌宠节奏师")
        .with_icon(Icon::from_rgba(icon.into_raw(), 24, 24)?)
        .build()?;
    let menu_proxy = proxy.clone();
    let open_id = open_item.id().clone();
    let lock_id = lock_item.id().clone();
    let quit_id = quit_item.id().clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let action = if event.id == open_id {
            UserEvent::ShowConsole
        } else if event.id == lock_id {
            UserEvent::ToggleLock
        } else if event.id == quit_id {
            UserEvent::Quit
        } else {
            return;
        };
        let _ = menu_proxy.send_event(action);
    }));
    let tray_proxy = proxy.clone();
    TrayIconEvent::set_event_handler(Some(move |event| {
        if matches!(event, TrayIconEvent::DoubleClick { .. }) {
            let _ = tray_proxy.send_event(UserEvent::ShowConsole);
        }
    }));
    let hotkeys = GlobalHotKeyManager::new()?;
    let lock_key = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyL);
    let console_key = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyC);
    // A second running copy may already own the shortcuts; do not prevent startup.
    for key in [lock_key, console_key] {
        if let Err(error) = hotkeys.register(key) {
            log(&format!("快捷键注册：{error}"));
        }
    }
    let key_proxy = proxy.clone();
    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        if event.state != HotKeyState::Pressed {
            return;
        }
        let action = if event.id == lock_key.id() {
            UserEvent::ToggleLock
        } else if event.id == console_key.id() {
            UserEvent::ShowConsole
        } else {
            return;
        };
        let _ = key_proxy.send_event(action);
    }));

    event_loop.run(move |event, target, flow| {
        if Instant::now() >= pointer_check_at {
            let _ = hit_mask.sync_pointer(&pet.window, locked);
            pointer_check_at = Instant::now() + Duration::from_millis(16);
        }
        *flow = ControlFlow::WaitUntil(pointer_check_at);
        // Keep shell resources alive until LoopDestroyed.
        let _keep_alive = (&tray, &hotkeys, &context);
        match event {
            Event::UserEvent(UserEvent::Popup(features, response)) => {
                if let Some(host) = &console {
                    let _ = response.send(Some(NewWindowOpener { webview: host.view.webview(), environment: host.view.environment() }));
                    show(&console);
                    return;
                }
                let result = (|| -> Result<Host, Box<dyn std::error::Error>> {
                    let mut builder = WindowBuilder::new().with_title("桌宠节奏师 · 控制台")
                        .with_inner_size(LogicalSize::new(470., 780.)).with_min_inner_size(LogicalSize::new(430., 500.))
                        .with_decorations(false).with_always_on_top(true);
                    if let (Ok(pos), Some(monitor)) = (pet.window.outer_position(), pet.window.current_monitor()) {
                        let area = monitor.position(); let size = monitor.size(); let scale = pet.window.scale_factor();
                        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
                        let work = if unsafe { GetMonitorInfoW(monitor.hmonitor() as _, &mut info) } != 0 {
                            info.rcWork
                        } else { windows_sys::Win32::Foundation::RECT { left:area.x, top:area.y, right:area.x + size.width as i32, bottom:area.y + size.height as i32 } };
                        let x = (pos.x + (552. * scale) as i32).min(work.right - (470. * scale) as i32).max(work.left);
                        let y = pos.y.min(work.bottom - (780. * scale) as i32).max(work.top);
                        builder = builder.with_position(PhysicalPosition::new(x, y));
                    }
                    let window = builder.build(target)?;
                    let view = WebViewBuilder::new().with_environment(features.opener.environment)
                        .with_https_scheme(true).with_custom_protocol("pet".into(), |_, req| resource(req))
                        .with_background_color((255, 249, 252, 255)).with_autoplay(true)
                        .with_devtools(std::env::var_os("PET_DEBUG_PORT").is_some()).with_navigation_handler(trusted)
                        .build(&window)?;
                    Ok(Host { view, window })
                })();
                match result {
                    Ok(host) => {
                        let _ = response.send(Some(NewWindowOpener { webview: host.view.webview(), environment: host.view.environment() }));
                        console = Some(host);
                    }
                    Err(error) => { log(&format!("控制台创建失败：{error}")); let _ = response.send(None); }
                }
            }
            Event::UserEvent(UserEvent::Ipc(raw)) => {
                let Ok(request) = serde_json::from_str::<Value>(&raw) else { return; };
                let id = request["id"].as_u64().unwrap_or(0);
                let command = request["command"].as_str().unwrap_or("");
                let result = match command {
                    "hit_region" => {
                        match hit_mask.update(&pet.window, &request) {
                            Ok(()) => { let _ = hit_mask.sync_pointer(&pet.window, locked); },
                            Err(error) => log(&format!("透明命中区域：{error}")),
                        }
                        return;
                    }
                    "get_locked" => Ok(json!(locked)),
                    "set_locked" => {
                        let result=lock(&pet, request["value"].as_bool().unwrap_or(false), &mut locked, &lock_item);
                        let _=hit_mask.sync_pointer(&pet.window,locked);result
                    }
                    "show_console" => { show(&console); Ok(Value::Null) }
                    "hide_console" => { if let Some(host) = &console { host.window.set_visible(false); } Ok(Value::Null) }
                    "drag" => {
                        if request["target"] == "console" { if let Some(host) = &console { let _ = host.window.drag_window(); } }
                        else if !locked { let _ = pet.window.drag_window(); }
                        Ok(Value::Null)
                    }
                    "sound_settings" => {
                        std::process::Command::new("explorer.exe").arg("ms-settings:sound").spawn().map(|_| Value::Null).map_err(|e| e.to_string())
                    }
                    "audio_start" => {
                        capture.take(); generation += 1;
                        capture = Some(audio::Capture::start(generation, id, proxy.clone()));
                        return;
                    }
                    "audio_stop" => { generation += 1; capture.take(); Ok(Value::Null) }
                    "frontend_error" => { log(&format!("界面错误：{}", request["message"])); return; }
                    "quit" => { *flow = ControlFlow::Exit; Ok(Value::Null) }
                    // Only the verification launcher enables this diagnostic command.
                    "inspect" if std::env::var_os("PET_DEBUG_PORT").is_some() => Ok(json!({
                        "locked":locked, "petSize": [pet.window.inner_size().width,pet.window.inner_size().height],
                        "clickThrough":unsafe { GetWindowLongW(pet.window.hwnd() as _, GWL_EXSTYLE) } as u32 & WS_EX_TRANSPARENT != 0,
                        "consoleSize":console.as_ref().map(|h| [h.window.inner_size().width,h.window.inner_size().height]),
                        "consoleVisible":console.as_ref().map(|h| h.window.is_visible()),
                        "scale":pet.window.scale_factor(), "audioActive":capture.is_some(), "generation":generation,
                        "regionRects":hit_mask.rect_count,
                        "windowShape":unsafe {
                            let region=CreateRectRgn(0,0,0,0);
                            let kind=GetWindowRgn(pet.window.hwnd() as _,region);
                            DeleteObject(region as _);kind
                        }
                    })),
                    "hit_test" if std::env::var_os("PET_DEBUG_PORT").is_some() => {
                        let scale=pet.window.scale_factor();
                        let x=(request["x"].as_f64().unwrap_or(0.)*scale).round() as i32;
                        let y=(request["y"].as_f64().unwrap_or(0.)*scale).round() as i32;
                        let pos=pet.window.inner_position().unwrap_or_default();
                        unsafe {
                            let point=POINT{x:pos.x+x,y:pos.y+y};
                            let inside=hit_mask.contains(x as f64/scale,y as f64/scale);
                            let _=hit_mask.sync_point(&pet.window,locked,point);
                            let root=GetAncestor(WindowFromPoint(point),GA_ROOT);
                            let result=json!({"inRegion":inside,"hitsPet":root==pet.window.hwnd() as _});
                            let _=hit_mask.sync_pointer(&pet.window,locked);
                            Ok(result)
                        }
                    }
                    "focus_pet" if std::env::var_os("PET_DEBUG_PORT").is_some() => {
                        pet.window.set_focus(); Ok(Value::Null)
                    }
                    "composition_probe" if std::env::var_os("PET_DEBUG_PORT").is_some() => {
                        composition_probe.command(&pet.window, target, &request)
                    }
                    _ => Err("未知桌面指令".into()),
                };
                reply(&pet.view, id, result);
            }
            Event::UserEvent(UserEvent::AudioStarted { generation: current, request }) => {
                reply(&pet.view, request, if current == generation { Ok(json!({"generation":current,"sampleRate":48000})) } else { Err("音频监听已取消".into()) });
            }
            Event::UserEvent(UserEvent::AudioFailed { generation: current, request, message }) => {
                if let Some(request) = request { reply(&pet.view, request, Err(message.clone())); }
                if current == generation {
                    capture.take();
                    let _ = pet.view.evaluate_script(&format!("window.__nativeAudio?.({})", json!({"generation":current,"error":message})));
                }
                log(&message);
            }
            Event::UserEvent(UserEvent::Pcm { generation: current, bytes, pending }) => {
                if current == generation && capture.is_some() {
                    let pcm = base64::engine::general_purpose::STANDARD.encode(bytes);
                    let completed = pending.clone();
                    if pet.view.evaluate_script_with_callback(&format!("window.__nativeAudio?.({})", json!({"generation":current,"pcm":pcm})),
                        move |_| { completed.fetch_sub(1, Ordering::AcqRel); }).is_err() {
                        pending.fetch_sub(1, Ordering::AcqRel);
                    }
                } else { pending.fetch_sub(1, Ordering::AcqRel); }
            }
            Event::UserEvent(UserEvent::ShowConsole) => show(&console),
            Event::UserEvent(UserEvent::ToggleLock) => { let _ = lock(&pet, !locked, &mut locked, &lock_item); }
            Event::UserEvent(UserEvent::Quit) => *flow = ControlFlow::Exit,
            Event::WindowEvent { window_id, event: WindowEvent::CloseRequested, .. } => {
                if window_id == pet.window.id() { *flow = ControlFlow::Exit; }
                else if let Some(host) = &console { if window_id == host.window.id() { host.window.set_visible(false); } }
            }
            Event::LoopDestroyed => { capture.take(); }
            _ => {}
        }
    });
}

fn main() {
    if let Err(error) = run() {
        log(&format!("启动失败：{error}"));
        #[link(name = "user32")]
        extern "system" {
            fn MessageBoxW(hwnd: isize, text: *const u16, caption: *const u16, kind: u32) -> i32;
        }
        let text: Vec<u16> =
            format!("桌宠启动失败：{error}\n请确认已安装 Microsoft Edge WebView2 运行时。")
                .encode_utf16()
                .chain(Some(0))
                .collect();
        let title: Vec<u16> = "桌宠节奏师".encode_utf16().chain(Some(0)).collect();
        unsafe {
            MessageBoxW(0, text.as_ptr(), title.as_ptr(), 0x10);
        }
        std::process::exit(1);
    }
}
