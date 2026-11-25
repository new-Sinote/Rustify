# Rustify Phase 4 — Performance Research

**Date:** 2026-11-25  
**Status:** Build not started. This report is the Phase 4 gate.  
**Question:** can a Rust host for the official Spotify web player meet the Phase 4 budgets on Windows, and which architecture gets closest?

> **Follow-up:** the shipping shell in `app/` implements the recommended `Low` lifecycle. Its certification run and results are in [CERTIFICATION_0.2.0.md](CERTIFICATION_0.2.0.md).

## 1. Executive summary

A WebView2 host is the only architecture tested that comes close to the Phase 4 budgets, but the candidate one-shot `Low` policy does **not** certify the playing budget. The engine choice is settled; the memory policy is not.

- The official Spotify web player runs unmodified in a bare wry + WebView2 host: free-account login persists, Widevine plays, ads play to completion, the queue advances, audio continues when the window is hidden, and SMTC reports the track. No Spotify API, page script, network response or DRM path was changed in the measured build.
- **Paused:** 60.5 MB PSS after a Normal→Low re-trim on a 40-minute warm session, and 76.5 MB on a fresh hydration. That is comfortably inside ≤300 MB. The same warm session without the re-trim sat at **401.0 MB** and missed the budget.
- **Playing:** 206.0 MB PSS immediately after a mid-playback re-trim, with the playhead advancing 2:03 → 2:46. With the candidate one-shot policy (trim once after hydration, then keep `Low`), the 35-minute soak ran **319.0 → 429.8 → 413.0 MB**, mean 376.3 MB, and **missed 400 MB at 20 and 30 minutes**. Both breaches came at ad boundaries.
- **Startup:** 298 ms with the custom two-WebView chrome, 177–401 ms across the bare/Tauri hosts; inside the 500 ms budget.
- **CPU:** 3.2–10.9% of one core while playing (mean 5.9%), 0.4–0.9% paused, 0.08% after a settled trim; the paused target is met, narrowly.
- **Edge and the desktop client are not competitive on memory:** Edge `--app` measured 732.7–754.2 MB PSS, the desktop client 672.5–773.4 MB. Both are 1.7–2.5× the embedded host and both fail the budgets.
- **The fix is lifecycle, not framework:** `Low` is a one-shot trim. Re-applying Normal→Low on pause reclaimed 387.3 → 60.5 MB; re-applying it during playback reclaimed 259.0 → 206.0 MB without an observed stall. A periodic-trim soak is the remaining gate.
- **Bench-only shell features:** the frameless Spotify-style bar and the hidden `Explore Premium` / `Install App` buttons are implemented and verified, but they are opt-in prototype flags, not part of the measured baseline. The second WebView bar costs roughly 38–60 MB PSS; V1 should prefer a native-painted caption or accept that cost explicitly.

**Verdict:** build V1 on WebView2, but treat the playing budget as **conditional**. Implement the `Low` lifecycle (hydrate, pause, periodic during playback, after ads), then run a 35-minute periodic-trim soak. If every 5-minute sample stays ≤400 MB, the budgets hold. If it does not, either revise the playing budget to ~450 MB or ship the Edge/desktop fallback. V1 is not authorized by this report alone.

## 2. Method and definitions

### 2.1 Machine and software

| Item | Value |
|---|---|
| CPU | 16 logical cores |
| Display | 2560×1440 |
| OS | Windows 11 x64 (build 26300) |
| WebView2 Runtime | 158.0.4370.33 (Evergreen) |
| Edge | 158.0.4370.33 (`C:\Program Files (x86)\Microsoft\EdgeCore\158.0.4370.33\msedge.exe`) |
| Spotify desktop | 1.3.10.2586.g81a118a22460 |
| Account | free tier, advertisements untouched |
| Network | one residential connection |

### 2.2 Metrics

- **PSS (primary):** sum of `PssBytes` from `QueryWorkingSetEx` over every process in the tree. Shared pages are divided by the number of sharers. `QueryWorkingSetEx` stores the share count in 3 bits, so pages shared by more than seven processes are over-attributed; PSS here is therefore approximate and biased upward for heavily shared runtime pages.
- **Private working set:** resident pages private to each process, from `GetProcessMemoryInfo`.
- **Private commit:** committed private bytes, from `GetProcessMemoryInfo`. This is the number that matters for the pagefile and for “will this fit on 8 GB”.
- **Total working set:** resident bytes including shared pages. It double-counts shared runtime pages and is reported only for completeness.
- **GPU memory:** `GPU Process Memory` performance counters per PID.
- **CPU:** `TotalProcessorTime` delta over the sample window, divided by elapsed wall time. Reported as a percentage of one logical core, with all-core percentage alongside.
- **JS heap / DOM:** Chrome DevTools Protocol `Runtime.getHeapUsage`, `Memory.getDOMCounters` and `Performance.getMetrics` against the page target.
- **Startup:** milliseconds from `main()` entry to the window being ready. This excludes Explorer process creation. Edge and the desktop app are not instrumented and are not compared on this metric.
- **Process tree:** the root process, every descendant, and any `msedgewebview2.exe` whose command line names the app's user-data directory. This captures the browser, GPU, renderer, utility, network and CDM processes without picking up unrelated WebView2 apps.

### 2.3 States

- **Shell blank:** the host app with a local blank page, Spotify not loaded.
- **Paused:** signed in, player loaded, playback stopped.
- **Playing:** a track playing with the window visible (unless marked hidden).
- **Hidden:** window hidden with `SW_HIDE` (the same call as tray hide), audio continuing. WebView2 may not propagate the parent's hidden state to the page; §9.2 records that caveat.
- **Soak:** 35 minutes of continuous playback, sampled every 5 minutes, with SMTC polled every 12 seconds.

