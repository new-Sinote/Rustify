#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy};
use tao::platform::windows::WindowExtWindows;
use tao::window::{Icon as WindowIcon, Window, WindowBuilder};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use url::Url;
use webview2_com::{
    IsDocumentPlayingAudioChangedEventHandler,
    Microsoft::Web::WebView2::Win32::{ICoreWebView2, ICoreWebView2_8},
};
#[cfg(feature = "ublock")]
use webview2_com::{
    Microsoft::Web::WebView2::Win32::{ICoreWebView2Profile7, ICoreWebView2_13},
    ProfileAddBrowserExtensionCompletedHandler,
};
use windows::core::{w, HSTRING, Interface, PCWSTR};
use windows::Win32::Foundation::{
    GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, LRESULT, RECT, WPARAM,
};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_ROUND,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, FindWindowW, GetWindowLongW, MessageBoxW, SetForegroundWindow,
    SetWindowLongW, SetWindowPos, ShowWindow, GWL_EXSTYLE, GWL_STYLE, MB_ICONINFORMATION,
    MB_ICONERROR, MB_OK, STYLESTRUCT, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOZORDER, SW_RESTORE, SW_SHOW, SW_SHOWNORMAL, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_STYLECHANGING, WS_CAPTION,
};
use wry::{
    MemoryUsageLevel, NewWindowResponse, PageLoadEvent, PermissionResponse, Theme, WebContext,
    WebView, WebViewBuilder, WebViewBuilderExtWindows, WebViewExtWindows,
};

const APP_NAME: &str = "Rustify";
const APP_VERSION: &str = "0.2.0";
const START_URL: &str = "https://open.spotify.com/";
const BAR_HEIGHT: f64 = 40.0;
const HYDRATE_TRIM: Duration = Duration::from_secs(30);
const PAUSE_DEBOUNCE: Duration = Duration::from_secs(25);
const TRIM_INTERVAL: Duration = Duration::from_secs(120);
const POST_GAP_TRIM: Duration = Duration::from_secs(15);
const SHORT_GAP: Duration = Duration::from_secs(20);
const TRIM_RESTORE: Duration = Duration::from_millis(300);

static APP_START: OnceLock<Instant> = OnceLock::new();
static LOG: OnceLock<Mutex<fs::File>> = OnceLock::new();

const HIDE_UPSELLS_JS: &str = r#"
(function () {
  var css = '[data-testid="global-nav-bar"] button[data-testid="upgrade-button"],' +
            '[data-testid="global-nav-bar"] a[href="/download"]{display:none !important}';
  function apply() {
    if (!document.head) return;
    if (document.getElementById('rustify-hide-upsells')) return;
    var el = document.createElement('style');
    el.id = 'rustify-hide-upsells';
    el.textContent = css;
    document.head.appendChild(el);
  }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', apply, { once: true });
  } else {
    apply();
  }
})();
"#;

