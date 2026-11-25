#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::webview::PageLoadEvent;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

static LOG: OnceLock<Mutex<File>> = OnceLock::new();
static PROBED: AtomicBool = AtomicBool::new(false);

const PROBE_JS: &str = r#"
(async () => {
  const out = {
    ua: navigator.userAgent,
    secure: isSecureContext,
    eme: typeof navigator.requestMediaKeySystemAccess === 'function',
    tauriInternals: typeof window.__TAURI_INTERNALS__,
    globalTauri: typeof window.__TAURI__
  };
  try {
    const cfg = [{
      initDataTypes: ['cenc'],
      audioCapabilities: [{ contentType: 'audio/mp4; codecs="mp4a.40.2"' }]
    }];
    const access = await navigator.requestMediaKeySystemAccess('com.widevine.alpha', cfg);
    out.wv = access.keySystem;
    const c = access.getConfiguration();
    out.robustness = (c.audioCapabilities && c.audioCapabilities[0] && c.audioCapabilities[0].robustness) || '';
  } catch (e) {
    out.wv = 'FAIL:' + e.name + ':' + e.message;
  }
  out.href = location.href;
  document.title = 'RUSTIFY_PROBE ' + JSON.stringify(out);
  location.href = 'https://open.spotify.com/__rustify_probe__?d=' + encodeURIComponent(JSON.stringify(out));
})();
"#;

fn timestamp() -> String {
    match time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
    {
        Ok(s) => s,
        Err(_) => String::from("unknown-time"),
    }
}

fn log(msg: impl AsRef<str>) {
    let line = format!("[{}] {}\n", timestamp(), msg.as_ref());
    if let Some(sink) = LOG.get() {
        if let Ok(mut file) = sink.lock() {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }
    #[cfg(debug_assertions)]
    print!("{line}");
}

fn init_log() {
    let dir = std::env::var("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("RustifySpike"))
        .unwrap_or_else(|_| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("spike.log");
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => {
            let _ = LOG.set(Mutex::new(file));
        }
        Err(_) => {}
    }
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "Show window", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "Hide to tray", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &quit])?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    log("tray_show");
                }
            }
            "hide" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                    log("tray_hide");
                }
            }
            "quit" => {
                log("tray_quit");
                app.exit(0);
            }
            _ => {}
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn main() {
    let started = Instant::now();
    init_log();
    log(format!("start pid={}", std::process::id()));

    let result = tauri::Builder::default()
        .setup(move |app| {
            if let Ok(dir) = app.path().app_local_data_dir() {
                log(format!("app_local_data_dir={}", dir.display()));
            }
            setup_tray(app)?;

            let url = tauri::Url::parse("https://open.spotify.com/")?;
            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("Rustify Spike")
                .inner_size(1280.0, 800.0)
                .on_navigation(|url| {
                    if url.path() == "/__rustify_probe__" {
                        log(format!("PROBE {}", url.as_str()));
                        return false;
                    }
                    let allowed = matches!(
                        url.host_str(),
                        Some("open.spotify.com")
                            | Some("accounts.spotify.com")
                            | Some("www.spotify.com")
                            | Some("challenges.cloudflare.com")
                    );
                    if !allowed {
                        log(format!(
                            "blocked_nav scheme={} url={}",
                            url.scheme(),
                            url.as_str()
                        ));
                    }
                    allowed
                })
                .on_page_load(|window, payload| {
                    log(format!(
                        "page_load event={:?} url={}",
                        payload.event(),
                        payload.url()
                    ));
                    if payload.event() == PageLoadEvent::Finished
                        && payload.url().host_str() == Some("open.spotify.com")
                        && !PROBED.swap(true, Ordering::SeqCst)
                    {
                        let window = window.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(Duration::from_secs(6));
                            match window.eval(PROBE_JS) {
                                Ok(()) => log("probe_eval_sent"),
                                Err(e) => log(format!("probe_eval_error={e}")),
                            }
                        });
                    }
                })
                .build()?;

            let close_window = window.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = close_window.hide();
                    log("window_close_requested_hidden");
                }
            });

            log(format!(
                "window_created_ms={}",
                started.elapsed().as_millis()
            ));
            let _ = window.show();

            #[cfg(windows)]
            std::thread::spawn(smtc_loop);

            Ok(())
        })
        .run(tauri::generate_context!());

    if let Err(error) = result {
        log(format!("fatal={error}"));
    }
}

#[cfg(windows)]
fn smtc_loop() {
    loop {
        match query_smtc() {
            Ok(lines) => {
                for line in lines {
                    log(line);
                }
            }
            Err(error) => log(format!("smtc_error={error}")),
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}

#[cfg(windows)]
fn query_smtc() -> windows::core::Result<Vec<String>> {
    use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;

    let manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.join()?;
    let sessions = manager.GetSessions()?;
    let count = sessions.Size()?;
    let mut lines = Vec::new();
    lines.push(format!("smtc_sessions={count}"));

    for index in 0..count {
        let session = match sessions.GetAt(index) {
            Ok(session) => session,
            Err(_) => continue,
        };
        let app_id = session
            .SourceAppUserModelId()
            .map(|value| value.to_string())
            .unwrap_or_default();
        let status = session
            .GetPlaybackInfo()
            .and_then(|info| info.PlaybackStatus())
            .map(|status| playback_status_name(status.0))
            .unwrap_or("Unknown");
        let position = session
            .GetTimelineProperties()
            .and_then(|timeline| timeline.Position())
            .map(|span| span.Duration as f64 / 10_000_000.0)
            .unwrap_or(-1.0);
        let (title, artist) = match session
            .TryGetMediaPropertiesAsync()
            .and_then(|op| op.join())
        {
            Ok(properties) => (
                properties
                    .Title()
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                properties
                    .Artist()
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
            ),
            Err(_) => (String::new(), String::new()),
        };
        lines.push(format!(
            "smtc app={app_id} status={status} pos={position:.1} title={title} artist={artist}"
        ));
    }
    Ok(lines)
}

#[cfg(windows)]
fn playback_status_name(value: i32) -> &'static str {
    match value {
        0 => "Closed",
        1 => "Opened",
        2 => "Changing",
        3 => "Stopped",
        4 => "Playing",
        5 => "Paused",
        _ => "Unknown",
    }
}