### 2.4 Budgets under test

| Budget | Target |
|---|---|
| Paused PSS | ≤300 MB |
| Playing PSS | ≤400 MB |
| Idle CPU | ≤0.5% of one core |
| Paused CPU | ≤1% of one core |
| Playing CPU | minimized, benchmarked |
| Startup | ≤500 ms |

## 3. Baselines

### 3.1 Official Spotify desktop client

Signed in, 1280×800 window, full process tree.

| State | Processes | Total WS | Private WS | Private commit | PSS | GPU | CPU (% one core) |
|---|---|---|---|---|---|---|---|
| Idle after launch | 8 | 906.1 MB | 480.8 MB | 746.4 MB | 672.5 MB | 89.6 MB | 3.45% |
| Paused, 1280×800 | 7 | 968.5 MB | 559.9 MB | 764.1 MB | **754.6 MB** | 93.6 MB | 8.29%* |
| Playing | 7 | 987.5 MB | 574.9 MB | 745.8 MB | **773.4 MB** | 54.8 MB | 3.33% |

*The paused CPU sample included the resize and repaint; it is not a steady-state idle figure.

The desktop client is CEF-based for its UI, but audio is native C++, not EME/Widevine. It is a reference point and a possible fallback, not an embeddable engine: its CEF does not need a CDM for music, so its CDM setup cannot be reused.

### 3.2 Spotify Web Player in Edge

Measured in an Edge `--app=https://open.spotify.com/` window with a dedicated user-data folder, signed in, stock flags, 1296×839 client area, on the logged-in home screen. This is the app-window variant of the Edge baseline; a normal tab carries the same renderer plus a tab strip.

| State | Processes | Total WS | Private WS | Private commit | PSS | GPU | CPU (% one core) | JS heap | DOM nodes |
|---|---|---|---|---|---|---|---|---|---|
| Paused, visible | 12 | 1168.4 MB | 539.8 MB | 894.3 MB | **754.2 MB** | 202.8 MB | 3.72%* | 77.5 MB | 9704 |
| Playing, visible | 12 | 1151.2 MB | 544.5 MB | 771.4 MB | **732.7 MB** | 69.5 MB | 3.32% | 80.3 MB | 9860 |

*The paused CPU includes the first paint after the window was restored; it is not steady state.

Notes:

- **Edge is roughly desktop-client weight.** 733–754 MB PSS is in the same band as the official Spotify client (673–773 MB) and ~240 MB above the embedded WebView2 host. The Spotify renderer itself is comparable (roughly 288–314 MB PSS); the difference is Edge's own browser process (~94 MB), extra renderer (~109 MB), GPU, CDM, audio service and search indexer.
- **The first launch is not a valid sample.** A fresh Edge profile added a hidden `edge://newtab` preload and a login-flow Google reCAPTCHA iframe, inflating the tree to 1209.7 MB paused / 1257.5 MB playing with 16k–23k DOM nodes. Re-launching on the signed-in profile removed the preload (the reCAPTCHA iframe remains) and the numbers fell to the table above. Both are recorded here because the inflation is a real cost of Edge's first run, but the clean re-launch is the baseline.
- **Occlusion releases memory.** With the window behind another app (page `visibility: hidden`), the same session measured 482.4 MB PSS because the compositor and GPU allocations were released. Visible-window numbers are used for the budget, since that is how a music player is normally used.
- **Ads played untouched.** Starting playback triggered two 30-second audio ads (`Advertisement • 1 of 2`), which played to completion. After the break the page stopped at 0:00 instead of rolling into the next track and needed one click; this was observed once and is not attributed to the host.

### 3.3 Edge PWA / `--app` window

The 3.2 table is the `--app` window, which is the form an installed PWA takes on Windows: an Edge-owned window with no tab strip or address bar, running in a dedicated profile. A separately installed PWA would reuse an existing Edge browser process unless given its own profile, so its steady-state tree is the same or larger than 3.2. No separate PWA install was measured; the app-window result is the lower bound for this architecture.

### 3.4 Phase 3 Tauri spike

Tauri 2.12.7, remote web player, no capabilities, navigation allowlist, tray and SMTC.

| State | Processes | Total WS | Private WS | Private commit | PSS | CPU (% one core) |
|---|---|---|---|---|---|---|
| Paused visible | 10 | 1102.7 MB | 426.1 MB | 701.8 MB | **558.0 MB** | 0.00% |
| Playing visible, 30 s | 10 | 1105.8 MB | 438.7 MB | 595.1 MB | **594.8 MB** | 8.35% |
| Playing hidden, 60 s | 10 | — | 446.6 MB | 613.8 MB | **603.0 MB** | 4.48% |
| Hidden paused, 5 min | 10 | — | 343.9 MB | 787.8 MB | **535.9 MB** | 0.00% |

### 3.5 Bare wry + WebView2 shell

wry 0.57.4 + tao 0.37.3, no Tauri. The table covers the local blank shell and the remote Spotify page in the same host.

| State | Processes | Total WS | Private WS | Private commit | PSS | GPU | CPU (% one core) | Window ready |
|---|---|---|---|---|---|---|---|---|
| Blank shell | 8 | 346.4 MB | 65.7 MB | 143.7 MB | **121.4 MB** | 26.1 MB | 1.00% | 282 ms |
| Spotify paused | 10 | 812.4 MB | 364.6 MB | 526.2 MB | **488.5 MB** | 80.4 MB | 0.00% | 177–185 ms |
| Spotify playing visible | 10 | 822.2 MB | 373.8 MB | 513.2 MB | **497.7 MB** | 60.4 MB | 3.75% | — |