const BAR_HTML: &str = r##"<!doctype html>
<html><head><meta charset="utf-8"><style>
html,body{margin:0;height:40px;background:#000;overflow:hidden;user-select:none}
#bar{display:flex;align-items:center;height:40px;font-family:"Segoe UI",system-ui,sans-serif;border-bottom:1px solid #202020}
#drag{flex:1;display:flex;align-items:center;gap:10px;height:100%;padding-left:14px}
#title{color:#d6d6d6;font-size:12.5px;font-weight:600;letter-spacing:.3px}
#subtitle{color:#6d6d6d;font-size:11px;margin-left:2px}
.btn{width:46px;height:40px;display:flex;align-items:center;justify-content:center;outline:none}
.btn:hover{background:#202020}
.btn:active{background:#2c2c2c}
.btn.close:hover{background:#e81123}
.btn.close:active{background:#f1707a}
.btn svg{display:block}
#restore{display:none}
body.maximized #restore{display:flex}
body.maximized #max{display:none}
</style></head><body>
<div id="bar">
  <div id="drag">
    <svg viewBox="0 0 32 32" width="21" height="21" aria-hidden="true">
      <rect x="1" y="1" width="30" height="30" rx="8" fill="#121212"/>
      <circle cx="16" cy="16" r="10.4" fill="#1ed760"/>
      <rect x="11" y="17" width="2.4" height="7" rx="1.2" fill="#0a0a0a"/>
      <rect x="14.8" y="12.5" width="2.4" height="11.5" rx="1.2" fill="#0a0a0a"/>
      <rect x="18.6" y="15" width="2.4" height="9" rx="1.2" fill="#0a0a0a"/>
    </svg>
    <div id="title">Rustify</div>
    <div id="subtitle">Spotify Web Player</div>
  </div>
  <div class="btn" id="min" title="Minimize" aria-label="Minimize">
    <svg width="10" height="10"><path d="M0 9.5h10" stroke="#fff" stroke-width="1.2"/></svg>
  </div>
  <div class="btn" id="max" title="Maximize" aria-label="Maximize">
    <svg width="10" height="10"><rect x="0.6" y="0.6" width="8.8" height="8.8" fill="none" stroke="#fff" stroke-width="1.1"/></svg>
  </div>
  <div class="btn" id="restore" title="Restore" aria-label="Restore">
    <svg width="11" height="11"><rect x="0.6" y="2.9" width="7" height="7" fill="none" stroke="#fff" stroke-width="1"/><path d="M2.9 2.6V0.6h7.5v7.5H8.4" fill="none" stroke="#fff" stroke-width="1"/></svg>
  </div>
  <div class="btn close" id="close" title="Close to tray" aria-label="Close to tray">
    <svg width="10" height="10"><path d="M0.5 0.5l9 9M9.5 0.5l-9 9" stroke="#fff" stroke-width="1.2"/></svg>
  </div>
</div>
<script>
(function(){
  var send=function(c){window.ipc.postMessage(c);};
  var drag=document.getElementById('drag');
  var down=false, dragging=false, sx=0, sy=0;

  drag.addEventListener('mousedown',function(e){
    if(e.button!==0)return;
    e.preventDefault();
    if(e.detail>=2){send('toggle-max');return;}
    down=true; dragging=false; sx=e.screenX; sy=e.screenY;
  });

  window.addEventListener('mousemove',function(e){
    if(!down||dragging)return;
    if(Math.abs(e.screenX-sx)>3||Math.abs(e.screenY-sy)>3){
      dragging=true; down=false; send('drag');
    }
  });

  window.addEventListener('mouseup',function(){down=false;});
  document.getElementById('min').addEventListener('click',function(){send('minimize');});
  document.getElementById('max').addEventListener('click',function(){send('toggle-max');});
  document.getElementById('restore').addEventListener('click',function(){send('toggle-max');});
  document.getElementById('close').addEventListener('click',function(){send('close');});
  window.setMaximized=function(v){document.body.classList.toggle('maximized',!!v);};
})();
</script></body></html>
"##;

#[derive(Clone, Debug)]
enum UserEvent {
    Show,
    Hide,
    Reload,
    ToggleMax,
    Minimize,
    Drag,
    Close,
    Quit,
    About,
    OpenDataFolder,
    PageLoaded,
    AudioChanged(bool),
}

struct Policy {
    hydrate_due: Option<Instant>,
    pause_due: Option<Instant>,
    trim_low_due: Option<Instant>,
    post_gap_due: Option<Instant>,
    periodic_due: Instant,
    paused_at: Option<Instant>,
    playing: bool,
    trim_interval: Duration,
    pause_debounce: Duration,
    post_gap_delay: Duration,
}

struct WindowState {
    width: f64,
    height: f64,
    maximized: bool,
    position: Option<(f64, f64)>,
}

impl Policy {
    fn new(trim_interval: Duration, pause_debounce: Duration, post_gap_delay: Duration) -> Self {
        Self {
            hydrate_due: None,
            pause_due: None,
            trim_low_due: None,
            post_gap_due: None,
            periodic_due: Instant::now() + trim_interval,
            paused_at: None,
            playing: false,
            trim_interval,
            pause_debounce,
            post_gap_delay,
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        let mut deadline = Some(self.periodic_due);
        for value in [
            self.hydrate_due,
            self.pause_due,
            self.trim_low_due,
            self.post_gap_due,
        ]
        .into_iter()
        .flatten()
        {
            deadline = Some(match deadline {
                Some(current) if current <= value => current,
                _ => value,
            });
        }
        deadline
    }

    fn on_page_loaded(&mut self) {
        self.hydrate_due = Some(Instant::now() + HYDRATE_TRIM);
        self.periodic_due = Instant::now() + self.trim_interval;
        log("page_loaded; hydrate trim scheduled");
    }

    fn on_audio_changed(&mut self, playing: bool) {
        if self.playing == playing {
            return;
        }
        self.playing = playing;
        if playing {
            self.pause_due = None;
            if let Some(paused_at) = self.paused_at.take() {
                if paused_at.elapsed() <= SHORT_GAP {
                    self.post_gap_due = Some(Instant::now() + self.post_gap_delay);
                    log("audio resumed after short gap; post-gap trim scheduled");
                }
            }
            log("audio=playing");
        } else {
            self.paused_at = Some(Instant::now());
            self.pause_due = Some(Instant::now() + self.pause_debounce);
            log("audio=paused; trim scheduled after debounce");
        }
    }

    fn tick(&mut self, main: &WebView, bar: Option<&WebView>) {
        let now = Instant::now();

        if let Some(due) = self.trim_low_due {
            if now >= due {
                self.trim_low_due = None;
                apply_low(main, bar);
                self.periodic_due = now + self.trim_interval;
                return;
            }
            return;
        }

        if let Some(due) = self.hydrate_due {
            if now >= due {
                self.hydrate_due = None;
                log("hydrate trim");
                self.start_trim(main, bar);
                return;
            }
        }

        if let Some(due) = self.post_gap_due {
            if now >= due {
                self.post_gap_due = None;
                log("post-gap trim");
                self.start_trim(main, bar);
                return;
            }
        }

        if let Some(due) = self.pause_due {
            if now >= due {
                self.pause_due = None;
                if !self.playing {
                    log("pause trim");
                    self.start_trim(main, bar);
                    return;
                }
            }
        }

        if now >= self.periodic_due {
            log("periodic trim");
            self.start_trim(main, bar);
        }
    }

    fn start_trim(&mut self, main: &WebView, bar: Option<&WebView>) {
        if self.trim_low_due.is_some() {
            return;
        }
        set_level(main, bar, MemoryUsageLevel::Normal, "normal");
        self.trim_low_due = Some(Instant::now() + TRIM_RESTORE);
        self.periodic_due = Instant::now() + self.trim_interval;
    }
}

fn main() {
    if let Err(error) = run() {
        log(format!("fatal={error}"));
        show_error(&format!(
            "Rustify could not start.\n\n{error}\n\n\
             Rustify needs the Microsoft Edge WebView2 Runtime.\n\
             Install it from https://developer.microsoft.com/microsoft-edge/webview2/ and try again."
        ));
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    let _ = APP_START.set(started);
    #[cfg(feature = "bench")]
    let args: Vec<String> = std::env::args().collect();

    #[cfg(feature = "bench")]
    let data_dir = arg_value(&args, "--profile")
        .map(PathBuf::from)
        .unwrap_or_else(default_data_dir);
    #[cfg(not(feature = "bench"))]
    let data_dir = default_data_dir();

    #[cfg(feature = "bench")]
    let log_path = arg_value(&args, "--log")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("rustify.log"));
    #[cfg(not(feature = "bench"))]
    let log_path = data_dir.join("rustify.log");
    fs::create_dir_all(&data_dir)?;
    init_log(&log_path);
    log(format!(
        "start pid={} version={} data_dir={}",
        std::process::id(),
        APP_VERSION,
        data_dir.display()
    ));

    #[cfg(feature = "ublock")]
    let ublock_ext = ublock_extension_path();
    #[cfg(feature = "ublock")]
    if let Some(path) = &ublock_ext {
        log(format!("ublock_ext={}", path.display()));
    } else {
        log("ublock_ext=none");
    }

    if !acquire_single_instance() {
        log("another instance is already running");
        return Ok(());
    }

    #[cfg(feature = "bench")]
    let cdp = arg_value(&args, "--cdp");
    #[cfg(not(feature = "bench"))]
    let cdp: Option<String> = None;

    #[cfg(feature = "bench")]
    let url = arg_value(&args, "--url").unwrap_or_else(|| START_URL.to_string());
    #[cfg(not(feature = "bench"))]
    let url = START_URL.to_string();

    #[cfg(feature = "bench")]
    let no_trim = args.iter().any(|arg| arg == "--no-trim");
    #[cfg(not(feature = "bench"))]
    let no_trim = false;

    #[cfg(feature = "bench")]
    let trim_sec = arg_value(&args, "--trim-sec")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(TRIM_INTERVAL.as_secs());
    #[cfg(not(feature = "bench"))]
    let trim_sec = TRIM_INTERVAL.as_secs();

    #[cfg(feature = "bench")]
    let pause_sec = arg_value(&args, "--pause-sec")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(PAUSE_DEBOUNCE.as_secs());
    #[cfg(not(feature = "bench"))]
    let pause_sec = PAUSE_DEBOUNCE.as_secs();

    #[cfg(feature = "bench")]
    let gap_sec = arg_value(&args, "--gap-sec")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(POST_GAP_TRIM.as_secs());
    #[cfg(not(feature = "bench"))]
    let gap_sec = POST_GAP_TRIM.as_secs();

    log(format!(
        "policy trim_sec={trim_sec} pause_sec={pause_sec} gap_sec={gap_sec}"
    ));

    let state = load_window_state(&data_dir);
    let (saved_width, saved_height, start_maximized) = (state.width, state.height, state.maximized);
    let mut event_loop_builder = EventLoopBuilder::<UserEvent>::with_user_event();
    let event_loop = event_loop_builder.build();
    let proxy = event_loop.create_proxy();

    let (position_x, position_y) = restored_position(
        &event_loop,
        state.position,
        saved_width,
        saved_height,
    );
    let mut window_builder = WindowBuilder::new()
        .with_title(APP_NAME)
        .with_inner_size(LogicalSize::new(saved_width, saved_height))
        .with_min_inner_size(LogicalSize::new(720.0, 480.0))
        .with_position(LogicalPosition::new(position_x, position_y))
        .with_decorations(true)
        .with_visible(false)
        .with_background_color((10, 10, 10, 255));
    if let Some(icon) = load_window_icon() {
        window_builder = window_builder.with_window_icon(Some(icon));
    }
    let window = window_builder.build(&event_loop)?;

    // Keep the native resize border and shadow, but remove the Windows caption so the
    // WebView title bar is the only chrome the user sees.
    strip_caption(&window);
    correct_client_size(&window, saved_width, saved_height);

    let mut context = WebContext::new(Some(data_dir.join("WebView2")));
    let mut browser_args = String::from(
        "--disable-features=msWebOOUI,msPdfOOUI --autoplay-policy=no-user-gesture-required",
    );
    if let Some(port) = &cdp {
        browser_args.push_str(&format!(" --remote-debugging-port={port}"));
    }
    log(format!("browser_args={browser_args}"));

    let proxy_nav = proxy.clone();
    let proxy_load = proxy.clone();
    let proxy_popup = proxy.clone();

    #[allow(unused_mut)]
    let mut builder = WebViewBuilder::new_with_web_context(&mut context)
        .with_url(&url)
        .with_theme(Theme::Dark)
        .with_background_color((12, 12, 12, 255))
        .with_autoplay(true)
        .with_devtools(cfg!(any(debug_assertions, feature = "bench")))
        .with_additional_browser_args(&browser_args)
        .with_initialization_script_for_main_only(HIDE_UPSELLS_JS, true)
        .with_navigation_handler(move |target| {
            let parsed = match Url::parse(&target) {
                Ok(parsed) => parsed,
                Err(_) => {
                    log("nav blocked: invalid url");
                    return false;
                }
            };
            if parsed.scheme() == "https" && is_allowed_host(parsed.host_str()) {
                return true;
            }
            let host = parsed.host_str().unwrap_or("unknown");
            log(format!("nav blocked host={host} scheme={}", parsed.scheme()));
            if matches!(parsed.scheme(), "http" | "https") {
                open_external(parsed.as_str());
            }
            let _ = proxy_nav.send_event(UserEvent::Show);
            false
        })
        .with_on_page_load_handler(move |event, target| {
            let host = Url::parse(&target)
                .ok()
                .and_then(|parsed| parsed.host_str().map(str::to_owned))
                .unwrap_or_else(|| "unknown".to_string());
            let label = match event {
                PageLoadEvent::Started => "started",
                PageLoadEvent::Finished => "finished",
            };
            log(format!("page_load={label} host={host}"));
            if matches!(event, PageLoadEvent::Finished) {
                let _ = proxy_load.send_event(UserEvent::PageLoaded);
            }
        })
        .with_permission_handler(|_| PermissionResponse::Deny)
        .with_new_window_req_handler(move |target, _features| {
            let host = Url::parse(&target)
                .ok()
                .and_then(|parsed| parsed.host_str().map(str::to_owned))
                .unwrap_or_else(|| "unknown".to_string());
            log(format!("popup blocked host={host}"));
            let _ = proxy_popup.send_event(UserEvent::Show);
            NewWindowResponse::Deny
        })
        .with_download_started_handler(move |target, _path| {
            let host = Url::parse(&target)
                .ok()
                .and_then(|parsed| parsed.host_str().map(str::to_owned))
                .unwrap_or_else(|| "unknown".to_string());
            log(format!("download blocked host={host}"));
            false
        });

    #[cfg(feature = "ublock")]
    if ublock_ext.is_some() {
        builder = builder.with_browser_extensions_enabled(true);
        log("ublock_extensions_enabled");
    }

    let logical = window
        .inner_size()
        .to_logical::<f64>(window.scale_factor());
    let main_rect = chrome_rect(
        0.0,
        BAR_HEIGHT,
        logical.width.max(1.0),
        (logical.height - BAR_HEIGHT).max(1.0),
    );
    let webview = builder.with_bounds(main_rect).build_as_child(&window)?;
    log("main_webview_ready");

    #[cfg(feature = "ublock")]
    if let Some(extension) = &ublock_ext {
        if let Err(error) = install_browser_extension(&webview, extension) {
            log(format!("ublock_install_error={error}"));
        }
    }

    let bar_rect = chrome_rect(0.0, 0.0, logical.width.max(1.0), BAR_HEIGHT);
    let bar_html = if start_maximized {
        BAR_HTML.replacen("<body>", "<body class=\"maximized\">", 1)
    } else {
        BAR_HTML.to_string()
    };
    let proxy_bar = proxy.clone();
    #[allow(unused_mut)]
    let mut bar_builder = WebViewBuilder::new_with_web_context(&mut context)
        .with_html(bar_html)
        .with_bounds(bar_rect)
        .with_theme(Theme::Dark)
        .with_background_color((0, 0, 0, 255))
        .with_additional_browser_args(&browser_args)
        .with_ipc_handler(move |request| {
            let event = match request.body().as_str() {
                "drag" => Some(UserEvent::Drag),
                "minimize" => Some(UserEvent::Minimize),
                "toggle-max" => Some(UserEvent::ToggleMax),
                "close" => Some(UserEvent::Close),
                _ => None,
            };
            if let Some(event) = event {
                let _ = proxy_bar.send_event(event);
            }
        });

    #[cfg(feature = "ublock")]
    if ublock_ext.is_some() {
        bar_builder = bar_builder.with_environment(webview.environment());
        log("ublock_chrome_environment_shared");
    }

    let bar = bar_builder.build_as_child(&window)?;
    log("chrome_bar_ready");

    if let Err(error) = watch_audio(&webview, proxy.clone()) {
        log(format!("audio_watch_error={error}"));
    }

    if start_maximized {
        window.set_maximized(true);
    }
    window.set_visible(true);
    window.set_focus();
    let client = window.inner_size();
    let outer = window.outer_size();
    log(format!(
        "window_ready_ms={} client={}x{} outer={}x{}",
        started.elapsed().as_millis(),
        client.width,
        client.height,
        outer.width,
        outer.height
    ));

    let tray = match setup_tray(proxy.clone()) {
        Ok(tray) => Some(tray),
        Err(error) => {
            log(format!("tray_error={error}"));
            None
        }
    };

    let mut policy = Policy::new(
        Duration::from_secs(trim_sec),
        Duration::from_secs(pause_sec),
        Duration::from_secs(gap_sec),
    );
    let mut last_maximized = window.is_maximized();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if tray.is_some() {
                    hide_window(&window);
                    save_window_state(&window, &data_dir);
                    log("close_requested_hidden");
                } else {
                    save_window_state(&window, &data_dir);
                    *control_flow = ControlFlow::Exit;
                }
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                layout(&window, &webview, Some(&bar));
                let maximized = window.is_maximized();
                if maximized != last_maximized {
                    last_maximized = maximized;
                    let _ = bar.evaluate_script(&format!(
                        "window.setMaximized && window.setMaximized({maximized})"
                    ));
                }
            }
            Event::WindowEvent {
                event: WindowEvent::ScaleFactorChanged { .. },
                ..
            } => {
                layout(&window, &webview, Some(&bar));
            }
            Event::UserEvent(UserEvent::Show) => {
                show_window(&window);
                log("window_shown");
            }
            Event::UserEvent(UserEvent::Hide) => {
                hide_window(&window);
                save_window_state(&window, &data_dir);
                log("window_hidden");
            }
            Event::UserEvent(UserEvent::Reload) => {
                let _ = webview.reload();
                log("reload_requested");
            }
            Event::UserEvent(UserEvent::ToggleMax) => {
                window.set_maximized(!window.is_maximized());
            }
            Event::UserEvent(UserEvent::Minimize) => {
                window.set_minimized(true);
            }
            Event::UserEvent(UserEvent::Drag) => {
                if let Err(error) = window.drag_window() {
                    log(format!("drag_error={error}"));
                }
            }
            Event::UserEvent(UserEvent::Close) => {
                if tray.is_some() {
                    hide_window(&window);
                    save_window_state(&window, &data_dir);
                    log("close_button_hidden");
                } else {
                    save_window_state(&window, &data_dir);
                    *control_flow = ControlFlow::Exit;
                }
            }
            Event::UserEvent(UserEvent::Quit) => {
                save_window_state(&window, &data_dir);
                log("quit_requested");
                *control_flow = ControlFlow::Exit;
                return;
            }
            Event::UserEvent(UserEvent::About) => {
                show_about();
            }
            Event::UserEvent(UserEvent::OpenDataFolder) => {
                open_external(&data_dir.to_string_lossy());
            }
            Event::UserEvent(UserEvent::PageLoaded) => {
                policy.on_page_loaded();
            }
            Event::UserEvent(UserEvent::AudioChanged(playing)) => {
                policy.on_audio_changed(playing);
            }
            _ => {}
        }

        if !no_trim {
            policy.tick(&webview, Some(&bar));
        }
        if let Some(deadline) = policy.next_deadline() {
            *control_flow = ControlFlow::WaitUntil(deadline);
        }
    });
}

fn acquire_single_instance() -> bool {
    unsafe {
        match CreateMutexW(None, true, w!("Local\\Rustify.SingleInstance")) {
            Ok(handle) => {
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    if let Ok(hwnd) = FindWindowW(PCWSTR::null(), w!("Rustify")) {
                        let _ = ShowWindow(hwnd, SW_SHOW);
                        let _ = ShowWindow(hwnd, SW_RESTORE);
                        let _ = SetForegroundWindow(hwnd);
                    }
                    let _ = handle;
                    false
                } else {
                    let _ = handle;
                    true
                }
            }
            Err(error) => {
                log(format!("single_instance_error={error}"));
                true
            }
        }
    }
}

fn setup_tray(proxy: EventLoopProxy<UserEvent>) -> Result<TrayIcon, Box<dyn Error>> {
    let show = MenuItem::with_id("show", "Show Rustify", true, None);
    let hide = MenuItem::with_id("hide", "Hide to tray", true, None);
    let reload = MenuItem::with_id("reload", "Reload player", true, None);
    let data = MenuItem::with_id("data", "Open data folder", true, None);
    let about = MenuItem::with_id("about", "About Rustify", true, None);
    let quit = MenuItem::with_id("quit", "Quit", true, None);
    let sep_one = PredefinedMenuItem::separator();
    let sep_two = PredefinedMenuItem::separator();
    let menu = Menu::with_items(&[
        &show,
        &hide,
        &reload,
        &sep_one,
        &data,
        &about,
        &sep_two,
        &quit,
    ])?;

    let click_proxy = proxy.clone();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let opened = matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            }
        );
        if opened {
            let _ = click_proxy.send_event(UserEvent::Show);
        }
    }));

    let menu_proxy = proxy;
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let mapped = match event.id().as_ref() {
            "show" => Some(UserEvent::Show),
            "hide" => Some(UserEvent::Hide),
            "reload" => Some(UserEvent::Reload),
            "data" => Some(UserEvent::OpenDataFolder),
            "about" => Some(UserEvent::About),
            "quit" => Some(UserEvent::Quit),
            _ => None,
        };
        if let Some(event) = mapped {
            let _ = menu_proxy.send_event(event);
        }
    }));

    let (rgba, width, height) = load_icon_rgba()?;
    let icon = tray_icon::Icon::from_rgba(rgba, width, height)?;
    Ok(TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .with_tooltip(APP_NAME)
        .with_menu_on_left_click(false)
        .build()?)
}

