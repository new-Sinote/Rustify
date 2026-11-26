#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
#[cfg(feature = "adblock")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "adblock")]
use std::sync::Arc;
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
#[cfg(feature = "adblock")]
use webview2_com::{take_pwstr, DocumentTitleChangedEventHandler};
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
const APP_VERSION: &str = "0.3.0";
const START_URL: &str = "https://open.spotify.com/";
const BAR_HEIGHT: f64 = 40.0;
const HYDRATE_TRIM: Duration = Duration::from_secs(30);
const PAUSE_DEBOUNCE: Duration = Duration::from_secs(25);
const TRIM_INTERVAL: Duration = Duration::from_secs(120);
const POST_GAP_TRIM: Duration = Duration::from_secs(15);
const SHORT_GAP: Duration = Duration::from_secs(20);
const TRIM_RESTORE: Duration = Duration::from_millis(300);
#[cfg(feature = "adblock")]
const AD_WATCHDOG: Duration = Duration::from_secs(120);
#[cfg(feature = "adblock")]
const AD_WATCHDOG_EXTEND: Duration = Duration::from_secs(30);
#[cfg(feature = "adblock")]
const AD_WATCHDOG_MAX: Duration = Duration::from_secs(15 * 60);
#[cfg(feature = "adblock")]
const AD_RETRY: Duration = Duration::from_secs(5);
#[cfg(feature = "adblock")]
const DISCOVERY_SKIP_INTERVAL: Duration = Duration::from_secs(5);
#[cfg(feature = "adblock")]
const DISCOVERY_MAX_SKIPS: u32 = 50;
#[cfg(feature = "adblock")]
const DISCOVERY_SKIP_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(feature = "adblock")]
const DISCOVERY_UNKNOWN_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(feature = "adblock")]
const PANEL_HEIGHT: f64 = 96.0;

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
html,body{margin:0;height:100%;background:#000;overflow:hidden;user-select:none}
#bar{display:flex;align-items:center;height:40px;font-family:"Segoe UI",system-ui,sans-serif;border-bottom:1px solid #202020}
#drag{flex:1;display:flex;align-items:center;gap:10px;height:100%;padding-left:14px}
#title{color:#d6d6d6;font-size:12.5px;font-weight:600;letter-spacing:.3px}
#subtitle{color:#6d6d6d;font-size:11px;margin-left:2px}
.btn{width:46px;height:40px;display:flex;align-items:center;justify-content:center;outline:none}
.btn:hover{background:#202020}
.btn:active{background:#2c2c2c}
.btn.close:hover{background:#e81123}
.btn.close:active{background:#f1707a}
.btn.test span{font:700 9px "Segoe UI",system-ui,sans-serif;color:#f5c518;letter-spacing:.5px}
body.discovery-on .btn.test span{color:#1ed760}
.btn svg{display:block}
#restore{display:none}
body.maximized #restore{display:flex}
body.maximized #max{display:none}
#panel{display:none;height:96px;box-sizing:border-box;padding:8px 14px;background:#0b0b0b;border-bottom:1px solid #202020;color:#cfcfcf;font-family:"Segoe UI",system-ui,sans-serif}
body.panel-open #panel{display:block}
.panel-row{display:flex;align-items:center;gap:10px;height:24px}
.panel-title{font-size:11.5px;font-weight:600;color:#e6e6e6;letter-spacing:.2px}
.tag{font-size:9px;font-weight:700;color:#0a0a0a;background:#f5c518;border-radius:3px;padding:1px 4px;margin-left:4px}
.enabled{font-size:9px;font-weight:700;color:#0a0a0a;background:#5a5a5a;border-radius:3px;padding:1px 4px;margin-left:6px}
body.discovery-on .enabled{background:#1ed760}
.state{font-size:10.5px;font-weight:600;color:#9ad;margin-left:auto}
body.ad-error .state{color:#f0a0a0}
.muted{display:none;font-size:9px;font-weight:700;color:#fff;background:#c0392b;border-radius:3px;padding:1px 5px}
body.ad-muted .muted{display:inline-block}
.controls{gap:8px}
.controls label{display:flex;align-items:center;gap:4px;color:#9a9a9a}
.controls input{width:42px;height:18px;background:#151515;color:#e6e6e6;border:1px solid #303030;border-radius:3px;font:10.5px "Segoe UI",system-ui,sans-serif;padding:0 4px}
.controls button{height:20px;padding:0 9px;background:#1e1e1e;color:#e6e6e6;border:1px solid #383838;border-radius:3px;font:10.5px "Segoe UI",system-ui,sans-serif;cursor:pointer}
.controls button:hover:not(:disabled){background:#2a2a2a}
.controls button:disabled{opacity:.4;cursor:default}
.stats{margin-left:auto;color:#9a9a9a;white-space:nowrap}
.stats b{color:#e6e6e6;font-weight:600}
.detail{height:24px;line-height:24px;color:#7d7d7d;font-size:10.5px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
#ad-start{color:#8fe3a1}
#ad-stop{color:#f0a0a0}
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
  <!--ADBLOCK_BUTTON-->
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
<!--ADBLOCK_PANEL-->
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
  <!--ADBLOCK_SCRIPT-->
})();
</script></body></html>
"##;

#[cfg(feature = "adblock")]
const ADBLOCK_BUTTON_HTML: &str = r#"<div class="btn test" id="test" title="Automatic Ad Discovery (test mode)" aria-label="Automatic Ad Discovery"><span>AD</span></div>"#;
#[cfg(not(feature = "adblock"))]
const ADBLOCK_BUTTON_HTML: &str = "";

#[cfg(feature = "adblock")]
const ADBLOCK_PANEL_HTML: &str = r#"<div id="panel">
  <div class="panel-row">
    <span class="panel-title">Automatic Ad Discovery <span class="tag">TEST</span></span>
    <span id="ad-enabled" class="enabled">OFF</span>
    <span id="ad-state" class="state">Off</span>
    <span class="muted">MUTED</span>
  </div>
  <div class="panel-row controls">
    <label>Interval <input id="ad-interval" type="number" min="2" max="60" value="5"> s</label>
    <label>Max skips <input id="ad-max" type="number" min="1" max="500" value="50"></label>
    <button id="ad-start" type="button">Start</button>
    <button id="ad-stop" type="button" disabled>Stop</button>
    <span class="stats" title="Successful mute/unmute cycles">Skips <b id="ad-skips">0</b> &middot; Ads <b id="ad-ads">0</b> &middot; Mute cycles <b id="ad-cycles">0</b></span>
  </div>
  <div id="ad-detail" class="detail">Opt-in test mode. Never starts automatically.</div>
</div>"#;
#[cfg(not(feature = "adblock"))]
const ADBLOCK_PANEL_HTML: &str = "";

#[cfg(feature = "adblock")]
const ADBLOCK_PANEL_SCRIPT: &str = r#"
  var adTest=document.getElementById('test');
  if(adTest){adTest.addEventListener('click',function(){send('adblock-panel');});}
  var adStart=document.getElementById('ad-start');
  if(adStart){adStart.addEventListener('click',function(){
    var interval=Math.max(2,Math.min(60,parseInt(document.getElementById('ad-interval').value,10)||5));
    var max=Math.max(1,Math.min(500,parseInt(document.getElementById('ad-max').value,10)||50));
    document.getElementById('ad-interval').value=interval;
    document.getElementById('ad-max').value=max;
    send('adblock-start:'+interval+':'+max);
  });}
  var adStop=document.getElementById('ad-stop');
  if(adStop){adStop.addEventListener('click',function(){send('adblock-stop');});}
  window.setPanelOpen=function(open){document.body.classList.toggle('panel-open',!!open);};
  window.setAdblockState=function(state){
    if(!state)return;
    document.body.classList.toggle('discovery-on',!!state.enabled);
    document.body.classList.toggle('ad-muted',!!state.muted);
    document.body.classList.toggle('ad-error',state.state==='Error');
    var stateEl=document.getElementById('ad-state');
    if(stateEl)stateEl.textContent=state.state||'Off';
    var enabledEl=document.getElementById('ad-enabled');
    if(enabledEl)enabledEl.textContent=state.enabled?'ON':'OFF';
    var set=function(id,value){var el=document.getElementById(id);if(el)el.textContent=String(value);};
    set('ad-skips',state.skips||0);
    set('ad-ads',state.ads||0);
    set('ad-cycles',state.cycles||0);
    var detail=document.getElementById('ad-detail');
    if(detail)detail.textContent=state.detail||'';
    var startBtn=document.getElementById('ad-start');
    var stopBtn=document.getElementById('ad-stop');
    if(startBtn)startBtn.disabled=!!state.enabled;
    if(stopBtn)stopBtn.disabled=!state.enabled;
  };
"#;
#[cfg(not(feature = "adblock"))]
const ADBLOCK_PANEL_SCRIPT: &str = "";

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
    #[cfg(feature = "adblock")]
    TitleChanged(TitleInfo),
    #[cfg(feature = "adblock")]
    SkipResult {
        generation: u64,
        result: SkipResult,
    },
    #[cfg(feature = "adblock")]
    DiscoveryStart {
        interval_secs: u64,
        max_skips: u32,
    },
    #[cfg(feature = "adblock")]
    DiscoveryStop,
    #[cfg(feature = "adblock")]
    ToggleAdblockPanel,
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

#[cfg(feature = "adblock")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TitleKind {
    Ad,
    Track,
    Unknown,
}

#[cfg(feature = "adblock")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TitleInfo {
    kind: TitleKind,
    hash: u64,
}

#[cfg(feature = "adblock")]
impl TitleInfo {
    fn unknown() -> Self {
        Self {
            kind: TitleKind::Unknown,
            hash: 0,
        }
    }
}

#[cfg(feature = "adblock")]
#[derive(Clone, Debug)]
enum SkipResult {
    Accepted,
    Rejected,
    NotPlaying,
    NextDisabled,
    Unavailable(String),
}

#[cfg(feature = "adblock")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiscoveryState {
    Off,
    Waiting,
    Playing,
    Skipping,
    AdDetected,
    Muted,
    Error,
}

#[cfg(feature = "adblock")]
struct Discovery {
    enabled: bool,
    state: DiscoveryState,
    interval: Duration,
    max_skips: u32,
    skips: u32,
    ads: u32,
    cycles: u32,
    current: TitleInfo,
    audio_playing: bool,
    next_skip_due: Option<Instant>,
    skip_in_flight: bool,
    skip_accepted: bool,
    skip_wait_due: Option<Instant>,
    skip_from_hash: u64,
    skip_generation: Arc<AtomicU64>,
    unknown_since: Option<Instant>,
    detail: String,
}

#[cfg(feature = "adblock")]
impl Discovery {
    fn new() -> Self {
        Self {
            enabled: false,
            state: DiscoveryState::Off,
            interval: DISCOVERY_SKIP_INTERVAL,
            max_skips: DISCOVERY_MAX_SKIPS,
            skips: 0,
            ads: 0,
            cycles: 0,
            current: TitleInfo::unknown(),
            audio_playing: false,
            next_skip_due: None,
            skip_in_flight: false,
            skip_accepted: false,
            skip_wait_due: None,
            skip_from_hash: 0,
            skip_generation: Arc::new(AtomicU64::new(0)),
            unknown_since: None,
            detail: "Opt-in test mode. Never starts automatically.".to_string(),
        }
    }

    fn start(
        &mut self,
        interval: Duration,
        max_skips: u32,
        current: TitleInfo,
        audio_playing: bool,
        now: Instant,
    ) {
        self.enabled = true;
        self.interval = interval;
        self.max_skips = max_skips;
        self.skips = 0;
        self.ads = 0;
        self.cycles = 0;
        self.current = current;
        self.audio_playing = audio_playing;
        self.next_skip_due = None;
        self.skip_in_flight = false;
        self.skip_accepted = false;
        self.skip_wait_due = None;
        self.skip_generation.fetch_add(1, Ordering::Relaxed);
        self.unknown_since = if current.kind == TitleKind::Unknown {
            Some(now)
        } else {
            None
        };
        self.detail = "started".to_string();
        self.state = DiscoveryState::Waiting;
        if current.kind == TitleKind::Ad {
            self.state = DiscoveryState::AdDetected;
        } else if current.kind == TitleKind::Track && audio_playing {
            self.state = DiscoveryState::Playing;
            self.schedule_skip(now);
        }
        log(format!(
            "discovery=start interval={} max_skips={}",
            interval.as_secs(),
            max_skips
        ));
    }

    fn stop(&mut self, reason: &str) {
        if !self.enabled && self.state == DiscoveryState::Off {
            return;
        }
        self.enabled = false;
        self.state = DiscoveryState::Off;
        self.cancel_skip();
        self.unknown_since = None;
        self.detail = reason.to_string();
        log(format!("discovery=stop reason={reason}"));
    }

    fn fail(&mut self, reason: &str) {
        self.enabled = false;
        self.state = DiscoveryState::Error;
        self.cancel_skip();
        self.unknown_since = None;
        self.detail = reason.to_string();
        log(format!("discovery=error reason={reason}"));
    }

    fn schedule_skip(&mut self, now: Instant) {
        if !self.enabled || self.skip_in_flight || self.next_skip_due.is_some() {
            return;
        }
        if self.skips >= self.max_skips {
            self.fail("max skips reached");
            return;
        }
        if self.current.kind != TitleKind::Track || !self.audio_playing {
            return;
        }
        self.next_skip_due = Some(now + self.interval);
        self.state = DiscoveryState::Playing;
        self.detail = format!("next skip in {}s", self.interval.as_secs());
    }

    fn on_title(&mut self, info: TitleInfo, now: Instant) {
        let previous = self.current;
        self.current = info;
        if !self.enabled {
            return;
        }
        if info.kind == TitleKind::Ad {
            self.unknown_since = None;
            self.cancel_skip();
            if previous.kind != TitleKind::Ad {
                self.ads += 1;
                log(format!("discovery=ad_detected count={}", self.ads));
            }
            self.state = DiscoveryState::AdDetected;
            self.detail = "advertisement detected; skipping stopped".to_string();
            return;
        }
        if info.kind == TitleKind::Track {
            self.unknown_since = None;
            if self.skip_in_flight {
                if info.hash == self.skip_from_hash {
                    return;
                }
                self.skip_in_flight = false;
                self.skip_wait_due = None;
                log("discovery=skip_confirmed");
            }
            self.state = DiscoveryState::Playing;
            self.schedule_skip(now);
            return;
        }
        self.next_skip_due = None;
        if self.skip_in_flight && info.hash != self.skip_from_hash {
            self.cancel_skip();
            log("discovery=skip_unknown");
        }
        if self.state != DiscoveryState::Error {
            self.state = DiscoveryState::Waiting;
        }
        self.detail = "waiting for a confirmed track title".to_string();
        if self.unknown_since.is_none() {
            self.unknown_since = Some(now);
        }
    }

    fn on_audio(&mut self, playing: bool, now: Instant) {
        self.audio_playing = playing;
        if !self.enabled {
            return;
        }
        if playing {
            if self.current.kind == TitleKind::Track && !self.skip_in_flight {
                self.schedule_skip(now);
            }
        } else {
            self.next_skip_due = None;
            if self.state == DiscoveryState::Playing {
                self.state = DiscoveryState::Waiting;
                self.detail = "paused; waiting for playback".to_string();
            }
        }
    }

    fn on_page_loaded(&mut self, current: TitleInfo, now: Instant) {
        self.current = current;
        self.cancel_skip();
        self.unknown_since = if self.enabled && current.kind == TitleKind::Unknown {
            Some(now)
        } else {
            None
        };
        if self.enabled {
            match current.kind {
                TitleKind::Track => {
                    self.state = DiscoveryState::Playing;
                    self.detail = "page loaded; track detected".to_string();
                    self.schedule_skip(now);
                }
                TitleKind::Ad => {
                    self.state = DiscoveryState::AdDetected;
                    self.detail = "page loaded; advertisement detected".to_string();
                }
                TitleKind::Unknown => {
                    self.state = DiscoveryState::Waiting;
                    self.detail = "page reload; waiting for a track".to_string();
                }
            }
            log("discovery=page_load_reset");
        }
    }

    fn begin_skip(&mut self, now: Instant) -> u64 {
        self.next_skip_due = None;
        let generation = self.skip_generation.fetch_add(1, Ordering::Relaxed) + 1;
        self.skip_in_flight = true;
        self.skip_accepted = false;
        self.skip_wait_due = Some(now + DISCOVERY_SKIP_TIMEOUT);
        self.skip_from_hash = self.current.hash;
        self.state = DiscoveryState::Skipping;
        self.detail = "skip requested".to_string();
        log("discovery=skip_requested");
        generation
    }

    fn skip_token(&self) -> Arc<AtomicU64> {
        self.skip_generation.clone()
    }

    fn cancel_skip(&mut self) {
        self.skip_generation.fetch_add(1, Ordering::Relaxed);
        self.skip_in_flight = false;
        self.skip_accepted = false;
        self.skip_wait_due = None;
        self.next_skip_due = None;
    }

    fn on_skip_result(&mut self, generation: u64, result: SkipResult, now: Instant) {
        if generation != self.skip_generation.load(Ordering::Relaxed) {
            log("discovery=skip_result_stale");
            return;
        }
        if !self.enabled {
            return;
        }
        match result {
            SkipResult::Accepted => {
                if self.skip_accepted {
                    log("discovery=skip_accepted_duplicate");
                    return;
                }
                self.skip_accepted = true;
                self.skips = self.skips.saturating_add(1);
                log(format!("discovery=skip_accepted count={}", self.skips));
                if !self.skip_in_flight {
                    log("discovery=skip_accepted_after_title_change");
                    self.state = DiscoveryState::Playing;
                    self.schedule_skip(now);
                    return;
                }
                self.skip_wait_due = Some(now + DISCOVERY_SKIP_TIMEOUT);
                self.state = DiscoveryState::Skipping;
                self.detail = "skip accepted; waiting for title change".to_string();
            }
            SkipResult::Rejected => {
                self.fail("skip rejected by player");
            }
            SkipResult::NotPlaying => {
                if self.skip_in_flight {
                    self.skip_in_flight = false;
                    self.skip_wait_due = None;
                    self.state = DiscoveryState::Waiting;
                    self.detail = "player is not playing; waiting".to_string();
                    log("discovery=skip_not_playing");
                }
            }
            SkipResult::NextDisabled => {
                self.fail("next disabled (Spotify skip limit?)");
            }
            SkipResult::Unavailable(reason) => {
                self.fail(&reason);
            }
        }
    }

    fn tick(&mut self, now: Instant) -> Option<u64> {
        if !self.enabled {
            return None;
        }
        if let Some(since) = self.unknown_since {
            if now >= since + DISCOVERY_UNKNOWN_TIMEOUT {
                self.fail("playback state unknown for too long");
                return None;
            }
        }
        if let Some(due) = self.skip_wait_due {
            if now >= due {
                if self.skip_in_flight {
                    self.fail("skip did not change the title");
                    return None;
                }
                self.skip_wait_due = None;
            }
        }
        if let Some(due) = self.next_skip_due {
            if now >= due {
                if self.skip_in_flight {
                    self.next_skip_due = None;
                    return None;
                }
                if self.current.kind != TitleKind::Track || !self.audio_playing {
                    self.next_skip_due = None;
                    return None;
                }
                if self.skips >= self.max_skips {
                    self.fail("max skips reached");
                    return None;
                }
                return Some(self.begin_skip(now));
            }
        }
        None
    }

    fn next_deadline(&self) -> Option<Instant> {
        if !self.enabled {
            return None;
        }
        let mut deadline = None;
        for value in [
            self.next_skip_due,
            self.skip_wait_due,
            self.unknown_since
                .map(|since| since + DISCOVERY_UNKNOWN_TIMEOUT),
        ]
        .into_iter()
        .flatten()
        {
            deadline = Some(deadline.map_or(value, |current: Instant| current.min(value)));
        }
        deadline
    }

    fn label(&self) -> &'static str {
        match self.state {
            DiscoveryState::Off => "Off",
            DiscoveryState::Waiting => "Waiting",
            DiscoveryState::Playing => "Playing",
            DiscoveryState::Skipping => "Skipping",
            DiscoveryState::AdDetected => "Advertisement Detected",
            DiscoveryState::Muted => "Muted",
            DiscoveryState::Error => "Error",
        }
    }
}

#[cfg(feature = "adblock")]
struct AdGuard {
    active: bool,
    watchdog_due: Option<Instant>,
    watchdog: Duration,
    watchdog_extend: Duration,
    watchdog_max: Duration,
    muted_at: Option<Instant>,
    suppress_ad: bool,
    count_cycle: bool,
    audio_playing: bool,
    discovery: Discovery,
    dirty: bool,
    proxy: EventLoopProxy<UserEvent>,
}

#[cfg(feature = "adblock")]
impl AdGuard {
    fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            active: false,
            watchdog_due: None,
            watchdog: AD_WATCHDOG,
            watchdog_extend: AD_WATCHDOG_EXTEND,
            watchdog_max: AD_WATCHDOG_MAX,
            muted_at: None,
            suppress_ad: false,
            count_cycle: false,
            audio_playing: false,
            discovery: Discovery::new(),
            dirty: true,
            proxy,
        }
    }

    fn on_title(&mut self, info: TitleInfo, webview: &WebView) {
        let now = Instant::now();
        self.discovery.on_title(info, now);
        if info.kind == TitleKind::Ad {
            if !self.active && !self.suppress_ad {
                if self.mute_ad(webview, now) {
                    if self.discovery.enabled {
                        self.discovery.state = DiscoveryState::Muted;
                        self.discovery.detail = "advertisement muted".to_string();
                    }
                } else if self.discovery.enabled {
                    self.discovery.fail("ad mute failed");
                }
            } else if self.active && self.discovery.enabled {
                self.discovery.state = DiscoveryState::Muted;
            }
        } else {
            self.suppress_ad = false;
            if self.active {
                self.active = false;
                self.unmute(webview, "adblock=ad_end unmuted", true);
            }
        }
        self.dirty = true;
    }

    fn on_audio(&mut self, playing: bool) {
        self.audio_playing = playing;
        self.discovery.on_audio(playing, Instant::now());
        self.dirty = true;
    }

    fn on_page_loaded(&mut self, webview: &WebView) {
        let now = Instant::now();
        let current = read_title_info(webview);
        self.discovery.on_page_loaded(current, now);
        if current.kind == TitleKind::Ad {
            self.suppress_ad = false;
            if !self.active {
                self.mute_ad(webview, now);
            }
            if self.active {
                if self.muted_at.is_none() {
                    self.muted_at = Some(now);
                }
                let cap = self
                    .muted_at
                    .map(|since| since + self.watchdog_max)
                    .unwrap_or(now + self.watchdog_max);
                self.watchdog_due = Some((now + self.watchdog).min(cap));
                if self.discovery.enabled {
                    self.discovery.state = DiscoveryState::Muted;
                    self.discovery.detail = "page loaded; advertisement muted".to_string();
                }
                log("adblock=page_load_ad muted");
            } else if self.discovery.enabled {
                self.discovery.fail("ad mute failed");
            }
        } else {
            self.active = false;
            self.suppress_ad = false;
            self.count_cycle = false;
            self.muted_at = None;
            self.unmute(webview, "adblock=page_load unmuted", false);
        }
        self.dirty = true;
    }

    fn start_discovery(&mut self, webview: &WebView, interval: Duration, max_skips: u32) {
        if self.discovery.enabled {
            return;
        }
        let now = Instant::now();
        let current = read_title_info(webview);
        self.suppress_ad = false;
        self.discovery
            .start(interval, max_skips, current, self.audio_playing, now);
        if current.kind == TitleKind::Ad {
            if !self.active {
                self.mute_ad(webview, now);
            }
            if self.active {
                self.discovery.state = DiscoveryState::Muted;
                self.discovery.detail = "advertisement muted".to_string();
            } else if self.discovery.enabled {
                self.discovery.fail("ad mute failed");
            }
        }
        self.dirty = true;
    }

    fn stop_discovery(&mut self, reason: &str) {
        self.discovery.stop(reason);
        self.dirty = true;
    }

    fn on_skip_result(&mut self, generation: u64, result: SkipResult) {
        self.discovery
            .on_skip_result(generation, result, Instant::now());
        self.dirty = true;
    }

    fn mute_ad(&mut self, webview: &WebView, now: Instant) -> bool {
        if set_muted(webview, true) {
            self.active = true;
            self.muted_at = Some(now);
            self.watchdog_due = Some(now + self.watchdog);
            self.count_cycle = true;
            log("adblock=ad_start muted");
            true
        } else {
            self.active = false;
            self.muted_at = None;
            self.watchdog_due = None;
            self.count_cycle = false;
            log("adblock=ad_start mute_failed; fail-open");
            false
        }
    }

    fn unmute(&mut self, webview: &WebView, success_message: &str, count: bool) {
        if set_muted(webview, false) {
            self.watchdog_due = None;
            self.muted_at = None;
            if count && self.count_cycle && self.discovery.enabled {
                self.discovery.cycles = self.discovery.cycles.saturating_add(1);
                self.discovery.detail = "mute/unmute cycle complete".to_string();
            }
            self.count_cycle = false;
            log(success_message);
        } else {
            self.watchdog_due = Some(Instant::now() + AD_RETRY);
            log("adblock=unmute_failed; retry scheduled");
        }
    }

    fn tick(&mut self, webview: &WebView) {
        let now = Instant::now();
        if let Some(due) = self.watchdog_due {
            if now >= due {
                if self.active {
                    let info = read_title_info(webview);
                    if info.kind == TitleKind::Ad && !self.suppress_ad {
                        let muted_for = self
                            .muted_at
                            .map(|since| now.duration_since(since))
                            .unwrap_or_default();
                        if muted_for < self.watchdog_max {
                            self.watchdog_due = Some(now + self.watchdog_extend);
                            log(format!(
                                "adblock=watchdog_extend muted_ms={}",
                                muted_for.as_millis()
                            ));
                            self.discovery.detail =
                                "ad still detected; watchdog extended".to_string();
                            self.dirty = true;
                        } else {
                            self.active = false;
                            self.suppress_ad = true;
                            self.count_cycle = false;
                            self.muted_at = None;
                            if set_muted(webview, false) {
                                self.watchdog_due = None;
                                log("adblock=watchdog_fail_open unmuted");
                            } else {
                                self.watchdog_due = Some(now + AD_RETRY);
                                log("adblock=watchdog_fail_open unmute_failed; retry scheduled");
                            }
                            if self.discovery.enabled {
                                self.discovery.fail("watchdog fail-open");
                            }
                            self.dirty = true;
                        }
                    } else {
                        self.active = false;
                        self.suppress_ad = false;
                        self.unmute(webview, "adblock=watchdog_unmute", true);
                        self.dirty = true;
                    }
                } else if set_muted(webview, false) {
                    self.watchdog_due = None;
                    if self.count_cycle && self.discovery.enabled {
                        self.discovery.cycles = self.discovery.cycles.saturating_add(1);
                        self.discovery.detail = "mute/unmute cycle complete".to_string();
                    }
                    self.count_cycle = false;
                    self.muted_at = None;
                    log("adblock=unmute_retry_ok");
                    self.dirty = true;
                } else {
                    self.watchdog_due = Some(now + AD_RETRY);
                    log("adblock=unmute_retry_failed");
                }
            }
        }
        if let Some(generation) = self.discovery.tick(now) {
            let expected_title = read_title_string(webview).unwrap_or_default();
            let token = self.discovery.skip_token();
            request_smtc_skip(self.proxy.clone(), generation, expected_title, token);
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        let mut deadline = self.watchdog_due;
        if let Some(discovery_due) = self.discovery.next_deadline() {
            deadline = Some(deadline.map_or(discovery_due, |current| current.min(discovery_due)));
        }
        deadline
    }

    fn take_dirty(&mut self) -> bool {
        let dirty = self.dirty;
        self.dirty = false;
        dirty
    }

    fn render(&self, bar: &WebView) {
        let state = if self.discovery.state == DiscoveryState::Error {
            "Error"
        } else if self.active {
            "Muted"
        } else {
            self.discovery.label()
        };
        let detail = json_escape(&self.discovery.detail);
        let script = format!(
            "window.setAdblockState && window.setAdblockState({{enabled:{},state:\"{}\",skips:{},ads:{},cycles:{},muted:{},detail:\"{}\"}})",
            self.discovery.enabled,
            state,
            self.discovery.skips,
            self.discovery.ads,
            self.discovery.cycles,
            self.active,
            detail
        );
        if let Err(error) = bar.evaluate_script(&script) {
            log(format!("adblock_ui_error={error}"));
        }
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
    // WebView2 does not publish its media session to Windows by default. The
    // adblock experiment needs SMTC to skip tracks during discovery, so enable
    // Chromium's documented media-session integration for that build only.
    #[cfg(feature = "adblock")]
    browser_args.push_str(" --enable-features=MediaSessionService");
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
            let bench_local = cfg!(feature = "bench")
                && parsed.scheme() == "http"
                && matches!(parsed.host_str(), Some("127.0.0.1") | Some("localhost"));
            if (parsed.scheme() == "https" && is_allowed_host(parsed.host_str())) || bench_local {
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
    let bar_html = BAR_HTML
        .replace("<!--ADBLOCK_BUTTON-->", ADBLOCK_BUTTON_HTML)
        .replace("<!--ADBLOCK_PANEL-->", ADBLOCK_PANEL_HTML)
        .replace("<!--ADBLOCK_SCRIPT-->", ADBLOCK_PANEL_SCRIPT);
    let bar_html = if start_maximized {
        bar_html.replacen("<body>", "<body class=\"maximized\">", 1)
    } else {
        bar_html
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
            let body = request.body();
            let event = match body.as_str() {
                "drag" => Some(UserEvent::Drag),
                "minimize" => Some(UserEvent::Minimize),
                "toggle-max" => Some(UserEvent::ToggleMax),
                "close" => Some(UserEvent::Close),
                #[cfg(feature = "adblock")]
                "adblock-panel" => Some(UserEvent::ToggleAdblockPanel),
                #[cfg(feature = "adblock")]
                "adblock-stop" => Some(UserEvent::DiscoveryStop),
                #[cfg(feature = "adblock")]
                other if other.starts_with("adblock-start:") => {
                    let mut parts = other["adblock-start:".len()..].split(':');
                    let interval_secs = parts
                        .next()
                        .and_then(|value| value.parse::<u64>().ok())
                        .unwrap_or(5)
                        .clamp(2, 60);
                    let max_skips = parts
                        .next()
                        .and_then(|value| value.parse::<u32>().ok())
                        .unwrap_or(50)
                        .clamp(1, 500);
                    Some(UserEvent::DiscoveryStart {
                        interval_secs,
                        max_skips,
                    })
                }
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

    #[cfg(feature = "adblock")]
    if let Err(error) = watch_adblock(&webview, proxy.clone()) {
        log(format!("adblock_watch_error={error}"));
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
    #[cfg(feature = "adblock")]
    let mut adguard = AdGuard::new(proxy.clone());
    #[cfg(feature = "adblock")]
    let mut bar_height = BAR_HEIGHT;
    #[cfg(feature = "adblock")]
    let mut panel_open = false;
    let mut last_maximized = window.is_maximized();

    #[cfg(feature = "adblock")]
    {
        #[cfg(feature = "bench")]
        {
            if let Some(secs) = arg_value(&args, "--adblock-watchdog-secs")
                .and_then(|value| value.parse::<u64>().ok())
            {
                adguard.watchdog = Duration::from_secs(secs.max(1));
            }
            if let Some(secs) = arg_value(&args, "--adblock-watchdog-extend-secs")
                .and_then(|value| value.parse::<u64>().ok())
            {
                adguard.watchdog_extend = Duration::from_secs(secs.max(1));
            }
            if let Some(secs) = arg_value(&args, "--adblock-watchdog-max-secs")
                .and_then(|value| value.parse::<u64>().ok())
            {
                adguard.watchdog_max = Duration::from_secs(secs.max(1));
            }
        }
        adguard.render(&bar);
        #[cfg(feature = "bench")]
        if args.iter().any(|arg| arg == "--adblock-autostart") {
            let interval = arg_value(&args, "--adblock-interval")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(DISCOVERY_SKIP_INTERVAL.as_secs())
                .clamp(2, 60);
            let max_skips = arg_value(&args, "--adblock-max-skips")
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(DISCOVERY_MAX_SKIPS)
                .clamp(1, 500);
            adguard.start_discovery(&webview, Duration::from_secs(interval), max_skips);
        }
    }

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
                #[cfg(feature = "adblock")]
                layout(&window, &webview, Some(&bar), bar_height);
                #[cfg(not(feature = "adblock"))]
                layout(&window, &webview, Some(&bar), BAR_HEIGHT);
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
                #[cfg(feature = "adblock")]
                layout(&window, &webview, Some(&bar), bar_height);
                #[cfg(not(feature = "adblock"))]
                layout(&window, &webview, Some(&bar), BAR_HEIGHT);
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
                #[cfg(feature = "adblock")]
                adguard.on_page_loaded(&webview);
            }
            Event::UserEvent(UserEvent::AudioChanged(playing)) => {
                policy.on_audio_changed(playing);
                #[cfg(feature = "adblock")]
                adguard.on_audio(playing);
            }
            #[cfg(feature = "adblock")]
            Event::UserEvent(UserEvent::TitleChanged(info)) => {
                adguard.on_title(info, &webview);
            }
            #[cfg(feature = "adblock")]
            Event::UserEvent(UserEvent::SkipResult { generation, result }) => {
                adguard.on_skip_result(generation, result);
            }
            #[cfg(feature = "adblock")]
            Event::UserEvent(UserEvent::DiscoveryStart {
                interval_secs,
                max_skips,
            }) => {
                adguard.start_discovery(&webview, Duration::from_secs(interval_secs), max_skips);
            }
            #[cfg(feature = "adblock")]
            Event::UserEvent(UserEvent::DiscoveryStop) => {
                adguard.stop_discovery("stopped by user");
            }
            #[cfg(feature = "adblock")]
            Event::UserEvent(UserEvent::ToggleAdblockPanel) => {
                panel_open = !panel_open;
                bar_height = if panel_open {
                    BAR_HEIGHT + PANEL_HEIGHT
                } else {
                    BAR_HEIGHT
                };
                let _ = bar.evaluate_script(&format!(
                    "window.setPanelOpen && window.setPanelOpen({panel_open})"
                ));
                layout(&window, &webview, Some(&bar), bar_height);
            }
            _ => {}
        }

        if !no_trim {
            policy.tick(&webview, Some(&bar));
        }
        #[cfg(feature = "adblock")]
        adguard.tick(&webview);
        #[cfg(feature = "adblock")]
        if adguard.take_dirty() {
            adguard.render(&bar);
        }
        #[allow(unused_mut)]
        let mut deadline = if no_trim {
            None
        } else {
            policy.next_deadline()
        };
        #[cfg(feature = "adblock")]
        if let Some(ad_due) = adguard.next_deadline() {
            deadline = Some(deadline.map_or(ad_due, |policy_due| policy_due.min(ad_due)));
        }
        if let Some(deadline) = deadline {
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

#[cfg(feature = "adblock")]
fn watch_adblock(
    webview: &WebView,
    proxy: EventLoopProxy<UserEvent>,
) -> Result<(), Box<dyn Error>> {
    let core: ICoreWebView2 = webview.webview().cast()?;
    let handler_proxy = proxy.clone();
    let handler = DocumentTitleChangedEventHandler::create(Box::new(
        move |sender: Option<ICoreWebView2>, _args: Option<windows::core::IUnknown>| {
            if let Some(sender) = sender {
                if let Some(title) = read_title(&sender) {
                    let _ = handler_proxy.send_event(UserEvent::TitleChanged(title_info(&title)));
                }
            }
            Ok(())
        },
    ));
    let mut token = 0i64;
    unsafe {
        core.add_DocumentTitleChanged(&handler, &mut token)?;
    }
    if let Some(title) = read_title(&core) {
        let _ = proxy.send_event(UserEvent::TitleChanged(title_info(&title)));
    }
    log(format!("adblock_watch_ready token={token}"));
    Ok(())
}

#[cfg(feature = "adblock")]
fn read_title(core: &ICoreWebView2) -> Option<String> {
    let mut title = windows::core::PWSTR::null();
    unsafe { core.DocumentTitle(&mut title) }.ok()?;
    Some(take_pwstr(title))
}

#[cfg(feature = "adblock")]
fn set_muted(webview: &WebView, muted: bool) -> bool {
    let core = match webview.webview().cast::<ICoreWebView2_8>() {
        Ok(core) => core,
        Err(error) => {
            log(format!("adblock_mute_error muted={muted} error={error}"));
            return false;
        }
    };
    match unsafe { core.SetIsMuted(muted) } {
        Ok(()) => true,
        Err(error) => {
            log(format!("adblock_mute_error muted={muted} error={error}"));
            false
        }
    }
}

#[cfg(feature = "adblock")]
const TITLE_SEPARATORS: [char; 6] = ['-', '\u{2013}', '\u{2014}', '\u{2022}', '\u{00b7}', '|'];

#[cfg(feature = "adblock")]
fn title_is_ad(title: &str) -> bool {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lower = trimmed.to_lowercase();
    if lower == "advertisement" {
        return true;
    }
    lower
        .rsplit(TITLE_SEPARATORS)
        .next()
        .map(str::trim)
        .is_some_and(|segment| segment == "advertisement")
}

#[cfg(feature = "adblock")]
fn has_ad_segment_lower(lower: &str) -> bool {
    lower
        .split(TITLE_SEPARATORS)
        .map(str::trim)
        .any(|segment| segment == "advertisement")
}

#[cfg(feature = "adblock")]
fn is_player_chrome_title(lower: &str) -> bool {
    lower == "spotify"
        || lower.contains("web player")
        || lower.contains("music for everyone")
        || lower.contains("sign up")
        || lower.contains("log in")
}

#[cfg(feature = "adblock")]
fn classify_title(title: &str) -> TitleKind {
    if title_is_ad(title) {
        return TitleKind::Ad;
    }
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return TitleKind::Unknown;
    }
    let lower = trimmed.to_lowercase();
    if is_player_chrome_title(&lower) {
        return TitleKind::Unknown;
    }
    // A title that mentions an advertisement but does not match the confirmed
    // parser is treated as unknown, never as a track. This keeps discovery from
    // skipping something that may be an ad.
    if has_ad_segment_lower(&lower) {
        return TitleKind::Unknown;
    }
    let mut segments = lower
        .split(TITLE_SEPARATORS)
        .map(str::trim)
        .filter(|segment| !segment.is_empty());
    let first = segments.next();
    let second = segments.next();
    if first.is_some() && second.is_some() {
        TitleKind::Track
    } else {
        TitleKind::Unknown
    }
}

#[cfg(feature = "adblock")]
fn title_hash(title: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    title.trim().to_lowercase().hash(&mut hasher);
    hasher.finish()
}

#[cfg(feature = "adblock")]
fn title_info(title: &str) -> TitleInfo {
    TitleInfo {
        kind: classify_title(title),
        hash: title_hash(title),
    }
}

#[cfg(feature = "adblock")]
fn read_title_string(webview: &WebView) -> Option<String> {
    let core = webview.webview().cast::<ICoreWebView2>().ok()?;
    read_title(&core)
}

#[cfg(feature = "adblock")]
fn read_title_info(webview: &WebView) -> TitleInfo {
    read_title_string(webview)
        .map(|title| title_info(&title))
        .unwrap_or_else(TitleInfo::unknown)
}

#[cfg(feature = "adblock")]
fn json_escape(value: &str) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(feature = "adblock")]
fn smtc_aumid_is_webview(aumid: &str) -> bool {
    let lower = aumid.to_lowercase();
    lower.contains("webview") || lower.contains("rustify") || lower.contains("win32webviewhost")
}

#[cfg(feature = "adblock")]
fn request_smtc_skip(
    proxy: EventLoopProxy<UserEvent>,
    generation: u64,
    expected_title: String,
    token: Arc<AtomicU64>,
) {
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession,
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus,
    };

    let operation = match GlobalSystemMediaTransportControlsSessionManager::RequestAsync() {
        Ok(operation) => operation,
        Err(error) => {
            let _ = proxy.send_event(UserEvent::SkipResult {
                generation,
                result: SkipResult::Unavailable(format!("session manager request failed: {error}")),
            });
            return;
        }
    };
    let outer_proxy = proxy.clone();
    let when_result = operation.when(move |result| {
        let manager = match result {
            Ok(manager) => manager,
            Err(error) => {
                let _ = outer_proxy.send_event(UserEvent::SkipResult {
                    generation,
                    result: SkipResult::Unavailable(format!(
                        "session manager unavailable: {error}"
                    )),
                });
                return;
            }
        };
        let sessions = match manager.GetSessions() {
            Ok(sessions) => sessions,
            Err(error) => {
                let _ = outer_proxy.send_event(UserEvent::SkipResult {
                    generation,
                    result: SkipResult::Unavailable(format!("session list failed: {error}")),
                });
                return;
            }
        };
        let total = sessions.Size().unwrap_or(0);
        let mut candidates: Vec<GlobalSystemMediaTransportControlsSession> = Vec::new();
        for session in &sessions {
            let aumid = session
                .SourceAppUserModelId()
                .map(|value| value.to_string())
                .unwrap_or_default();
            if smtc_aumid_is_webview(&aumid) {
                if cfg!(debug_assertions) {
                    log(format!("discovery=smtc_match aumid={aumid}"));
                }
                candidates.push(session);
            } else if cfg!(debug_assertions) {
                log(format!("discovery=smtc_other aumid={aumid}"));
            }
        }
        log(format!(
            "discovery=smtc_sessions total={total} webview={}",
            candidates.len()
        ));
        // Another playing app means the media session is ambiguous. Fail closed
        // instead of risking a skip in a different application.
        if total == 0 {
            let _ = outer_proxy.send_event(UserEvent::SkipResult {
                generation,
                result: SkipResult::Unavailable("no media sessions".to_string()),
            });
            return;
        }
        if total != 1 {
            let _ = outer_proxy.send_event(UserEvent::SkipResult {
                generation,
                result: SkipResult::Unavailable(format!(
                    "ambiguous media sessions: {total}"
                )),
            });
            return;
        }
        let session = match candidates.len() {
            1 => candidates.remove(0),
            0 => {
                let _ = outer_proxy.send_event(UserEvent::SkipResult {
                    generation,
                    result: SkipResult::Unavailable("no WebView2 media session".to_string()),
                });
                return;
            }
            _ => {
                let _ = outer_proxy.send_event(UserEvent::SkipResult {
                    generation,
                    result: SkipResult::Unavailable(
                        "multiple WebView2 media sessions".to_string(),
                    ),
                });
                return;
            }
        };
        let playback = match session.GetPlaybackInfo() {
            Ok(playback) => playback,
            Err(error) => {
                let _ = outer_proxy.send_event(UserEvent::SkipResult {
                    generation,
                    result: SkipResult::Unavailable(format!("playback info failed: {error}")),
                });
                return;
            }
        };
        let status = match playback.PlaybackStatus() {
            Ok(status) => status,
            Err(error) => {
                let _ = outer_proxy.send_event(UserEvent::SkipResult {
                    generation,
                    result: SkipResult::Unavailable(format!("playback status failed: {error}")),
                });
                return;
            }
        };
        if status != GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing {
            let _ = outer_proxy.send_event(UserEvent::SkipResult {
                generation,
                result: SkipResult::NotPlaying,
            });
            return;
        }
        let controls = match playback.Controls() {
            Ok(controls) => controls,
            Err(error) => {
                let _ = outer_proxy.send_event(UserEvent::SkipResult {
                    generation,
                    result: SkipResult::Unavailable(format!("playback controls failed: {error}")),
                });
                return;
            }
        };
        match controls.IsNextEnabled() {
            Ok(true) => {}
            Ok(false) => {
                let _ = outer_proxy.send_event(UserEvent::SkipResult {
                    generation,
                    result: SkipResult::NextDisabled,
                });
                return;
            }
            Err(error) => {
                let _ = outer_proxy.send_event(UserEvent::SkipResult {
                    generation,
                    result: SkipResult::Unavailable(format!("next control failed: {error}")),
                });
                return;
            }
        }
        smtc_verify_and_skip(session, outer_proxy, generation, expected_title, token);
    });
    if let Err(error) = when_result {
        let _ = proxy.send_event(UserEvent::SkipResult {
            generation,
            result: SkipResult::Unavailable(format!(
                "session completion handler failed: {error}"
            )),
        });
    }
}

#[cfg(feature = "adblock")]
fn normalize_title(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut pending_space = false;
    for ch in value.chars() {
        if ch.is_alphanumeric() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.extend(ch.to_lowercase());
        } else {
            pending_space = true;
        }
    }
    out
}

#[cfg(feature = "adblock")]
fn smtc_title_matches(document_title: &str, media_title: &str) -> bool {
    let document = normalize_title(document_title);
    let media = normalize_title(media_title);
    if document.is_empty() || media.is_empty() {
        return false;
    }
    document.contains(&media) || (document.len() >= 5 && media.contains(&document))
}

#[cfg(feature = "adblock")]
fn smtc_verify_and_skip(
    session: windows::Media::Control::GlobalSystemMediaTransportControlsSession,
    proxy: EventLoopProxy<UserEvent>,
    generation: u64,
    expected_title: String,
    token: Arc<AtomicU64>,
) {
    if token.load(Ordering::Relaxed) != generation {
        return;
    }
    let properties = match session.TryGetMediaPropertiesAsync() {
        Ok(properties) => properties,
        Err(error) => {
            let _ = proxy.send_event(UserEvent::SkipResult {
                generation,
                result: SkipResult::Unavailable(format!(
                    "media properties request failed: {error}"
                )),
            });
            return;
        }
    };
    let skip_proxy = proxy.clone();
    let when_properties = properties.when(move |result| {
        let media_title = match result {
            Ok(properties) => properties
                .Title()
                .map(|value| value.to_string())
                .unwrap_or_default(),
            Err(error) => {
                let _ = skip_proxy.send_event(UserEvent::SkipResult {
                    generation,
                    result: SkipResult::Unavailable(format!(
                        "media properties unavailable: {error}"
                    )),
                });
                return;
            }
        };
        let matched = smtc_title_matches(&expected_title, &media_title);
        if cfg!(debug_assertions) {
            log(format!("discovery=smtc_title_match matched={matched}"));
        }
        if !matched {
            let _ = skip_proxy.send_event(UserEvent::SkipResult {
                generation,
                result: SkipResult::Unavailable(
                    "media session title does not match the player".to_string(),
                ),
            });
            return;
        }
        if token.load(Ordering::Relaxed) != generation {
            return;
        }
        let skip = match session.TrySkipNextAsync() {
            Ok(skip) => skip,
            Err(error) => {
                let _ = skip_proxy.send_event(UserEvent::SkipResult {
                    generation,
                    result: SkipResult::Unavailable(format!("skip request failed: {error}")),
                });
                return;
            }
        };
        let skip_result_proxy = skip_proxy.clone();
        let when_skip = skip.when(move |result| {
            let outcome = match result {
                Ok(true) => SkipResult::Accepted,
                Ok(false) => SkipResult::Rejected,
                Err(error) => SkipResult::Unavailable(format!("skip failed: {error}")),
            };
            let _ = skip_result_proxy.send_event(UserEvent::SkipResult {
                generation,
                result: outcome,
            });
        });
        if let Err(error) = when_skip {
            let _ = skip_proxy.send_event(UserEvent::SkipResult {
                generation,
                result: SkipResult::Unavailable(format!(
                    "skip completion handler failed: {error}"
                )),
            });
        }
    });
    if let Err(error) = when_properties {
        let _ = proxy.send_event(UserEvent::SkipResult {
            generation,
            result: SkipResult::Unavailable(format!(
                "media properties completion handler failed: {error}"
            )),
        });
    }
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

fn layout(window: &Window, main: &WebView, bar: Option<&WebView>, bar_height: f64) {
    let logical = window
        .inner_size()
        .to_logical::<f64>(window.scale_factor());
    if let Some(bar) = bar {
        let _ = bar.set_bounds(chrome_rect(0.0, 0.0, logical.width.max(1.0), bar_height));
    }
    let _ = main.set_bounds(chrome_rect(
        0.0,
        bar_height,
        logical.width.max(1.0),
        (logical.height - bar_height).max(1.0),
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
    #[cfg(feature = "adblock")]
    let ad_note = "Adblocking experiment enabled: advertisements may be muted.\n\
                   Playback and DRM are handled by Spotify's own player.";
    #[cfg(not(feature = "adblock"))]
    let ad_note = "Ads, playback, and DRM are handled by Spotify's own player.";
    let text = format!(
        "{APP_NAME} {APP_VERSION}\n\n\
         An unofficial native host for the official Spotify Web Player.\n\
         {ad_note}\n\n\
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
    let explicit = arg_value(&args, "--ublock-ext")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("RUSTIFY_UBLOCK_EXT").map(PathBuf::from));
    if let Some(path) = explicit {
        if path.join("manifest.json").is_file() {
            return Some(path);
        }
        log(format!(
            "ublock_ext_invalid path={} (manifest.json not found)",
            path.display()
        ));
    }
    let bundled = std::env::current_exe().ok().and_then(|exe| {
        exe.parent()
            .map(|dir| dir.join("extensions").join("ublock"))
    })?;
    bundled.join("manifest.json").is_file().then_some(bundled)
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

#[cfg(all(test, feature = "adblock"))]
mod adblock_tests {
    use super::{
        classify_title, smtc_title_matches, title_is_ad, Discovery, DiscoveryState, SkipResult,
        TitleInfo, TitleKind,
    };
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};

    fn track(hash: u64) -> TitleInfo {
        TitleInfo {
            kind: TitleKind::Track,
            hash,
        }
    }

    fn ad() -> TitleInfo {
        TitleInfo {
            kind: TitleKind::Ad,
            hash: 999,
        }
    }

    fn unknown() -> TitleInfo {
        TitleInfo::unknown()
    }

    #[test]
    fn detects_plain_advertisement() {
        assert!(title_is_ad("Advertisement"));
        assert!(title_is_ad("  Advertisement  "));
    }

    #[test]
    fn detects_advertisement_suffix() {
        assert!(title_is_ad("Spotify - Advertisement"));
        assert!(title_is_ad("Spotify \u{2013} Advertisement"));
        assert!(title_is_ad("Spotify \u{2022} Advertisement"));
    }

    #[test]
    fn ignores_track_titles() {
        assert!(!title_is_ad(""));
        assert!(!title_is_ad("Spotify"));
        assert!(!title_is_ad("Advertisement - Spotify"));
        assert!(!title_is_ad("Track - Artist"));
    }

    #[test]
    fn classifies_titles() {
        assert_eq!(classify_title("Spotify - Advertisement"), TitleKind::Ad);
        assert_eq!(classify_title("Advertisement"), TitleKind::Ad);
        assert_eq!(
            classify_title("Advertisement - Spotify"),
            TitleKind::Unknown
        );
        assert_eq!(classify_title("Track - Artist"), TitleKind::Track);
        assert_eq!(classify_title("Track \u{2013} Artist"), TitleKind::Track);
        assert_eq!(
            classify_title("Spotify - Web Player: Music for everyone"),
            TitleKind::Unknown
        );
        assert_eq!(
            classify_title("Spotify \u{2013} Web Player: Music for everyone"),
            TitleKind::Unknown
        );
        assert_eq!(
            classify_title("Spotify \u{2022} Web Player"),
            TitleKind::Unknown
        );
        assert_eq!(classify_title("Log in - Spotify"), TitleKind::Unknown);
        assert_eq!(classify_title("Spotify"), TitleKind::Unknown);
        assert_eq!(classify_title("Track"), TitleKind::Unknown);
        assert_eq!(classify_title("Anzeige"), TitleKind::Unknown);
        assert_eq!(classify_title("Publicit\u{00e9}"), TitleKind::Unknown);
    }

    #[test]
    fn smtc_title_match_normalizes_and_fails_closed() {
        assert!(smtc_title_matches("Track 1 - Test Artist", "Track 1"));
        assert!(smtc_title_matches(
            "Song \u{2022} Artist - Spotify",
            "Song \u{2022} Artist"
        ));
        assert!(smtc_title_matches("Song Name", "Song Name \u{2022} Artist"));
        assert!(!smtc_title_matches("Unrelated - Artist", "Track 1"));
        assert!(!smtc_title_matches("Track 1 - Test Artist", ""));
        assert!(!smtc_title_matches("", "Track 1"));
    }

    #[test]
    fn discovery_start_unknown_times_out() {
        let now = Instant::now();
        let mut discovery = Discovery::new();
        discovery.start(Duration::from_secs(5), 10, unknown(), true, now);
        assert_eq!(discovery.state, DiscoveryState::Waiting);
        assert!(discovery.unknown_since.is_some());
        assert!(discovery
            .tick(now + Duration::from_secs(30))
            .is_none());
        assert_eq!(discovery.state, DiscoveryState::Error);
        assert!(!discovery.enabled);
    }

    #[test]
    fn discovery_schedules_track_and_stops_on_ad() {
        let now = Instant::now();
        let mut discovery = Discovery::new();
        discovery.start(Duration::from_secs(5), 10, track(1), true, now);
        assert!(discovery.enabled);
        assert_eq!(discovery.state, DiscoveryState::Playing);
        assert_eq!(
            discovery.next_skip_due,
            Some(now + Duration::from_secs(5))
        );

        assert!(discovery
            .tick(now + Duration::from_secs(4))
            .is_none());
        let generation = discovery
            .tick(now + Duration::from_secs(5))
            .expect("skip should be issued");
        assert_eq!(discovery.state, DiscoveryState::Skipping);
        assert_eq!(discovery.skips, 0);
        assert!(discovery.skip_in_flight);
        assert!(discovery
            .tick(now + Duration::from_secs(6))
            .is_none());
        let token = discovery.skip_token();
        assert_eq!(token.load(Ordering::Relaxed), generation);

        discovery.on_title(ad(), now + Duration::from_secs(7));
        assert_eq!(discovery.state, DiscoveryState::AdDetected);
        assert_eq!(discovery.ads, 1);
        assert!(!discovery.skip_in_flight);
        assert!(discovery.next_skip_due.is_none());
        assert_ne!(token.load(Ordering::Relaxed), generation);

        discovery.on_skip_result(generation, SkipResult::Accepted, now + Duration::from_secs(8));
        assert_eq!(discovery.state, DiscoveryState::AdDetected);
        assert_eq!(discovery.skips, 0);
    }

    #[test]
    fn discovery_resumes_after_ad() {
        let now = Instant::now();
        let mut discovery = Discovery::new();
        discovery.start(Duration::from_secs(5), 10, track(1), true, now);
        discovery.on_title(ad(), now + Duration::from_secs(1));
        assert_eq!(discovery.state, DiscoveryState::AdDetected);
        discovery.on_title(track(2), now + Duration::from_secs(2));
        assert_eq!(discovery.state, DiscoveryState::Playing);
        assert_eq!(
            discovery.next_skip_due,
            Some(now + Duration::from_secs(7))
        );
    }

    #[test]
    fn discovery_does_not_skip_unknown_or_ad() {
        let now = Instant::now();
        let mut discovery = Discovery::new();
        discovery.start(Duration::from_secs(5), 10, unknown(), true, now);
        assert_eq!(discovery.state, DiscoveryState::Waiting);
        assert!(discovery.next_skip_due.is_none());
        discovery.on_title(ad(), now + Duration::from_secs(1));
        assert_eq!(discovery.state, DiscoveryState::AdDetected);
        assert!(discovery.next_skip_due.is_none());
        discovery.on_title(unknown(), now + Duration::from_secs(2));
        assert_eq!(discovery.state, DiscoveryState::Waiting);
        assert!(discovery.next_skip_due.is_none());
    }

    #[test]
    fn discovery_pause_cancels_pending_skip() {
        let now = Instant::now();
        let mut discovery = Discovery::new();
        discovery.start(Duration::from_secs(5), 10, track(1), true, now);
        assert!(discovery.next_skip_due.is_some());
        discovery.on_audio(false, now + Duration::from_secs(1));
        assert_eq!(discovery.state, DiscoveryState::Waiting);
        assert!(discovery.next_skip_due.is_none());
        assert!(discovery
            .tick(now + Duration::from_secs(10))
            .is_none());
    }

    #[test]
    fn discovery_max_skips_stops() {
        let now = Instant::now();
        let mut discovery = Discovery::new();
        discovery.start(Duration::from_secs(1), 1, track(1), true, now);
        let generation = discovery
            .tick(now + Duration::from_secs(1))
            .expect("skip should be issued");
        discovery.on_skip_result(generation, SkipResult::Accepted, now + Duration::from_secs(1));
        assert_eq!(discovery.skips, 1);
        discovery.on_title(track(2), now + Duration::from_secs(2));
        assert_eq!(discovery.state, DiscoveryState::Error);
        assert!(!discovery.enabled);
        assert!(discovery.next_skip_due.is_none());
        assert_eq!(discovery.skips, 1);
    }

    #[test]
    fn discovery_rejected_skip_stops() {
        let now = Instant::now();
        let mut discovery = Discovery::new();
        discovery.start(Duration::from_secs(1), 10, track(1), true, now);
        let generation = discovery
            .tick(now + Duration::from_secs(1))
            .expect("skip should be issued");
        discovery.on_skip_result(generation, SkipResult::Rejected, now + Duration::from_secs(2));
        assert_eq!(discovery.state, DiscoveryState::Error);
        assert!(!discovery.enabled);
    }

    #[test]
    fn discovery_not_playing_waits_without_counting() {
        let now = Instant::now();
        let mut discovery = Discovery::new();
        discovery.start(Duration::from_secs(1), 10, track(1), true, now);
        let generation = discovery
            .tick(now + Duration::from_secs(1))
            .expect("skip should be issued");
        assert_eq!(discovery.skips, 0);
        discovery.on_skip_result(generation, SkipResult::NotPlaying, now + Duration::from_secs(2));
        assert_eq!(discovery.state, DiscoveryState::Waiting);
        assert_eq!(discovery.skips, 0);
        assert!(!discovery.skip_in_flight);
    }

    #[test]
    fn discovery_disabled_has_no_deadline() {
        let now = Instant::now();
        let mut discovery = Discovery::new();
        discovery.on_page_loaded(unknown(), now);
        assert!(discovery.unknown_since.is_none());
        assert!(discovery.next_deadline().is_none());

        discovery.start(Duration::from_secs(5), 10, unknown(), true, now);
        assert!(discovery.next_deadline().is_some());
    }

    #[test]
    fn discovery_counts_accepted_skip_after_title_change() {
        let now = Instant::now();
        let mut discovery = Discovery::new();
        discovery.start(Duration::from_secs(1), 10, track(1), true, now);
        let generation = discovery
            .tick(now + Duration::from_secs(1))
            .expect("skip should be issued");
        discovery.on_title(track(2), now + Duration::from_secs(2));
        assert!(!discovery.skip_in_flight);
        assert_eq!(discovery.skips, 0);

        discovery.on_skip_result(generation, SkipResult::Accepted, now + Duration::from_secs(3));
        assert_eq!(discovery.skips, 1);
        assert_eq!(discovery.state, DiscoveryState::Playing);
        assert!(discovery.next_skip_due.is_some());

        discovery.on_skip_result(generation, SkipResult::Accepted, now + Duration::from_secs(4));
        assert_eq!(discovery.skips, 1);
    }
}