The bare shell reproduces the whole Spotify experience without Tauri: login persists, Widevine works, audio plays, and background playback continues. Reusing the spike's WebView2 user-data folder preserved the session once the data directory was corrected to the **parent** of `EBWebView` (WebView2 appends `EBWebView` itself).

Two bench-only variants were also built and verified: `--hide-upsells` (one CSS rule that hides the nav-bar upgrade button and download link) and `--custom-chrome` (frameless window plus a 40 px WebView bar). They are not part of the measured baseline; results and costs are in §8 and Appendix B.

### 3.6 Minimal Tauri shell

Minimal Tauri 2.12.7 app, local page, no commands.

| State | Processes | Total WS | Private WS | Private commit | PSS | GPU | CPU |
|---|---|---|---|---|---|---|---|
| Blank shell | 7 | 345.3 MB | 66.5 MB | 156.1 MB | **117.7 MB** | 36.3 MB | 0.00% |

### 3.7 Early Edge app-window probe

A first probe with `--app=about:blank` opened a New Tab and was still loading:

| State | Processes | Total WS | Private WS | Private commit | PSS | GPU | CPU |
|---|---|---|---|---|---|---|---|
| New Tab, 20 s | 9 | 682.0 MB | 312.2 MB | 479.4 MB | 390.0 MB | 108.9 MB | 14.68% |

This is **not** a valid Edge baseline; it is recorded only to show that the measurement harness can isolate a dedicated Edge profile tree. The real baseline uses `--app=https://open.spotify.com/`.

## 4. Shell comparison: wry vs Tauri (both on WebView2)

The shells differ by a few megabytes, not by hundreds:

- Bare wry + WebView2: 121.4 MB PSS blank.
- Minimal Tauri + WebView2: 117.7 MB PSS blank.
- Phase 3 Tauri spike with Spotify paused: 558.0 MB PSS.
- Same Spotify page in bare wry: 488.5 MB PSS paused, 497.7 MB playing.
- Same page in an Edge app window: 754.2 MB paused, 732.7 MB playing.

The 60–70 MB difference between the spike and the bare host is page/profile state and sample timing, not framework overhead: the spike had a fresh profile and its page had fully hydrated, while the wry run reused the warmed profile. The blank-shell numbers prove the point: Tauri and wry cost essentially the same once WebView2 is running.

**Conclusion:** choosing a lighter Rust framework cannot solve the memory problem. Tauri's value is tray, updater, single-instance and packaging, not memory. wry's value is a smaller dependency surface and direct control of the controller, environment and memory API.

## 5. WebView2 memory APIs and flags

### 5.1 `MemoryUsageTargetLevel.Low`

`CoreWebView2MemoryUsageTargetLevel` has `Normal` and `Low`. `Low` is a **best-effort** request: scripts keep running, the view does not need to be hidden, and the browser process may swap memory to disk. Microsoft warns that responsiveness can degrade. The level is sticky until set back to `Normal`, but its trimming effect is one-shot. It is documented as mutually exclusive with `TrySuspendAsync`.

Measured on the Spotify page with wry 0.57.4:

| State | PSS | Private WS | Private commit | GPU | CPU | Playback |
|---|---|---|---|---|---|---|
| Paused, Low set at launch (16 s, still hydrating) | 229.1 MB | 208.3 MB | 622.7 MB | 138.5 MB | 2.92%* | paused |
| Paused, Low re-applied after a Normal cycle | **34.9 MB** | 20.3 MB | 483.5 MB | 45.6 MB | 1.51%* | paused |
| Paused, Low re-applied again | **52.8 MB** | 43.9 MB | 534.6 MB | 79.4 MB | 0.80%* | paused |
| Normal restored, paused | 125.8 MB | 96.8 MB | 512.8 MB | 75.9 MB | 0.50% | paused |
| Playing, still Low | **225.5 MB** | 187.0 MB | 514.3 MB | 63.5 MB | 1.16% | advanced 3:50 → 4:21 |
| Playing, Normal after a Low cycle | 243.5 MB | 203.9 MB | 516.5 MB | 63.0 MB | 2.75% | advanced 2:37 → 3:09 |
| Warm paused after 35 min, before re-trim | 387.3 MB | 297.0 MB | 618.4 MB | 58.3 MB | 0.40% | paused |
| Warm paused, `Low` re-applied | **60.5 MB** | 46.8 MB | 613.7 MB | 57.7 MB | 0.92%* | paused, heap/DOM intact |
| Warm playing, before re-trim | 259.0 MB | 214.9 MB | 654.9 MB | 87.1 MB | 3.84% | playing at 1:23 |
| Warm playing, `Low` re-applied | **206.0 MB** | 181.6 MB | 618.0 MB | 56.6 MB | 5.49% | advanced 2:03 → 2:46 |

*The 0.92% paused sample includes the 30-second window after the trim; it is still under the 1% target. The earlier paused CPU figures include repaint, hydration or sampling after a trim and are not steady state; `Low` at launch is also much less aggressive than a trim on a hydrated page (229 MB versus 35–53 MB). The warm rows come from the same 40-minute session, not a fresh profile; a 60-second pause immediately after the soak measured 401.0 MB before this settled 387.3 MB pre-trim state (see §9.2).