fn watch_audio(webview: &WebView, proxy: EventLoopProxy<UserEvent>) -> Result<(), Box<dyn Error>> {
    let core: ICoreWebView2_8 = webview.webview().cast()?;
    let handler_proxy = proxy.clone();
    let handler = IsDocumentPlayingAudioChangedEventHandler::create(Box::new(
        move |sender: Option<ICoreWebView2>, _args| {
            if let Some(sender) = sender {
                if let Ok(core) = sender.cast::<ICoreWebView2_8>() {
                    if let Some(playing) = read_playing(&core) {
                        let _ = handler_proxy.send_event(UserEvent::AudioChanged(playing));
                    }
                }
            }
            Ok(())
        },
    ));
    let mut token = 0i64;
    unsafe {
        core.add_IsDocumentPlayingAudioChanged(&handler, &mut token)?;
    }
    if let Some(playing) = read_playing(&core) {
        let _ = proxy.send_event(UserEvent::AudioChanged(playing));
    }
    log(format!("audio_watch_ready token={token}"));
    Ok(())
}

fn read_playing(core: &ICoreWebView2_8) -> Option<bool> {
    let mut playing = windows::core::BOOL::default();
    unsafe { core.IsDocumentPlayingAudio(&mut playing) }.ok()?;
    Some(playing.as_bool())
}

