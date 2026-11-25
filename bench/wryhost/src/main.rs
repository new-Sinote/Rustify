#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::{
    MemoryUsageLevel, WebContext, WebViewBuilder, WebViewBuilderExtWindows, WebViewExtWindows,
};

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

const BAR_HEIGHT: f64 = 40.0;

const BAR_HTML: &str = r##"<!doctype html>
<html><head><meta charset="utf-8"><style>
html,body{margin:0;height:40px;background:#000;overflow:hidden;user-select:none}
#bar{display:flex;align-items:center;height:40px;font-family:"Segoe UI",system-ui,sans-serif}
#drag{flex:1;display:flex;align-items:center;gap:9px;height:100%;padding-left:14px}
#title{color:#b3b3b3;font-size:12px;font-weight:600;letter-spacing:.2px}
.btn{width:46px;height:40px;display:flex;align-items:center;justify-content:center}
.btn:hover{background:#1f1f1f}
.btn.close:hover{background:#e81123}
.btn svg{display:block}
#restore{display:none}
body.maximized #restore{display:flex}
body.maximized #max{display:none}
</style></head><body>
<div id="bar">
  <div id="drag">
    <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true">
      <circle cx="12" cy="12" r="11" fill="#1ed760"/>
      <path d="M6.3 9.4c4-1.2 8.2-1 11.2.8" stroke="#000" stroke-width="1.8" fill="none" stroke-linecap="round"/>
      <path d="M7 12.5c3.3-.9 6.6-.7 9.1.7" stroke="#000" stroke-width="1.6" fill="none" stroke-linecap="round"/>
      <path d="M7.6 15.4c2.6-.7 5.2-.5 7.2.6" stroke="#000" stroke-width="1.4" fill="none" stroke-linecap="round"/>
    </svg>
    <div id="title">Spotify</div>
  </div>
  <div class="btn" id="min" title="Minimize">
    <svg width="10" height="10"><path d="M0 9.5h10" stroke="#fff" stroke-width="1.2"/></svg>
  </div>
  <div class="btn" id="max" title="Maximize">
    <svg width="10" height="10"><rect x="0.6" y="0.6" width="8.8" height="8.8" fill="none" stroke="#fff" stroke-width="1.1"/></svg>
  </div>
  <div class="btn" id="restore" title="Restore">
    <svg width="11" height="11"><rect x="0.6" y="2.9" width="7" height="7" fill="none" stroke="#fff" stroke-width="1"/><path d="M2.9 2.6V0.6h7.5v7.5H8.4" fill="none" stroke="#fff" stroke-width="1"/></svg>
  </div>
  <div class="btn close" id="close" title="Close">
    <svg width="10" height="10"><path d="M0.5 0.5l9 9M9.5 0.5l-9 9" stroke="#fff" stroke-width="1.2"/></svg>
  </div>
</div>
<script>
(function(){
  var send=function(c){window.ipc.postMessage(c);};
  var drag=document.getElementById('drag');
  drag.addEventListener('mousedown',function(e){
    if(e.button!==0)return;
    if(e.detail===2){send('toggle-max');return;}
    send('drag');
  });
  document.getElementById('min').addEventListener('click',function(){send('minimize');});
  document.getElementById('max').addEventListener('click',function(){send('toggle-max');});
  document.getElementById('restore').addEventListener('click',function(){send('toggle-max');});
  document.getElementById('close').addEventListener('click',function(){send('close');});
  window.setMaximized=function(v){document.body.classList.toggle('maximized',!!v);};
})();
</script></body></html>
"##;

fn chrome_rect(x: f64, y: f64, width: f64, height: f64) -> wry::Rect {
    wry::Rect {
        position: wry::dpi::LogicalPosition::new(x, y).into(),
        size: wry::dpi::LogicalSize::new(width, height).into(),
    }
}

fn log(path: &PathBuf, message: impl AsRef<str>) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "[{:?}] {}", Instant::now(), message.as_ref());
    }
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

#[derive(Clone, Copy, Debug)]
enum UserEvent {
    Low,
    Normal,
    Drag,
    Minimize,
    ToggleMax,
    Close,
}

fn main() -> wry::Result<()> {
    let started = Instant::now();
    let args: Vec<String> = std::env::args().collect();
    let log_path = PathBuf::from(
        arg_value(&args, "--log").unwrap_or_else(|| "wryhost.log".to_string()),
    );
    let url = arg_value(&args, "--url").unwrap_or_else(|| "about:blank".to_string());
    let title = arg_value(&args, "--title").unwrap_or_else(|| "Rustify wryhost".to_string());
    let data_dir = arg_value(&args, "--data-dir").map(PathBuf::from);
    let memory_low = args.iter().any(|a| a == "--memory-low");
    let hide_on_close = args.iter().any(|a| a == "--hide-on-close");
    let hide_upsells = args.iter().any(|a| a == "--hide-upsells");
    let custom_chrome = args.iter().any(|a| a == "--custom-chrome");
    let cdp = arg_value(&args, "--cdp");
    let extra_args = arg_value(&args, "--browser-args").unwrap_or_default();
    let toggle_file = arg_value(&args, "--toggle-file").map(PathBuf::from);

    log(&log_path, format!("start pid={}", std::process::id()));
    log(&log_path, format!("url={url}"));
    log(&log_path, format!("data_dir={data_dir:?}"));
    log(&log_path, format!("memory_low={memory_low}"));
    log(&log_path, format!("hide_upsells={hide_upsells}"));
    log(&log_path, format!("custom_chrome={custom_chrome}"));
    log(&log_path, format!("toggle_file={toggle_file:?}"));

    let mut event_loop_builder = EventLoopBuilder::<UserEvent>::with_user_event();
    let event_loop = event_loop_builder.build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title(&title)
        .with_inner_size(LogicalSize::new(1280.0, 800.0))
        .with_decorations(!custom_chrome)
        .build(&event_loop)
        .expect("window");

    let mut context = WebContext::new(data_dir.clone());
    let mut browser_args = String::from(
        "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
         --autoplay-policy=no-user-gesture-required",
    );
    if let Some(port) = &cdp {
        browser_args.push_str(&format!(" --remote-debugging-port={port}"));
    }
    if !extra_args.is_empty() {
        browser_args.push(' ');
        browser_args.push_str(&extra_args);
    }
    log(&log_path, format!("browser_args={browser_args}"));

    let mut builder = WebViewBuilder::new_with_web_context(&mut context)
        .with_url(&url)
        .with_additional_browser_args(&browser_args)
        .with_autoplay(true);
    if hide_upsells {
        builder = builder.with_initialization_script_for_main_only(HIDE_UPSELLS_JS, true);
    }

    let (webview, chrome_bar) = if custom_chrome {
        let logical = window.inner_size().to_logical::<f64>(window.scale_factor());
        let main_rect = chrome_rect(
            0.0,
            BAR_HEIGHT,
            logical.width.max(1.0),
            (logical.height - BAR_HEIGHT).max(1.0),
        );
        let bar_rect = chrome_rect(0.0, 0.0, logical.width.max(1.0), BAR_HEIGHT);
        let webview = match builder.with_bounds(main_rect).build_as_child(&window) {
            Ok(webview) => {
                log(&log_path, "chrome_main_ok");
                webview
            }
            Err(error) => {
                log(&log_path, format!("chrome_main_error={error:?}"));
                return Err(error);
            }
        };
        let proxy_for_bar = proxy.clone();
        let bar = match WebViewBuilder::new_with_web_context(&mut context)
            .with_html(BAR_HTML)
            .with_additional_browser_args(&browser_args)
            .with_bounds(bar_rect)
            .with_ipc_handler(move |request| {
                let event = match request.body().as_str() {
                    "drag" => Some(UserEvent::Drag),
                    "minimize" => Some(UserEvent::Minimize),
                    "toggle-max" => Some(UserEvent::ToggleMax),
                    "close" => Some(UserEvent::Close),
                    _ => None,
                };
                if let Some(event) = event {
                    let _ = proxy_for_bar.send_event(event);
                }
            })
            .build_as_child(&window)
        {
            Ok(bar) => {
                log(&log_path, "chrome_bar_ok");
                bar
            }
            Err(error) => {
                log(&log_path, format!("chrome_bar_error={error:?}"));
                return Err(error);
            }
        };
        (webview, Some(bar))
    } else {
        (builder.build(&window)?, None)
    };
    if memory_low {
        match webview.set_memory_usage_level(MemoryUsageLevel::Low) {
            Ok(()) => log(&log_path, "memory_usage_level=low_ok"),
            Err(error) => log(&log_path, format!("memory_usage_level=low_error:{error}")),
        }
    }
    log(
        &log_path,
        format!("window_ready_ms={}", started.elapsed().as_millis()),
    );

    if let Some(toggle) = toggle_file {
        let toggle_log = log_path.clone();
        std::thread::spawn(move || {
            let mut last = String::new();
            loop {
                let value = std::fs::read_to_string(&toggle)
                    .unwrap_or_default()
                    .trim()
                    .to_lowercase();
                if value != last {
                    last = value.clone();
                    match value.as_str() {
                        "low" => {
                            let _ = proxy.send_event(UserEvent::Low);
                        }
                        "normal" => {
                            let _ = proxy.send_event(UserEvent::Normal);
                        }
                        _ => {}
                    }
                    log(&toggle_log, format!("toggle_seen={value}"));
                }
                std::thread::sleep(Duration::from_millis(400));
            }
        });
    }

    let mut last_max = false;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if hide_on_close {
                    window.set_visible(false);
                    log(&log_path, "close_requested_hidden");
                } else {
                    *control_flow = ControlFlow::Exit;
                }
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                if let Some(bar) = &chrome_bar {
                    let logical = size.to_logical::<f64>(window.scale_factor());
                    let _ = bar.set_bounds(chrome_rect(
                        0.0,
                        0.0,
                        logical.width.max(1.0),
                        BAR_HEIGHT,
                    ));
                    let _ = webview.set_bounds(chrome_rect(
                        0.0,
                        BAR_HEIGHT,
                        logical.width.max(1.0),
                        (logical.height - BAR_HEIGHT).max(1.0),
                    ));
                    let maximized = window.is_maximized();
                    if maximized != last_max {
                        last_max = maximized;
                        let _ = bar.evaluate_script(&format!(
                            "window.setMaximized && window.setMaximized({maximized})"
                        ));
                    }
                }
            }
            Event::UserEvent(UserEvent::Drag) => {
                let _ = window.drag_window();
            }
            Event::UserEvent(UserEvent::Minimize) => {
                window.set_minimized(true);
            }
            Event::UserEvent(UserEvent::ToggleMax) => {
                let target = !window.is_maximized();
                window.set_maximized(target);
            }
            Event::UserEvent(UserEvent::Close) => {
                if hide_on_close {
                    window.set_visible(false);
                    log(&log_path, "close_requested_hidden");
                } else {
                    *control_flow = ControlFlow::Exit;
                }
            }
            Event::UserEvent(UserEvent::Low) => {
                match webview.set_memory_usage_level(MemoryUsageLevel::Low) {
                    Ok(()) => log(&log_path, "memory_usage_level=low_ok"),
                    Err(error) => log(&log_path, format!("memory_usage_level=low_error:{error}")),
                }
            }
            Event::UserEvent(UserEvent::Normal) => {
                match webview.set_memory_usage_level(MemoryUsageLevel::Normal) {
                    Ok(()) => log(&log_path, "memory_usage_level=normal_ok"),
                    Err(error) => log(&log_path, format!("memory_usage_level=normal_error:{error}")),
                }
            }
            _ => {}
        }
        let _ = &webview;
    });
}