In practice, `Low` behaves like a one-shot trim request, not a continuous cap: setting it before the page loads did not prevent the soak from starting at 465.8 MB after a playlist navigation, while re-applying it to a settled paused page produced 35–53 MB. A production host should therefore apply `Low` at defined lifecycle points — after the page has hydrated, after a pause has been confirmed, and periodically during playback — and measure after each application. A single mid-playback re-application was measured clean (259.0 → 206.0 MB, playhead 2:03 → 2:46), but frequent re-application has not been soaked and may cause paging churn.

Two findings changed the Phase 3 conclusion:

1. **Low does not break playback.** The earlier “spinner” was a click on the seek bar, not a failed play. With Low active, the playhead advanced for 31 seconds and audio continued.
2. **Low is not a free win.** It cuts resident memory by paging aggressively; commit stays 480–620 MB, and the page can fault back in quickly. The 35-minute soak showed that a one-shot trim does not hold the playing budget (429.8 MB at 20 minutes); periodic re-trims are required.

Policy implications:

- Apply `Low` after the page has hydrated, again after a confirmed pause, and on a conservative timer during playback. Do not restore `Normal` for playback unless the periodic soak shows an audio or interaction cost; one mid-playback trim was clean, but the safe cadence is unresolved.
- Re-apply `Low` after an ad break. In soak B both 400 MB breaches came at 30-second ad boundaries, and the trim is what resets the growth.
- Use WebView2's `IsDocumentPlayingAudioChanged` event, not a UI click, to drive the transition. The event fires when the document starts or stops producing audio. Add a short debounce (for example 20–30 seconds of confirmed silence) before trimming, so track gaps do not trigger a trim.
- Never call `TrySuspendAsync` while audio is expected: suspension is blocked by playing audio, and forcing it would stop the page's timers and websocket keepalive.
- Treat `Low` as a Windows-memory-pressure feature, not as a way to hit a number: commit and pagefile use remain high, and first interaction after a long pause may be slower.

### 5.2 `TrySuspendAsync`

Suspension requires `IsVisible = false`, pauses script timers and animations, and lets the OS reclaim renderer memory. It is the wrong tool for a music player: playing audio blocks suspension, and the page's state machine, websocket and playback position would stop. It could only be used when paused **and** hidden, but `Low` already covers that state without stopping scripts. Not recommended.

### 5.3 Process model and shared environments

- One browser process per user-data folder. Every WebView2 app pointed at the same folder joins that browser process and shares GPU/utility/network processes.
- Renderers are still created per site/frame by Chromium's process model; sharing a folder does not collapse the Spotify renderer.
- A shared folder is **not** a security boundary: cookies, cache and storage are shared.
- Two separate Rustify instances with the same folder would share the browser process only if their environment options match. That is a possible future single-instance strategy, but it couples crash domains and is not needed for V1.
- Fixed Version WebView2 adds >250 MB to the package; Evergreen reuses the system runtime. Evergreen is the right choice for a small installer, at the cost of depending on the installed runtime.

### 5.4 Chromium flags

WebView2 forwards `AdditionalBrowserArguments` to the browser process; flags apply to every WebView in that user-data folder and are explicitly **not for production** (Microsoft may remove or change them). Candidate flags and their expected effect:

| Flag | Expected effect | Risk |
|---|---|---|
| `--enable-low-end-device-mode` | Forces low-end heuristics; may reduce cache sizes and renderer limits | Version-dependent, undocumented in WebView2 |
| `--renderer-process-limit=1` | Collapses renderer processes | Site isolation weakened; Spotify may still need its own renderer |
| `--disable-gpu` | Removes GPU process/VRAM cost | Microsoft says troubleshooting only; breaks compositing performance |
| `--js-flags=--max-old-space-size=N` | Caps V8 old space | Page may crash or GC-thrash; not a WebView2 contract |

These were not needed for the first pass, and the periodic-trim policy is preferred over undocumented flags. If the periodic soak still misses, they are a last-resort experiment, not a shipping plan, because shipping them would violate the “supported configuration” principle of the design.

## 6. Alternative architectures

### 6.1 Edge PWA or `--app` window

- Runs in Edge's own process tree and profile; no separate runtime, no CDM work. Widevine comes from Edge, updates come from Edge.
- An installed PWA is a normal app window in the existing Edge browser process; it does not get a separate process group unless a separate profile is used.
- `--app="URL" --user-data-dir="..."` gives a dedicated profile and a measurable tree, but flags only apply when a new browser process starts.
- Costs: no tray integration or shell control; coupled to Edge's version, profile and update policy; the app window is a browser window with browser chrome removed, not a product shell. A shared profile is not a security boundary.
- Still the lowest-effort fallback if a Rust host fails the budgets.

### 6.2 CEF

- Widevine is enabled by default since M93 and fetched by the component updater, but CEF builds need `enable_widevine`, `enable_cdm_host_verification`, `enable_cdm_storage_id` and `enable_rlz`; Windows ARM64 is unsupported.
- VMP/host verification is the sharp edge: persistent-license and VMP failures are tracked in CEF issues #3404 and #3952; Windows VMP with bootstrap was broken until M139.
- Size: a M107 x86 release is ~169 MB without PGO (~181 MB with), with `libcef.dll` alone ~156–166 MB. A Spotify-sized CEF app is 150+ MB before content.
- Maintenance: branches track every other Chromium milestone (~4 weeks), and Chrome moves to a two-week major cycle in September 2026. A milestone update is roughly a week of work, mostly renames and reorgs.
- Technically the most control, and the highest permanent cost. Not justified for a shell whose only job is to display one origin.

### 6.3 Electron