#[cfg(feature = "ublock")]
fn install_browser_extension(webview: &WebView, extension: &Path) -> Result<(), Box<dyn Error>> {
    let profile = unsafe {
        webview
            .webview()
            .cast::<ICoreWebView2_13>()?
            .Profile()?
            .cast::<ICoreWebView2Profile7>()?
    };
    let path = HSTRING::from(extension);
    let handler =
        ProfileAddBrowserExtensionCompletedHandler::create(Box::new(|result, _extension| {
            match result {
                Ok(()) => log("ublock_install_ok"),
                Err(error) => log(format!(
                    "ublock_install_failed code={:#x} message={}",
                    error.code().0,
                    error.message()
                )),
            }
            Ok(())
        }));
    unsafe { profile.AddBrowserExtension(&path, &handler)? };
    log("ublock_install_requested");
    Ok(())
}

fn layout(window: &Window, main: &WebView, bar: Option<&WebView>) {
    let logical = window
        .inner_size()
        .to_logical::<f64>(window.scale_factor());
    if let Some(bar) = bar {
        let _ = bar.set_bounds(chrome_rect(0.0, 0.0, logical.width.max(1.0), BAR_HEIGHT));
    }
    let _ = main.set_bounds(chrome_rect(
        0.0,
        BAR_HEIGHT,
        logical.width.max(1.0),
        (logical.height - BAR_HEIGHT).max(1.0),
    ));
}

fn set_level(main: &WebView, bar: Option<&WebView>, level: MemoryUsageLevel, label: &str) {
    match main.set_memory_usage_level(level) {
        Ok(()) => log(format!("memory={label} main=ok")),
        Err(error) => log(format!("memory={label} main_error={error}")),
    }
    if let Some(bar) = bar {
        match bar.set_memory_usage_level(level) {
            Ok(()) => log(format!("memory={label} bar=ok")),
            Err(error) => log(format!("memory={label} bar_error={error}")),
        }
    }
}

fn apply_low(main: &WebView, bar: Option<&WebView>) {
    set_level(main, bar, MemoryUsageLevel::Low, "low");
}

unsafe extern "system" fn keep_frameless(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if message == WM_STYLECHANGING && wparam.0 as i32 == GWL_STYLE.0 {
        let style = &mut *(lparam.0 as *mut STYLESTRUCT);
        style.styleNew &= !(WS_CAPTION.0);
    }
    DefSubclassProc(hwnd, message, wparam, lparam)
}

fn strip_caption(window: &Window) {
    let hwnd = HWND(window.hwnd() as _);
    unsafe {
        // Tao re-applies WS_CAPTION whenever it updates window flags (show, hide,
        // maximize), so the subclass keeps the caption out of every style change.
        let _ = SetWindowSubclass(hwnd, Some(keep_frameless), 1, 0);
        let style = GetWindowLongW(hwnd, GWL_STYLE);
        let _ = SetWindowLongW(hwnd, GWL_STYLE, style & !(WS_CAPTION.0 as i32));
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
        let dark = 1i32;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const _ as _,
            4,
        );
        let round = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &round as *const _ as _,
            4,
        );
    }
}