- Stock Electron does not ship a usable Widevine: docs call it experimental, the app must supply the CDM, and VMP signing may be required. PR #50046 (merged 2026-10-15 for 46-x-y) adds CDM support, but the CDM files and production VMP signing remain the app's responsibility.
- castLabs Electron for Content Security (ECS) is the production path: runtime CDM install/update and VMP, Windows/macOS. It requires EVS signing for production.
- Size: Electron apps are typically >100 MB installed and 100+ MB zipped; one 2026 third-party measurement reports 106 MB installer / 367 MB on disk, unverified.
- Not competitive with WebView2 for a single-origin shell: larger installer, duplicate Chromium, CDM/VMP ownership.

### 6.4 Official desktop client

- Its CEF draws the UI only; audio is native. It is not a reusable Widevine host and cannot be embedded.
- Measured 672–773 MB PSS, heavier than the Phase 3 spike. It is a fallback product, not an architecture.

### 6.5 Other options

- Legacy MSHTML/Trident: no EME. Ruled out.
- Qt WebEngine / NW.js: Chromium shells with the same CDM/VMP work as CEF, not evaluated.
- WebView2 Fixed Version: larger package, manual updates, reported to lack PlayReady; no reason to choose it over Evergreen here.

### 6.6 Widevine in WebView2: what is actually supported

Microsoft has **no official statement** that Widevine is supported in WebView2. Widevine's own overview lists Edge and CEF/Electron, not WebView2. The feature request for official support (#4828) has been open since September 2024. Against that, the Evergreen runtime ships a `WidevineCdm` component, Microsoft engineers have said WebView2 supports Widevine out of the box because it ships with Edge, and this machine's Phase 3 probe returned `com.widevine.alpha` success and played DRM audio for hours.

The honest position: **it works on the tested runtime and is not a contractual guarantee.** V1 must probe EME at startup and fail with a clear message if the runtime lacks Widevine, rather than assume it. Fixed Version runtimes should not be used without retesting EME.

## 7. Spotify page bottlenecks

- The Spotify renderer alone is **286–316 MB PSS** in normal mode; browser, GPU, utilities and shell add roughly 200–280 MB. No shell change can remove the renderer floor.
- JS heap is 69–76 MB, with ~8.8–9k DOM nodes and ~4.5–4.7k JS event listeners. This is a large, long-lived single-page app, not a thin audio element.
- The page keeps allocating after playback starts. In the first six minutes after a `Low` trim, PSS rose from 225.5 MB to 395.3 MB while playing; soak A then flattened at 427-436 MB for 25 minutes. Soak B, trimmed on a hydrated paused page, started lower (319.0 MB) but still climbed to 429.8 MB at 20 minutes and 413.0 MB at 30 minutes, both at ad boundaries. The growth is cache refill, not an unbounded leak, but a one-shot trim does not keep the plateau under 400 MB; re-trimming resets it to 60.5 MB paused and 206.0 MB playing.
- Public reports of the web player growing to 1.7 GB after two songs (2016), ~6 GB before OOM in 1–2 hours (2024), and >2 GB with Brave Shields are unverified but consistent in direction: the page has a history of unbounded growth in some configurations. Our own soak is the only evidence used for the verdict.
- Ads were audio-only in every observed break (one in soak A, two in soak B, two in Edge) and are served by the page itself. Blocking none of them is both a product requirement and the simplest path: no filters, no proxies, no DNS changes.
- Background playback works when minimized and hidden; the page does not pause on visibility change. Chromium throttles rendering for occluded windows (CPU falls), but audio and the Connect session continue.

## 8. Rust shell overhead

- Blank wry shell: 121.4 MB PSS, of which the Rust process is ~5–6 MB; the rest is WebView2's browser/GPU/renderer/utility floor.
- Minimal Tauri shell: 117.7 MB PSS, same floor.
- Window creation: 177–282 ms for the wry host, 401 ms for the Phase 3 Tauri spike. Both are inside the 500 ms budget with margin.
- Installer: 1.08 MiB for the Phase 3 Tauri NSIS installer; the bare wry host is a 0.6 MB single exe plus the system runtime (the Phase 3 Tauri exe is 3.05 MiB).
- The Rust shell's own CPU is effectively zero; the 0.5% idle budget is consumed by WebView2 and the page, not by Rust.
- Bench-only custom chrome: a 40 px local WebView title bar in a frameless host. Window ready was **298 ms** with two webviews; the tree stayed at 10 processes but gained a renderer. The bar renderer measured **38.5 MB PSS**, and the paused tree measured 563.8 MB (private 407.4 MB, commit 598.6 MB, GPU 105.8 MB) at 2.35% CPU. Maximize, restore, minimize, drag and close all worked; closing the bar exited the host and left no profile children. A native caption would avoid the 38–60 MB cost.

**Conclusion:** the shell is not the problem and cannot be the solution. Optimization effort belongs in WebView2 state policy (`Low`/`Normal`, visibility, process sharing), not in rewriting the host in a different framework.

## 9. Stability soak and ad observation

Two 35-minute soaks were run on the bare wry host, both playing a Spotify-owned playlist continuously with the window visible, SMTC polled every 12 seconds and metrics sampled every 5 minutes.

### 9.1 Soak A — `Low` set before navigation

The host was launched with `MemoryUsageTargetLevel.Low` already set, then brought to a playlist and played. This is the weakest way to apply `Low`: the level is set before the page has allocated, so there is little to trim.