/// The builder sizes the decorated window, then the caption is stripped. Compute
/// the real frame for the resulting style with `AdjustWindowRectEx` so the outer
/// size yields the requested client area without the size creeping every launch.
fn correct_client_size(window: &Window, width: f64, height: f64) {
    let hwnd = HWND(window.hwnd() as _);
    let scale = window.scale_factor();
    let target_width = (width * scale).round() as i32;
    let target_height = (height * scale).round() as i32;
    unsafe {
        let style = WINDOW_STYLE(GetWindowLongW(hwnd, GWL_STYLE) as u32);
        let ex_style = WINDOW_EX_STYLE(GetWindowLongW(hwnd, GWL_EXSTYLE) as u32);
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: target_width,
            bottom: target_height,
        };
        if AdjustWindowRectEx(&mut rect, style, false, ex_style).is_ok() {
            let outer_width = rect.right - rect.left;
            let outer_height = rect.bottom - rect.top;
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                outer_width,
                outer_height,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

fn hide_window(window: &Window) {
    window.set_visible(false);
}

fn show_window(window: &Window) {
    window.set_visible(true);
    window.set_minimized(false);
    window.set_focus();
}

fn chrome_rect(x: f64, y: f64, width: f64, height: f64) -> wry::Rect {
    wry::Rect {
        position: wry::dpi::LogicalPosition::new(x, y).into(),
        size: wry::dpi::LogicalSize::new(width, height).into(),
    }
}

fn is_allowed_host(host: Option<&str>) -> bool {
    matches!(
        host,
        Some("open.spotify.com")
            | Some("accounts.spotify.com")
            | Some("www.spotify.com")
            | Some("spotify.com")
            | Some("challenges.cloudflare.com")
    )
}

fn restored_position(
    event_loop: &EventLoop<UserEvent>,
    saved: Option<(f64, f64)>,
    width: f64,
    height: f64,
) -> (f64, f64) {
    if let Some((x, y)) = saved {
        let on_screen = event_loop.available_monitors().any(|monitor| {
            let scale = monitor.scale_factor();
            let origin = monitor.position().to_logical::<f64>(scale);
            let size = monitor.size().to_logical::<f64>(scale);
            x >= origin.x
                && y >= origin.y
                && x < origin.x + size.width
                && y < origin.y + size.height
        });
        if on_screen {
            return (x, y);
        }
    }
    centered_position(event_loop.primary_monitor(), width, height)
}

fn centered_position(
    monitor: Option<tao::monitor::MonitorHandle>,
    width: f64,
    height: f64,
) -> (f64, f64) {
    if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let logical = monitor.size().to_logical::<f64>(scale);
        let origin = monitor.position().to_logical::<f64>(scale);
        (
            origin.x + ((logical.width - width) / 2.0).max(0.0),
            origin.y + ((logical.height - height) / 2.0).max(0.0),
        )
    } else {
        (160.0, 100.0)
    }
}

fn default_data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Rustify")
}