| Minute | PSS | Private WS | Private commit | GPU | CPU (% one core) | JS heap | DOM nodes |
|---|---|---|---|---|---|---|---|
| 0 | 465.8 MB | 366.1 MB | 599.0 MB | 61.1 MB | 5.13% | 87.5 MB | 5089 |
| 5 | 454.6 MB | 343.4 MB | 577.5 MB | 58.4 MB | 4.86% | 60.1 MB | 4987 |
| 10 | 426.8 MB | 316.8 MB | 588.9 MB | 59.5 MB | 5.90% | 61.3 MB | 5024 |
| 15 | 430.8 MB | 315.2 MB | 580.6 MB | 59.5 MB | 5.89% | 61.0 MB | 4962 |
| 20 | 433.7 MB | 322.6 MB | 584.5 MB | 58.5 MB | 4.34% | 62.1 MB | 4966 |
| 25 | 428.1 MB | 315.3 MB | 589.9 MB | 58.5 MB | 6.48% | 61.8 MB | 5083 |
| 30 | 435.5 MB | 324.5 MB | 592.0 MB | 59.6 MB | 2.79% | 63.3 MB | 4942 |

- **It plateaus, it does not leak.** PSS fell from 465.8 to 426.8 MB over the first 10 minutes (the page releasing hydration allocations) and then stayed in a 427–436 MB band for the remaining 25 minutes. JS heap was flat at 60–63 MB and DOM at ~5k nodes.
- **It misses the playing budget by ~8–9%.** The plateau is 427–436 MB against a 400 MB target.
- **Private commit is stable at 578–599 MB.** The page is not hiding memory in the pagefile during playback; the resident number is real.
- **An advertisement played normally.** At 31.2–32.0 minutes SMTC reported `Advertisement / Playing` for a ~30-second spot (five 12-second polls, `end_s = 30.1`), then playback resumed into the queue at 32.2 minutes. Nothing blocked, skipped or muted it.
- **Playback was continuous.** 15 tracks over 35 minutes; the only gap was a single 12-second `NoSession` poll at a track boundary (22.6 min), which is an SMTC reporting gap, not an audio stop.
- **CPU averaged 5.2% of one core** (2.8–6.5% band) while visible and playing.

### 9.2 Soak B — `Low` applied to a hydrated, paused page, retained during playback

Same host and page, but `Normal` at launch, then a settled `Low` trim while paused, then playback resumed with `Low` retained. This is the candidate production policy and the run that decides the playing budget. The run started at 19:06 UTC on host PID 4772, window visible, and lasted 35 minutes.

Before the trim, paused `Normal`: **545.4 MB PSS**, 413.1 MB private WS, 613.5 MB commit, 2.53% CPU (still hydrating). After a 45-second settled `Low`: **76.5 MB PSS**, 61.4 MB private WS, 502.7 MB commit, 0.08% CPU. The renderer dropped from 286.1 to 41.1 MB PSS. A first attempt at the same sequence measured 550.3 MB before the trim and 77.8 MB after, so the result repeats within ~1–2 MB.

| Minute | PSS | Private WS | Private commit | GPU | CPU (% one core) | JS heap | DOM nodes |
|---|---|---|---|---|---|---|---|
| 0 | 319.0 MB | 252.1 MB | 572.7 MB | 72.8 MB | 4.92% | 69.0 MB | 4723 |
| 5 | 347.4 MB | 272.3 MB | 556.8 MB | 54.3 MB | 7.53% | 58.1 MB | 4762 |
| 10 | 346.6 MB | 271.6 MB | 567.5 MB | 55.6 MB | 3.84% | 62.9 MB | 4900 |
| 15 | 387.3 MB | 305.2 MB | 606.9 MB | 61.6 MB | 10.91% | 73.3 MB | 4834 |
| 20 | **429.8 MB** | 336.0 MB | 632.7 MB | 60.5 MB | 3.24% | 66.2 MB | 4761 |
| 25 | 390.9 MB | 301.6 MB | 617.0 MB | 57.5 MB | 5.38% | 64.6 MB | 4717 |
| 30 | **413.0 MB** | 322.1 MB | 636.3 MB | 60.5 MB | 5.30% | 66.5 MB | 4859 |

- **It does not hold the playing budget.** Mean 376.3 MB, median 390.9 MB, min 319.0 MB, max 429.8 MB. Two of seven samples exceeded 400 MB, at 20 and 30 minutes. The mean is inside 400; the requirement is not a mean.
- **Both breaches came at ad boundaries.** The 20-minute sample followed the first 30.1-second ad (14.0-14.8 minutes); the 30-minute sample was taken during the second (30.0-30.8 minutes). Playback resumed after each. Memory rose through the ad and was still elevated at the next sample.
- **Playback was continuous.** 168 SMTC polls, all `Playing`, 11 tracks, no `NoSession` gap. CPU averaged 5.87% (3.24–10.91%); JS heap stayed at 58–73 MB and DOM at 4.7–4.9k nodes. The tree stayed at 10 processes.
- **The post-soak paused state fails without a re-trim.** Visible playing measured 413.2–419.3 MB; paused after the run with `Low` still set but no new trim measured **401.0 MB** at 0.91% CPU, and resume worked. The level was still active, but new allocations were not reclaimed.
- **A pause re-trim fixes it.** On the same warm session, Normal→Low on the paused page moved **387.3 → 60.5 MB PSS**, with commit 613.7 MB, heap 67.2 MB, DOM 4781 and CPU 0.92%. Resume advanced the track from 1:05 to 1:17.
- **A playing re-trim also worked in one sample.** **259.0 → 206.0 MB PSS**, commit 654.9 → 618.0 MB, CPU 5.49%, playhead 2:03 → 2:46. No stall or SMTC gap was observed.

| Post-soak experiment | Before | After | Commit after | CPU | Playback |
|---|---|---|---|---|---|
| Warm paused, Normal→Low | 387.3 MB | **60.5 MB** | 613.7 MB | 0.92% | resumed at 1:17 |
| Warm playing, Normal→Low | 259.0 MB | **206.0 MB** | 618.0 MB | 5.49% | advanced 2:03 → 2:46 |
| Paused after soak, no re-trim | — | 401.0 MB | 688.3 MB | 0.91% | resumed |
| Hidden via `SW_HIDE`, playing | — | 417.9 MB | 628.3 MB | 5.35% | continued; page still reported visible |

**Hidden caveat:** `SW_HIDE` hid the top-level window, but the page's `document.visibilityState` stayed `visible` because WebView2 did not propagate the parent's hidden state. The 417.9 MB sample is therefore effectively another visible-window sample, not evidence that hidden playback is cheaper. If hidden-state trimming matters, drive `ICoreWebView2Controller::IsVisible` (or suspend only when paused and hidden) instead of hiding the parent HWND.

**Soak B verdict:** the one-shot candidate policy fails. The page is not leaking, but it refills cache after trims and after ads, and `Low` must be re-applied to keep it down. The 35-minute periodic-trim soak is the remaining gate before V1.

## 10. Ranked recommendation

The ranking is by measured PSS, CPU and startup. The first two entries are the same Chromium engine in different packaging; the V1 choice between them is about tray/updater/installer, not memory.

### 10.1 Measured ranking

| Rank | Architecture | Paused PSS | Playing PSS | Playing CPU | Startup | Processes | Verdict |
|---|---|---|---|---|---|---|---|
| 1 | **wry + WebView2, lifecycle `Low`** | **60.5 MB** after re-trim; 76.5 fresh | **206 MB** after re-trim; 319–430 one-shot soak | 3.2–10.9% | 177–298 ms | 10 | Paused passes; playing conditional on periodic trim |
| 2 | **Tauri 2 + WebView2, same policy** | 558 MB without policy (Phase 3) | 595 MB without policy (Phase 3) | 8.35% playing / 4.48% hidden | 401 ms | 10 | Same engine; port the policy, then certify |
| 3 | **Edge `--app` window** | 754.2 MB | 732.7 MB | 3.3–3.7% | not instrumented | 12 | Fails paused and playing; fallback only |
| 4 | **Official Spotify desktop client** | 754.6 MB | 773.4 MB | 3.33% playing | not instrumented | 7 | Fails paused and playing; not embeddable |
| 5 | **CEF / Electron** | not measured | not measured | not measured | not measured | not measured | Installer, CDM/VMP and milestone cost not justified for one origin |

### 10.2 Recommendation

**Build V1 on Tauri 2 + WebView2, with the memory policy proven in the wry host, once the gate below passes.** Tauri adds the installer, tray, updater, single-instance and packaging that a desktop music player needs; the measurements show the framework itself is not the memory cost. Port the wry policy into the Tauri shell and re-measure, rather than assuming the numbers transfer. The wry host wins the measured table because it has no packaging layer; Tauri's packaging is not a memory cost, and V1 needs it.

The policy is:

1. Apply `Low` after the page has hydrated and the library has settled.
2. On a confirmed pause (20–30 seconds of silence, driven by `IsDocumentPlayingAudioChanged`), cycle Normal→Low and leave it there. Measured: 387.3 → 60.5 MB.
3. During playback, re-apply Normal→Low every 5–10 minutes and after every ad break. Measured once: 259.0 → 206.0 MB with the playhead advancing. The cadence is not yet certified.
4. Never `TrySuspendAsync` while audio is expected; never suspend the page just because the window is hidden.
5. Keep the window visible baseline for budget tests; `SW_HIDE` did not change Chromium's visibility state in this build.

**Do not add a second WebView for the title bar in V1.** The custom bar works and looks right, but it adds a renderer (~38.5 MB PSS) and browser/utility overhead; the measured tree was 563.8 MB with 10 processes. Use a native-painted caption (GDI/Direct2D) or a DWM-themed native frame, both of which cost no extra renderer. If the exact Spotify bar is a hard requirement, budget 40–60 MB and re-run the soak with the bar present.

**Keep the measured build unmodified.** Ads must play untouched. If the `Explore Premium` / `Install App` hide ships, it must be a separate, opt-in cosmetic setting, off by default, with fail-safe selectors; it is page injection and needs an explicit allowlist revision. It has no measurable performance effect.

**Gate before V1 feature work:** a 35-minute periodic-trim soak in which every 5-minute sample is ≤400 MB PSS, playback survives ads and track changes, and a 10-minute hidden/paused run stays ≤300 MB. If that fails, the honest options are to revise the playing budget to ~450 MB or ship the Edge/desktop fallback; do not spend V1 effort pretending the number is met.

### 10.3 Open questions

Resolved by this phase:

- **Does the `Low`-trimmed page stay under 400 MB for 35 minutes?** Not with a one-shot trim. It breached at 20 and 30 minutes, both at ad boundaries. It does stay under after a re-trim in short samples.
- **Does Edge beat the embedded host?** No. Edge is 733–754 MB PSS versus 319–430 MB for the host, and carries 12 processes.
- **Does the first interaction after a long pause still work?** Yes. Resume from the 60.5 MB paused state advanced the track; subjective input latency was not measured.

Still open:

- Does periodic Normal→Low during playback hold ≤400 MB for 35 minutes without audible glitches or paging stalls?
- Does a native-painted caption meet the visual bar without a second renderer?
- Is 400 MB the right playing budget for a 10-process Chromium app on an 8 GB machine, given 600–690 MB private commit?
- Does the policy behave the same on Intel iGPU and 125–150% DPI, where GPU memory and composition costs differ?

## Appendix A — Rejected features: Spicetify and ad blocking