fn load_window_state(data_dir: &Path) -> WindowState {
    let defaults = WindowState {
        width: 1280.0,
        height: 800.0,
        maximized: false,
        position: None,
    };
    let text = match fs::read_to_string(data_dir.join("window.txt")) {
        Ok(text) => text,
        Err(_) => return defaults,
    };
    let mut lines = text.lines();
    let width = lines
        .next()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .unwrap_or(defaults.width)
        .clamp(720.0, 3840.0);
    let height = lines
        .next()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .unwrap_or(defaults.height)
        .clamp(480.0, 2160.0);
    let maximized = lines
        .next()
        .map(|value| value.trim() == "1")
        .unwrap_or(false);
    let x = lines.next().and_then(|value| value.trim().parse::<f64>().ok());
    let y = lines.next().and_then(|value| value.trim().parse::<f64>().ok());
    let position = match (x, y) {
        (Some(x), Some(y)) => Some((x, y)),
        _ => None,
    };
    WindowState {
        width,
        height,
        maximized,
        position,
    }
}

fn save_window_state(window: &Window, data_dir: &Path) {
    if window.is_minimized() {
        return;
    }
    let previous = load_window_state(data_dir);
    let maximized = window.is_maximized();
    let scale = window.scale_factor();
    let (width, height, position) = if maximized {
        (previous.width, previous.height, previous.position)
    } else {
        let logical = window.inner_size().to_logical::<f64>(scale);
        let position = window
            .outer_position()
            .ok()
            .map(|position| position.to_logical::<f64>(scale))
            .map(|position| (position.x, position.y));
        (logical.width, logical.height, position)
    };
    let text = format!(
        "{:.0}\n{:.0}\n{}\n{}\n{}",
        width.max(720.0),
        height.max(480.0),
        u8::from(maximized),
        position.map(|(x, _)| format!("{x:.0}")).unwrap_or_default(),
        position.map(|(_, y)| format!("{y:.0}")).unwrap_or_default()
    );
    let _ = fs::write(data_dir.join("window.txt"), text);
}