These were evaluated at the user's request and are **not supported** in V1. The Phase 0–2 allowlist and Phase 4 acceptance criteria require the official web player, intact advertisements, and no page injection, extensions, proxies or modified clients. The opt-in upsell-hiding experiment in Appendix B is outside this supported scope and is documented separately.

### A.1 Spicetify

Spicetify is a CLI that customizes the **official desktop client** by patching its installed `xpui.spa` bundle on Windows, macOS and Linux, and it must be re-applied after each Spotify update ([getting started](https://spicetify.app/docs/getting-started), [FAQ](https://spicetify.app/docs/faq)). Rustify hosts `https://open.spotify.com` in WebView2; there is no `xpui.spa` and no desktop-client UI to patch.

“Support” would therefore require one of:

- switching Rustify to launch or patch the desktop client instead of hosting the web player — a different product, with the measured 673–773 MB PSS baseline and recurring update breakage; or
- reimplementing Spicetify-style injection against the web player — page injection, which is explicitly prohibited.

Spicetify's Marketplace also offers the `adblockify` extension, which is ad blocking and falls under the same prohibition.

### A.2 Ad blocking

WebView2 has no supported ad-block API. The technical routes are:

| Route | Why it is rejected |
|---|---|
| `WebResourceRequested` host filtering | Ads share Spotify CDN and playback infrastructure with tracks; filtering is brittle, detectable, and can break audio or DRM. It is also a product-level ad blocker. |
| Script/CSS injection | Hides ad UI but does not stop audio ads; injection is prohibited. |
| Edge extension (uBlock Origin) | Not available to the WebView2 shell; extensions are prohibited. |
| DNS/proxy/MITM filtering | Machine-wide, breaks other applications, weakens security; prohibited. |

The free tier is ad-supported. Spotify's terms restrict modifying the client and circumventing restrictions, and modified or blocking setups carry account risk. Ad blocking also does not help the memory budgets: an extension or filter list is a few MB, while the page and Chromium dominate.

**Decision:** ads play untouched, as specified. If the product requirements are ever changed to allow ad blocking or client modification, that work must be scoped as a separate experiment outside Rustify, with the ToS/account and breakage risks explicitly accepted; it is not part of V1.

## Appendix B — Bench-only shell experiments

These are opt-in flags in `bench/wryhost`, not V1 defaults, and they were **not** active during the performance soaks.

### B.1 Upsell hiding

`--hide-upsells` injects one `<style>` element at document load, main frame only:

```css
[data-testid="global-nav-bar"] button[data-testid="upgrade-button"],
[data-testid="global-nav-bar"] a[href="/download"] { display: none !important; }
```

Verified on the live page: both elements computed `display: none` at 0×0, one style tag, no observers, no network or behavior changes. The selectors use Spotify's `data-testid`/`href` hooks and fail safe: if the hooks change, the buttons simply stay visible. This is still page injection and is outside the Phase 0–2 allowlist; it is off by default and did not affect any measurement. Shipping it requires a separate scope/allowlist decision.

### B.2 Custom chrome

Frameless window (`decorations(false)`) with a 40 px child WebView bar, local HTML/CSS/SVG, and IPC commands `drag`, `minimize`, `toggle-max`, `close`.

- Verified: maximize (window zoomed), restore, minimize, drag (+120/+72 px), and close (host exits, zero profile children left behind).
- Two WebViews sharing a user-data folder must use identical `CoreWebView2EnvironmentOptions`. The bar first failed with `HRESULT 0x8007139F` until it received the same additional browser arguments as the main view. This is a wry/WebView2 requirement, not a Rustify bug.
- Cost: the bar's renderer measured 38.5 MB PSS; the paused tree measured 563.8 MB. A native GDI/Direct2D caption or DWM-themed native caption avoids the second renderer; V1 should use that unless the exact Spotify bar is a hard requirement.
- Double-click maximize is unreliable in the HTML bar because the first mousedown starts the native drag loop. The maximize button works; native double-click needs `WM_NCLBUTTONDBLCLK` on a native caption.

## 11. Sources

- WebView2 `MemoryUsageTargetLevel`: https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2.memoryusagetargetlevel
- WebView2 performance best practices: https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance
- WebView2 process model: https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-model
- WebView2 user data folder: https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/user-data-folder
- WebView2 environment options and same-folder constraint: https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2environment.createcorewebview2controllerasync
- WebView2 distribution: https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution
- WebView2 feature flags: https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/webview-features-flags
- WebView2 Widevine request #4828: https://github.com/MicrosoftEdge/WebView2Feedback/issues/4828
- WebView2 Widevine inconsistency #2021: https://github.com/MicrosoftEdge/WebView2Feedback/issues/2021
- CEF Widevine announcement: https://groups.google.com/g/cef-announce/c/wlcge0kBMC0
- CEF branches and release cadence: https://chromiumembedded.github.io/cef/branches_and_building.html
- CEF binary size #2956: https://github.com/chromiumembedded/cef/issues/2956
- Electron Widevine testing: https://github.com/electron/electron/blob/main/docs/tutorial/testing-widevine-cdm.md
- castLabs ECS: https://github.com/castlabs/electron-releases/
- Chromium memory backgrounder: https://www.chromium.org/developers/memory-usage-backgrounder/
- Chromium renderer process limit: https://chromium.googlesource.com/chromium/src/+/main/content/public/common/content_switches.cc
- Edge PWA UX: https://learn.microsoft.com/en-us/microsoft-edge/progressive-web-apps/ux
- Spotify engineering on the desktop client: https://engineering.atspotify.com/2021/04/building-the-future-of-our-desktop-apps