fn show_about() {
    let text = format!(
        "{APP_NAME} {APP_VERSION}\n\n\
         An unofficial native host for the official Spotify Web Player.\n\
         Ads, playback, and DRM are handled by Spotify's own player.\n\n\
         MIT licensed. Not affiliated with Spotify AB."
    );
    unsafe {
        let message = HSTRING::from(text);
        MessageBoxW(
            None,
            &message,
            w!("About Rustify"),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

fn show_error(message: &str) {
    unsafe {
        let text = HSTRING::from(message);
        MessageBoxW(None, &text, w!("Rustify"), MB_OK | MB_ICONERROR);
    }
}

fn open_external(target: &str) {
    unsafe {
        let target = HSTRING::from(target);
        let result = ShellExecuteW(
            None,
            w!("open"),
            &target,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
        if result.0 as isize <= 32 {
            log(format!("shell_execute_failed result={}", result.0 as isize));
        }
    }
}

fn include_icon() -> (Vec<u8>, u32, u32) {
    let bytes = include_bytes!("../assets/icon-64.png");
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .expect("embedded icon must be a valid PNG");
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buffer)
        .expect("embedded icon must decode");
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    buffer.truncate(info.buffer_size());
    (buffer, info.width, info.height)
}

fn load_icon_rgba() -> Result<(Vec<u8>, u32, u32), Box<dyn Error>> {
    Ok(include_icon())
}

fn load_window_icon() -> Option<WindowIcon> {
    let (rgba, width, height) = include_icon();
    WindowIcon::from_rgba(rgba, width, height).ok()
}

#[cfg(any(feature = "bench", feature = "ublock"))]
fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

#[cfg(feature = "ublock")]
fn ublock_extension_path() -> Option<PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    let path = arg_value(&args, "--ublock-ext")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("RUSTIFY_UBLOCK_EXT").map(PathBuf::from))?;
    if path.join("manifest.json").is_file() {
        Some(path)
    } else {
        log(format!(
            "ublock_ext_invalid path={} (manifest.json not found)",
            path.display()
        ));
        None
    }
}

fn init_log(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(metadata) = fs::metadata(path) {
        if metadata.len() > 2 * 1024 * 1024 {
            let old = path.with_extension("log.1");
            let _ = fs::remove_file(&old);
            let _ = fs::rename(path, old);
        }
    }
    if let Ok(file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = LOG.set(Mutex::new(file));
    }
}

fn log(message: impl AsRef<str>) {
    let elapsed = APP_START
        .get()
        .map(|start| start.elapsed().as_millis())
        .unwrap_or(0);
    if let Some(sink) = LOG.get() {
        if let Ok(mut file) = sink.lock() {
            let _ = writeln!(file, "[{elapsed:>7} ms] {}", message.as_ref());
        }
    }
    #[cfg(debug_assertions)]
    eprintln!("[{elapsed:>7} ms] {}", message.as_ref());
}
