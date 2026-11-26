# Rustify Performance Research

**Research window:** 2026-11-25  
**Final certification:** 2026-11-26  
**Status:** Complete. The shipping 0.2.0 shell passed every release gate on the tested Windows 11 x64 machine.  
**Question:** can a Rust host for the official Spotify Web Player meet the memory, CPU, and startup budgets on Windows — and which architecture and lifecycle get there?

> **Final answer:** yes. Use WebView2, leave Spotify's player in charge, and drive `MemoryUsageTargetLevel.Low` as a lifecycle rather than a one-shot setting. The shipping `wry` + `tao` shell measured **95.1 MB peak paused**, **263.3 MB peak playing**, **272 ms window-ready**, and **0.24–0.39% sustained paused CPU**. The full gate report is [CERTIFICATION_0.2.0.md](CERTIFICATION_0.2.0.md).

> **Historical baseline - superseded product requirement.** This document records the 0.2.0 measurements and the old ads-untouched decision. On 2026-11-26 the owner made adblocking a first-class requirement. The 0.2.0 numbers remain the baseline for 0.2.0, but they do not certify any adblocking build. See [ADR-0001](ADR-0001-adblocking-direction.md) and [ADBLOCKING_RESEARCH.md](ADBLOCKING_RESEARCH.md).

> **Update for v0.3.0-adblock-pre.1 (2026-11-26):** the adblocking pre-release adds a native mute fallback and an opt-in discovery mode. Its synthetic process-tree measurements are in [Appendix C](#appendix-c---adblocking-branch-smoke-test). They are smoke numbers on a local page, not real Spotify playback, and they do not extend the 0.2.0 certification.

## 1. Executive summary

The engine question was settled early: a WebView2 host is the only tested architecture that can run the official Spotify Web Player with Widevine, background playback, and untouched ads while staying near the memory budgets. The difficult part was not the Rust framework. It was the page's tendency to refill cache after every trim.

The Phase 4 one-shot policy failed the playing soak: it trimmed once after hydration, then let the page climb back to 429.8 MB at 20 minutes and 413.0 MB at 30 minutes. Both breaches landed at ad boundaries. The fix was lifecycle, not a different framework:

- Apply `Low` after the page has hydrated.
- Cycle `Normal -> Low` after a confirmed pause.
- Re-apply it every 120 seconds during playback, including while paused and hidden.
- Re-apply it shortly after audio resumes at track and ad boundaries.

That policy, shipped in `app/`, passed the gate with substantial margin.

### 1.1 Final certified results

| Gate | Target | Rustify 0.2.0 | Result |
|---|---:|---|---|
| Paused PSS, hidden 10-minute run | ≤ 300 MB | **95.1 MB max**, 60.0 MB mean | PASS |
| Playing PSS, every 5-minute soak sample | ≤ 400 MB | **263.3 MB max**, 226.9 MB mean | PASS |
| Paused CPU | ≤ 1% of one core | 0.32% mean; **0.24–0.39% sustained** | PASS |
| Idle CPU | ≤ 0.5% of one core | **0.24–0.39% sustained** | PASS |
| Window ready | ≤ 500 ms | **272 ms warm; 271 ms fresh profile** | PASS |
| Installer | per-user, no admin | 534,512 bytes, silent install/uninstall verified | PASS |

One 10-second CPU sample in the paused run read 1.05%. Two 60-second steady-state windows measured afterward read 0.24% and 0.39%; the transient is recorded in the certification report rather than hidden. A first launch on a WebView2 runtime that had never been initialized once measured 652 ms; normal starts do not reproduce it.

### 1.2 What changed from the Phase 4 draft

- **Architecture:** the shipping shell is `wry` + `tao`, not Tauri. The shell features Tauri would have supplied — tray, single-instance activation, window state, installer — are implemented directly, which keeps the dependency surface small and gives direct access to the WebView2 controller and memory API.
- **Policy:** the one-shot `Low` experiment was replaced by the periodic lifecycle above. The 35-minute gate now passes at 263.3 MB peak.
- **Chrome:** the exact Spotify-style 40 px caption bar was kept as a product requirement. Its second WebView adds a renderer and roughly 38.5 MB PSS before trims; the certified totals include it, so the cost is inside the budget rather than assumed away.
- **Upsells:** the shipping build hides the two nav-bar upsells with one scoped stylesheet. It remains the only page change and is disclosed as a cosmetic rule, not an ad-blocking feature.
- **Widevine:** the player works on the tested Evergreen runtime. The shipping build relies on the player's own playback error surface rather than adding a startup EME probe; that remains a compatibility risk worth watching on other runtimes.

### 1.3 The one-sentence lesson

`Low` is a trim request, not a memory cap: it works when re-applied at the right moments, and it is wasted if applied before the page has allocated.

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

- **PSS (primary):** sum of `PssBytes` from `QueryWorkingSetEx` over every process in the tree. Shared pages are divided by the number of sharers. `QueryWorkingSetEx` stores the share count in 3 bits, so pages shared by more than seven processes are over-attributed; PSS here is approximate and biased upward for heavily shared runtime pages.
- **Private working set:** resident pages private to each process, from `GetProcessMemoryInfo`.
- **Private commit:** committed private bytes, from `GetProcessMemoryInfo`. This is the number that matters for the pagefile and for “will this fit on 8 GB”.
- **Total working set:** resident bytes including shared pages. It double-counts shared runtime pages and is reported only for completeness.
- **GPU memory:** `GPU Process Memory` performance counters per PID.
- **CPU:** `TotalProcessorTime` delta over the sample window divided by elapsed wall time. Reported as a percentage of one logical core, with all-core percentage alongside.
- **JS heap / DOM:** Chrome DevTools Protocol `Runtime.getHeapUsage`, `Memory.getDOMCounters`, and `Performance.getMetrics` against the page target.
- **Startup:** milliseconds from `main()` entry to the window being ready. This excludes Explorer process creation. Edge and the desktop app are not instrumented and are not compared on this metric.
- **Process tree:** the root process, every descendant, and any `msedgewebview2.exe` whose command line names the app's user-data directory. This captures the browser, GPU, renderer, utility, network, and CDM processes without picking up unrelated WebView2 apps.

### 2.3 States

- **Shell blank:** the host app with a local blank page, Spotify not loaded.
- **Paused:** signed in, player loaded, playback stopped.
- **Playing:** a track playing with the window visible unless marked hidden.
- **Hidden:** window hidden with the close-to-tray path; audio continues.
- **Soak:** 35 minutes of continuous playback, sampled every 5 minutes, with SMTC polled every 12 seconds.
- **Paused/hidden gate:** pause, hide to tray, wait for the pause trim, then sample once a minute for 10 minutes.

### 2.4 Budgets

| Budget | Target |
|---|---:|
| Paused PSS | ≤ 300 MB |
| Playing PSS | ≤ 400 MB |
| Idle CPU | ≤ 0.5% of one core |
| Paused CPU | ≤ 1% of one core |
| Playing CPU | minimized, benchmarked |
| Startup | ≤ 500 ms |

## 3. The release gate

### 3.1 Policy under test

- **Hydration trim:** 30 seconds after the page-load-finished event, set `Normal`, then `Low` 300 ms later.
- **Pause trim:** 25 seconds after `IsDocumentPlayingAudioChanged` reports that audio stopped.
- **Periodic trim:** every 120 seconds, including while paused and hidden.
- **Post-gap trim:** 15 seconds after audio resumes within 20 seconds of a pause, covering track and ad boundaries.
- Both the player WebView and the 40 px chrome WebView receive the same level changes.

The certification harness passed `--gap-sec 10` to exercise boundaries more aggressively than the shipping default of 15 seconds. A shorter gap can only trim sooner; it does not inflate the measured peak. The shipping default has not been separately soaked at 15 seconds, but the post-gap trim is an addition to the 120-second periodic path.

### 3.2 Playing soak

Method: 35 minutes of continuous playback with the window visible at 1280×800 (1294×814 outer) and kept topmost so Windows did not occlude it and release GPU memory. The soak started from the hydrated state, about four minutes after process launch and after the 30-second hydration trim. Every five minutes the harness sampled the full process tree for 10 seconds, read the JS heap and DOM counters over CDP, and polled SMTC every 12 seconds.

| Sample | Elapsed | PSS (MB) | Private WS (MB) | CPU (% one core) | SMTC | Gate |
|---|---:|---:|---:|---:|---|---|
| soak_00m | 0:00 | 243.6 | 204.9 | 10.56 | Playing | PASS |
| soak_05m | 5:00 | 263.3 | 222.1 | 5.90 | Playing | PASS |
| soak_10m | 10:00 | 221.6 | 186.2 | 5.24 | Playing | PASS |
| soak_15m | 15:00 | 216.2 | 179.9 | 5.68 | Playing | PASS |
| soak_20m | 20:00 | 215.9 | 181.7 | 3.83 | Playing | PASS |
| soak_25m | 25:00 | 174.8 | 133.5 | 4.26 | Playing | PASS |
| soak_30m | 30:00 | 252.9 | 205.9 | 8.99 | Playing | PASS |

**Result:** max 263.3 MB, mean 226.9 MB PSS. Every sample is at least 136 MB below the 400 MB limit. SMTC recorded 10 advertisement rows across the run, and playback continued normally at the end. CPU averaged 6.35% of one core while visible and playing.

### 3.3 Paused and hidden

Method: pause from the player control, hide the window with the custom close button (the same path as close-to-tray), wait 75 seconds for the 25-second pause debounce and the trim to take effect, then sample once a minute for 10 minutes with the window hidden.

| Sample | PSS (MB) | CPU (% one core, 10 s) | Window | SMTC |
|---|---:|---:|---|---|
| hidden_00 | 95.1 | 0.00 | hidden | Paused |
| hidden_01 | 52.6 | 1.05 | hidden | Paused |
| hidden_02 | 86.2 | 0.15 | hidden | Paused |
| hidden_03 | 40.7 | 0.00 | hidden | Paused |
| hidden_04 | 71.2 | 0.60 | hidden | Paused |
| hidden_05 | 37.6 | 0.00 | hidden | Paused |
| hidden_06 | 81.9 | 0.45 | hidden | Paused |
| hidden_07 | 36.3 | 0.60 | hidden | Paused |
| hidden_08 | 68.8 | 0.00 | hidden | Paused |
| hidden_09 | 29.3 | 0.30 | hidden | Paused |

**Result:** max 95.1 MB, mean 60.0 MB PSS. Every sample is more than 200 MB below the 300 MB limit. The ten 10-second CPU windows averaged 0.32% of one core. One window reported 1.05%, at the resolution limit of a 10-second sample, so two 60-second steady-state windows were measured after the run: **0.24%** and **0.39%** of one core. The sustained figure meets both the paused (≤ 1%) and idle (≤ 0.5%) budgets.

### 3.4 Startup and hydration

- **Window ready:** 272 ms from `main()` entry to the visible window on the shipping build, warm profile, 1280×800 client. A separate launch with a brand-new app WebView2 profile measured **271 ms**.
- An earlier run on a WebView2 runtime that had never been initialized measured **652 ms**. That is a one-time Windows/WebView2 runtime cost; once the runtime has started normally, a fresh app profile does not reproduce it. It is recorded rather than hidden.
- The player hydrates for roughly the first minute after launch. The hydration trim is applied 30 seconds after page load, and WebView2 reclaims memory asynchronously. The 35-minute soak starts from the hydrated state; the launch transient is not part of the playing-budget samples.
- The 40 px chrome WebView is a second small renderer. It is included in every process-tree total.

### 3.5 Gate verdict

**PASS on all release gates** on the tested Windows 11 x64 machine: playing PSS, paused PSS, paused CPU, idle CPU, startup, and installer behavior. The margins are large enough that the result is not a single-sample accident: the worst playing sample is 263.3 MB against a 400 MB limit, and the worst paused sample is 95.1 MB against 300 MB.

## 4. Baselines

### 4.1 Official Spotify desktop client

Signed in, 1280×800 window, full process tree.

| State | Processes | Total WS | Private WS | Private commit | PSS | GPU | CPU (% one core) |
|---|---|---|---|---|---|---|---|
| Idle after launch | 8 | 906.1 MB | 480.8 MB | 746.4 MB | 672.5 MB | 89.6 MB | 3.45% |
| Paused, 1280×800 | 7 | 968.5 MB | 559.9 MB | 764.1 MB | **754.6 MB** | 93.6 MB | 8.29%* |
| Playing | 7 | 987.5 MB | 574.9 MB | 745.8 MB | **773.4 MB** | 54.8 MB | 3.33% |

*The paused CPU sample included the resize and repaint; it is not a steady-state idle figure.

The desktop client is CEF-based for its UI, but audio is native C++, not EME/Widevine. It is a reference point and a possible fallback, not an embeddable engine: its CEF does not need a CDM for music, so its CDM setup cannot be reused.

### 4.2 Spotify Web Player in Edge

Measured in an Edge `--app=https://open.spotify.com/` window with a dedicated user-data folder, signed in, stock flags, 1296×839 client area, on the logged-in home screen. This is the app-window variant of the Edge baseline; a normal tab carries the same renderer plus a tab strip.

| State | Processes | Total WS | Private WS | Private commit | PSS | GPU | CPU (% one core) | JS heap | DOM nodes |
|---|---|---|---|---|---|---|---|---|---|
| Paused, visible | 12 | 1168.4 MB | 539.8 MB | 894.3 MB | **754.2 MB** | 202.8 MB | 3.72%* | 77.5 MB | 9704 |
| Playing, visible | 12 | 1151.2 MB | 544.5 MB | 771.4 MB | **732.7 MB** | 69.5 MB | 3.32% | 80.3 MB | 9860 |

*The paused CPU includes the first paint after the window was restored; it is not steady state.

Notes:

- **Edge is roughly desktop-client weight.** 733–754 MB PSS is in the same band as the official Spotify client (673–773 MB) and about 240 MB above the embedded WebView2 host. The Spotify renderer itself is comparable (roughly 288–314 MB PSS); the difference is Edge's own browser process, extra renderer, GPU, CDM, audio service, and search indexer.
- **The first launch is not a valid sample.** A fresh Edge profile added a hidden `edge://newtab` preload and a login-flow Google reCAPTCHA iframe, inflating the tree to 1209.7 MB paused / 1257.5 MB playing with 16k–23k DOM nodes. Re-launching on the signed-in profile removed the preload and the numbers fell to the table above.
- **Occlusion releases memory.** With the window behind another app (page `visibility: hidden`), the same session measured 482.4 MB PSS because the compositor and GPU allocations were released. Visible-window numbers are used for the budget, since that is how a music player is normally used.
- **Ads played untouched.** Starting playback triggered two 30-second audio ads (`Advertisement • 1 of 2`), which played to completion. After the break the page stopped at 0:00 instead of rolling into the next track and needed one click; this was observed once and is not attributed to the host.

### 4.3 Edge PWA / `--app` window

The 4.2 table is the `--app` window, which is the form an installed PWA takes on Windows: an Edge-owned window with no tab strip or address bar, running in a dedicated profile. A separately installed PWA would reuse an existing Edge browser process unless given its own profile, so its steady-state tree is the same or larger. No separate PWA install was measured; the app-window result is the lower bound for this architecture.

### 4.4 Phase 3 Tauri spike

Tauri 2.12.7, remote web player, no capabilities, navigation allowlist, tray, and SMTC.

| State | Processes | Total WS | Private WS | Private commit | PSS | CPU (% one core) |
|---|---|---|---|---|---|---|
| Paused visible | 10 | 1102.7 MB | 426.1 MB | 701.8 MB | **558.0 MB** | 0.00% |
| Playing visible, 30 s | 10 | 1105.8 MB | 438.7 MB | 595.1 MB | **594.8 MB** | 8.35% |
| Playing hidden, 60 s | 10 | — | 446.6 MB | 613.8 MB | **603.0 MB** | 4.48% |
| Hidden paused, 5 min | 10 | — | 343.9 MB | 787.8 MB | **535.9 MB** | 0.00% |

The spike had no memory policy. It proved the Spotify experience but was never a performance baseline.

### 4.5 Bare wry + WebView2 shell

wry 0.57.4 + tao 0.37.3, no Tauri. The table covers the local blank shell and the remote Spotify page in the same host.

| State | Processes | Total WS | Private WS | Private commit | PSS | GPU | CPU (% one core) | Window ready |
|---|---|---|---|---|---|---|---|---|
| Blank shell | 8 | 346.4 MB | 65.7 MB | 143.7 MB | **121.4 MB** | 26.1 MB | 1.00% | 282 ms |
| Spotify paused | 10 | 812.4 MB | 364.6 MB | 526.2 MB | **488.5 MB** | 80.4 MB | 0.00% | 177–185 ms |
| Spotify playing visible | 10 | 822.2 MB | 373.8 MB | 513.2 MB | **497.7 MB** | 60.4 MB | 3.75% | — |

The bare shell reproduced the whole Spotify experience without Tauri: login persisted, Widevine worked, audio played, and background playback continued. Reusing the spike's WebView2 user-data folder preserved the session once the data directory was corrected to the **parent** of `EBWebView` (WebView2 appends `EBWebView` itself).

### 4.6 Minimal Tauri shell

Minimal Tauri 2.12.7 app, local page, no commands.

| State | Processes | Total WS | Private WS | Private commit | PSS | GPU | CPU |
|---|---|---|---|---|---|---|---|
| Blank shell | 7 | 345.3 MB | 66.5 MB | 156.1 MB | **117.7 MB** | 36.3 MB | 0.00% |

### 4.7 Early Edge app-window probe

A first probe with `--app=about:blank` opened a New Tab and was still loading:

| State | Processes | Total WS | Private WS | Private commit | PSS | GPU | CPU |
|---|---|---|---|---|---|---|---|
| New Tab, 20 s | 9 | 682.0 MB | 312.2 MB | 479.4 MB | 390.0 MB | 108.9 MB | 14.68% |

This is **not** a valid Edge baseline; it is recorded only to show that the measurement harness can isolate a dedicated Edge profile tree. The real baseline uses `--app=https://open.spotify.com/`.

## 5. Framework comparison: wry vs Tauri

The shells differ by a few megabytes, not by hundreds:

- Bare wry + WebView2: 121.4 MB PSS blank.
- Minimal Tauri + WebView2: 117.7 MB PSS blank.
- Phase 3 Tauri spike with Spotify paused: 558.0 MB PSS.
- Same Spotify page in bare wry: 488.5 MB paused, 497.7 MB playing.
- Same page in an Edge app window: 754.2 MB paused, 732.7 MB playing.

The 60–70 MB difference between the spike and the bare host is page/profile state and sample timing, not framework overhead: the spike had a fresh profile and its page had fully hydrated, while the wry run reused the warmed profile. The blank-shell numbers prove the point: Tauri and wry cost essentially the same once WebView2 is running.

**Conclusion:** choosing a lighter Rust framework cannot solve the memory problem. Tauri's value is tray, updater, single-instance, and packaging; wry's value is a smaller dependency surface and direct control of the controller, environment, and memory API. The shipping build implements the Windows features directly and uses `wry` + `tao`; the Tauri spike remains as the Phase 3 record.

## 6. WebView2 memory behavior

### 6.1 `MemoryUsageTargetLevel.Low`

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

*The 0.92% paused sample includes the 30-second window after the trim; it is still under the 1% target. The earlier paused CPU figures include repaint, hydration, or sampling after a trim and are not steady state. `Low` at launch is much less aggressive than a trim on a hydrated page (229 MB versus 35–53 MB). The warm rows come from the same 40-minute session, not a fresh profile; a 60-second pause immediately after the soak measured 401.0 MB before the settled 387.3 MB pre-trim state (see §8).

In practice, `Low` behaves like a one-shot trim request, not a continuous cap: setting it before the page loads did not prevent the soak from starting at 465.8 MB after a playlist navigation, while re-applying it to a settled paused page produced 35–53 MB. A production host should apply `Low` at defined lifecycle points — after the page has hydrated, after a pause has been confirmed, and periodically during playback — and measure after each application.

Two findings changed the Phase 3 conclusion:

1. **Low does not break playback.** The earlier “spinner” was a click on the seek bar, not a failed play. With Low active, the playhead advanced and audio continued.
2. **Low is not a free win.** It cuts resident memory by paging aggressively; commit stays 480–620 MB, and the page can fault back in quickly. The 35-minute one-shot soak showed that a single trim does not hold the playing budget; periodic re-trims are required.

### 6.2 `TrySuspendAsync`

Suspension requires `IsVisible = false`, pauses script timers and animations, and lets the OS reclaim renderer memory. It is the wrong tool for a music player: playing audio blocks suspension, and the page's state machine, websocket, and playback position would stop. It could only be used when paused **and** hidden, but `Low` already covers that state without stopping scripts. Not recommended, and not used in the shipping build.

### 6.3 Process model and shared environments

- One browser process per user-data folder. Every WebView2 app pointed at the same folder joins that browser process and shares GPU/utility/network processes.
- Renderers are still created per site/frame by Chromium's process model; sharing a folder does not collapse the Spotify renderer.
- A shared folder is **not** a security boundary: cookies, cache, and storage are shared.
- Two separate Rustify instances with the same folder would share the browser process only if their environment options match. That is a possible future single-instance strategy, but it couples crash domains and is not needed; the shipping app enforces a single instance at the app level.
- Fixed Version WebView2 adds >250 MB to the package; Evergreen reuses the system runtime. Evergreen is the right choice for a small installer, at the cost of depending on the installed runtime.

### 6.4 Chromium flags

WebView2 forwards `AdditionalBrowserArguments` to the browser process; flags apply to every WebView in that user-data folder and are explicitly **not for production** (Microsoft may remove or change them). Candidate flags and their expected effect:

| Flag | Expected effect | Risk |
|---|---|---|
| `--enable-low-end-device-mode` | Forces low-end heuristics; may reduce cache sizes and renderer limits | Version-dependent, undocumented in WebView2 |
| `--renderer-process-limit=1` | Collapses renderer processes | Site isolation weakened; Spotify may still need its own renderer |
| `--disable-gpu` | Removes GPU process/VRAM cost | Microsoft says troubleshooting only; breaks compositing performance |
| `--js-flags=--max-old-space-size=N` | Caps V8 old space | Page may crash or GC-thrash; not a WebView2 contract |

These were not needed and are not shipped. The lifecycle policy is preferred over undocumented flags because shipping them would violate the “supported configuration” principle of the design.

## 7. The shipping policy

The shipping shell applies the following, to both WebViews:

| Event | Action | Why |
|---|---|---|
| Page load finished + 30 s | `Normal`, wait 300 ms, `Low` | Let the player hydrate and allocate before trimming |
| Audio stopped + 25 s | `Normal`, wait 300 ms, `Low` | Confirmed pause; track gaps shorter than the debounce do not trigger it |
| Every 120 s | `Normal`, wait 300 ms, `Low` | Reset cache growth during long playback, including paused/hidden |
| Audio resumes within 20 s of a pause + 15 s | `Normal`, wait 300 ms, `Low` | Reset allocations at track and ad boundaries |

`Low` is one-shot, so the shell deliberately returns to `Normal` and re-applies `Low` instead of assuming the level is still active. The 300 ms `Normal` dwell gives WebView2 a chance to process the change before the trim request.

The pause path is driven by `ICoreWebView2_8.IsDocumentPlayingAudioChanged`, not by a UI click. That matters: the player can stop audio for its own reasons, and the shell should react to the audio state rather than infer it from a button. The 25-second debounce prevents a trim during a short track gap; the post-gap trim then catches the refill after playback resumes.

**Tradeoff:** commit and pagefile use remain high, and the first interaction after a long paused period may be slower while pages fault back in. The 10-minute paused run measured 0.24–0.39% sustained CPU, but input latency after a long pause was not separately instrumented. The design is a Windows memory-pressure optimization, not a way to make the page use less virtual memory.

## 8. Why the one-shot policy failed

Two 35-minute soaks were run on the bare wry host before the shipping policy existed, both playing a Spotify-owned playlist continuously with the window visible, SMTC polled every 12 seconds, and metrics sampled every 5 minutes. They explain why the lifecycle exists.

### 8.1 Soak A — `Low` set before navigation

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

- **It plateaus; it does not leak.** PSS fell from 465.8 to 426.8 MB over the first 10 minutes (the page releasing hydration allocations) and then stayed in a 427–436 MB band for the remaining 25 minutes. JS heap was flat at 60–63 MB and DOM at ~5k nodes.
- **It misses the playing budget by ~8–9%.** The plateau is 427–436 MB against a 400 MB target.
- **Private commit is stable at 578–599 MB.** The page is not hiding memory in the pagefile during playback; the resident number is real.
- **An advertisement played normally.** At 31.2–32.0 minutes SMTC reported `Advertisement / Playing` for a ~30-second spot (five 12-second polls, `end_s = 30.1`), then playback resumed into the queue at 32.2 minutes. Nothing blocked, skipped, or muted it.
- **Playback was continuous.** 15 tracks over 35 minutes; the only gap was a single 12-second `NoSession` poll at a track boundary (22.6 min), which is an SMTC reporting gap, not an audio stop.
- **CPU averaged 5.2% of one core** (2.8–6.5% band) while visible and playing.

### 8.2 Soak B — `Low` applied to a hydrated, paused page, retained during playback

Same host and page, but `Normal` at launch, then a settled `Low` trim while paused, then playback resumed with `Low` retained. This was the candidate production policy and the run that decided the playing budget. The run started at 19:06 UTC on host PID 4772, window visible, and lasted 35 minutes.

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
- **Both breaches came at ad boundaries.** The 20-minute sample followed the first 30.1-second ad (14.0–14.8 minutes); the 30-minute sample was taken during the second (30.0–30.8 minutes). Playback resumed after each. Memory rose through the ad and was still elevated at the next sample.
- **Playback was continuous.** 168 SMTC polls, all `Playing`, 11 tracks, no `NoSession` gap. CPU averaged 5.87% (3.24–10.91%); JS heap stayed at 58–73 MB and DOM at 4.7–4.9k nodes. The tree stayed at 10 processes.
- **The post-soak paused state fails without a re-trim.** Visible playing measured 413.2–419.3 MB; paused after the run with `Low` still set but no new trim measured **401.0 MB** at 0.91% CPU, and resume worked. The level was still active, but new allocations were not reclaimed.
- **A pause re-trim fixes it.** On the same warm session, Normal→Low on the paused page moved **387.3 → 60.5 MB PSS**, with commit 613.7 MB, heap 67.2 MB, DOM 4781, and CPU 0.92%. Resume advanced the track from 1:05 to 1:17.
- **A playing re-trim also worked in one sample.** **259.0 → 206.0 MB PSS**, commit 654.9 → 618.0 MB, CPU 5.49%, playhead 2:03 → 2:46. No stall or SMTC gap was observed.

| Post-soak experiment | Before | After | Commit after | CPU | Playback |
|---|---|---|---|---|---|
| Warm paused, Normal→Low | 387.3 MB | **60.5 MB** | 613.7 MB | 0.92% | resumed at 1:17 |
| Warm playing, Normal→Low | 259.0 MB | **206.0 MB** | 618.0 MB | 5.49% | advanced 2:03 → 2:46 |
| Paused after soak, no re-trim | — | 401.0 MB | 688.3 MB | 0.91% | resumed |
| Hidden via `SW_HIDE`, playing | — | 417.9 MB | 628.3 MB | 5.35% | continued; page still reported visible |

**Hidden caveat:** `SW_HIDE` hid the top-level window, but the page's `document.visibilityState` stayed `visible` because WebView2 did not propagate the parent's hidden state. The 417.9 MB sample is therefore effectively another visible-window sample, not evidence that hidden playback is cheaper. The shipping app's hidden/paused result is achieved by the lifecycle, not by relying on page visibility.

**Soak B verdict:** the one-shot candidate policy failed. The page is not leaking, but it refills cache after trims and after ads, and `Low` must be re-applied to keep it down. That conclusion drove the 120-second periodic trim and the post-gap trim in the shipping build.

## 9. Spotify page bottlenecks

- The Spotify renderer alone is **286–316 MB PSS** in normal mode; browser, GPU, utilities, and shell add roughly 200–280 MB. No shell change can remove the renderer floor; the lifecycle reduces residency by paging, not by deleting the page.
- JS heap is 69–76 MB, with ~8.8–9k DOM nodes and ~4.5–4.7k JS event listeners. This is a large, long-lived single-page app, not a thin audio element.
- The page keeps allocating after playback starts. In the first six minutes after a `Low` trim, PSS rose from 225.5 MB to 395.3 MB while playing; soak A then flattened at 427–436 MB for 25 minutes. Soak B, trimmed on a hydrated paused page, started lower (319.0 MB) but still climbed to 429.8 MB at 20 minutes and 413.0 MB at 30 minutes, both at ad boundaries. The growth is cache refill, not an unbounded leak, but a one-shot trim does not keep the plateau under 400 MB; re-trimming resets it to 60.5 MB paused and 206.0 MB playing.
- Public reports of the web player growing to 1.7 GB after two songs (2016), ~6 GB before OOM in 1–2 hours (2024), and >2 GB with Brave Shields are unverified but consistent in direction: the page has a history of unbounded growth in some configurations. This report uses only its own soak as evidence for the verdict.
- Ads were audio-only in every observed break (one in soak A, two in soak B, two in Edge) and are served by the page itself. Blocking none of them is both a product requirement and the simplest path: no filters, no proxies, no DNS changes.
- Background playback works when minimized and hidden; the page does not pause on visibility change. Chromium throttles rendering for occluded windows (CPU falls), but audio and the Connect session continue.

## 10. Shell overhead and the chrome decision

- Blank wry shell: 121.4 MB PSS, of which the Rust process is ~5–6 MB; the rest is WebView2's browser/GPU/renderer/utility floor.
- Minimal Tauri shell: 117.7 MB PSS, same floor.
- Window creation: 177–282 ms for the wry host, 401 ms for the Phase 3 Tauri spike. The shipping shell measured 272 ms with two webviews and the full product chrome.
- Installer: 1.08 MiB for the Phase 3 Tauri NSIS installer; the shipping `wry` installer is **534,512 bytes** (about 535 KB), plus the system WebView2 runtime.
- The Rust shell's own CPU is effectively zero; the idle budget is consumed by WebView2 and the page, not by Rust.
- **Custom chrome:** a 40 px local WebView title bar in a frameless host. Window ready was **298 ms** with two webviews in the benchmark; the tree stayed at 10 processes but gained a renderer. The bar renderer measured **38.5 MB PSS** before trims, and the paused tree measured 563.8 MB (private 407.4 MB, commit 598.6 MB, GPU 105.8 MB) at 2.35% CPU. After the lifecycle trims, the bar is small; the certified 95.1 MB paused and 263.3 MB playing totals include it.

**Decision:** the exact Spotify-style bar was a hard product requirement, so the shipping build keeps the second WebView and accepts its renderer cost. The alternative — a native GDI/Direct2D or DWM-themed caption — would avoid the renderer but would not produce the required visual result. The cost is measured rather than hidden: 38.5 MB before trims, included in every certified total.

**Conclusion:** the shell is not the problem and cannot be the solution. Optimization effort belongs in WebView2 state policy (`Low`/`Normal`, timing, process sharing), not in rewriting the host in a different framework.

## 11. Alternative architectures

### 11.1 Edge PWA or `--app` window

- Runs in Edge's own process tree and profile; no separate runtime, no CDM work. Widevine comes from Edge, updates come from Edge.
- An installed PWA is a normal app window in the existing Edge browser process; it does not get a separate process group unless a separate profile is used.
- `--app="URL" --user-data-dir="..."` gives a dedicated profile and a measurable tree, but flags only apply when a new browser process starts.
- Costs: no tray integration or shell control; coupled to Edge's version, profile, and update policy; the app window is a browser window with browser chrome removed, not a product shell. A shared profile is not a security boundary.
- Still the lowest-effort fallback if a Rust host fails the budgets, but it measured 733–754 MB PSS here and carries 12 processes.

### 11.2 CEF

- Widevine is enabled by default since M93 and fetched by the component updater, but CEF builds need `enable_widevine`, `enable_cdm_host_verification`, `enable_cdm_storage_id`, and `enable_rlz`; Windows ARM64 is unsupported.
- VMP/host verification is the sharp edge: persistent-license and VMP failures are tracked in CEF issues #3404 and #3952; Windows VMP with bootstrap was broken until M139.
- Size: a M107 x86 release is ~169 MB without PGO (~181 MB with), with `libcef.dll` alone ~156–166 MB. A Spotify-sized CEF app is 150+ MB before content.
- Maintenance: branches track every other Chromium milestone (~4 weeks), and Chrome moves to a two-week major cycle in September 2026. A milestone update is roughly a week of work, mostly renames and reorgs.
- Technically the most control, and the highest permanent cost. Not justified for a shell whose only job is to display one origin.

### 11.3 Electron

- Stock Electron does not ship a usable Widevine: docs call it experimental, the app must supply the CDM, and VMP signing may be required. PR #50046 (merged 2026-10-15 for 46-x-y) adds CDM support, but the CDM files and production VMP signing remain the app's responsibility.
- castLabs Electron for Content Security (ECS) is the production path: runtime CDM install/update and VMP, Windows/macOS. It requires EVS signing for production.
- Size: Electron apps are typically >100 MB installed and 100+ MB zipped; one 2026 third-party measurement reports 106 MB installer / 367 MB on disk, unverified.
- Not competitive with WebView2 for a single-origin shell: larger installer, duplicate Chromium, CDM/VMP ownership.

### 11.4 Official desktop client

- Its CEF draws the UI only; audio is native. It is not a reusable Widevine host and cannot be embedded.
- Measured 672–773 MB PSS, heavier than the Phase 3 spike. It is a fallback product, not an architecture.

### 11.5 Other options

- Legacy MSHTML/Trident: no EME. Ruled out.
- Qt WebEngine / NW.js: Chromium shells with the same CDM/VMP work as CEF, not evaluated.
- WebView2 Fixed Version: larger package, manual updates, reported to lack PlayReady; no reason to choose it over Evergreen here.

### 11.6 Widevine in WebView2: what is actually supported

Microsoft has **no official statement** that Widevine is supported in WebView2. Widevine's own overview lists Edge and CEF/Electron, not WebView2. The feature request for official support (#4828) has been open since September 2024. Against that, the Evergreen runtime ships a `WidevineCdm` component, Microsoft engineers have said WebView2 supports Widevine out of the box because it ships with Edge, and this machine's Phase 3 probe returned `com.widevine.alpha` success and played DRM audio for hours.

The honest position: **it works on the tested runtime and is not a contractual guarantee.** The shipping build relies on the player's own playback error surface rather than adding a startup EME probe. If a future runtime or a corporate-managed machine lacks the CDM, the player will fail where a browser would; a startup probe would make that failure clearer, and is worth adding if support burden grows.

## 12. Final recommendation

**Ship `wry` + `tao` + WebView2 with the lifecycle policy above.** The measured result meets all budgets with margin:

- Paused: 95.1 MB peak, 60.0 MB mean.
- Playing: 263.3 MB peak, 226.9 MB mean.
- Paused/idle CPU: 0.24–0.39% sustained.
- Window ready: 272 ms warm, 271 ms fresh profile.
- Installer: 534,512 bytes, per-user, no admin.

Keep the product decisions that made the shell feel finished:

- The exact Spotify-style caption bar, with its second WebView, because it is a product requirement and fits the budget.
- Tray show/hide, close-to-tray, single-instance activation, and persisted window state.
- The navigation allowlist, popup/download blocking, and permission denial.
- One scoped CSS rule for the two nav-bar upsells, disclosed as cosmetic and not extended into ad blocking.
- No proxies, no token extraction, no undocumented Spotify APIs, and no modification of ads, DRM, or playback.
- No undocumented Chromium flags; the lifecycle is the supported mechanism.

Do **not** replace the lifecycle with a one-shot `Low`. The page refills after trims and ads; the periodic and post-gap re-applications are what hold the budget. Do not use `TrySuspendAsync` while audio is expected. Do not add renderer limits or GPU-disabling flags to chase a smaller number.

## 13. Open questions

Resolved by the 0.2.0 gate:

- **Does periodic `Normal -> Low` hold ≤ 400 MB for 35 minutes?** Yes: 263.3 MB max.
- **Does a 10-minute hidden/paused run stay ≤ 300 MB?** Yes: 95.1 MB max.
- **Does the second WebView for the caption bar fit the budget?** Yes; the certified totals include it.
- **Does `Low` break playback or ads?** No; playback advanced and 10 ad rows were recorded during the soak.
- **Is the startup budget met?** Yes, 272 ms warm and 271 ms with a fresh app profile. The 652 ms first-ever runtime initialization is a disclosed one-time outlier.
- **Does Edge beat the embedded host?** No. Edge is 733–754 MB PSS versus 95/263 MB for the shipping shell.

Still open:

- Behavior on Intel integrated GPUs and 125–150% DPI, where GPU memory and composition costs differ.
- Sessions longer than 35 minutes; the periodic trim should hold, but an 8-hour drift test has not been run.
- Input latency after a long paused period, when pages fault back in.
- Behavior on 8 GB machines, where private commit of 600–690 MB and pagefile pressure matter more than PSS.
- Fixed Version WebView2 runtimes and Widevine availability.
- Social-login popups, which the strict top-level allowlist blocks; email/password sign-in works.
- Code signing and auto-update, neither of which is in 0.2.0.
- A native-painted caption alternative if the exact Spotify bar is ever relaxed.

## 14. Reproduction

The harness is in [`bench/measure/`](../bench/measure/):

```powershell
# Build the measured binary (shipping shell + local CDP for the harness).
cargo build --release --features bench --manifest-path app\Cargo.toml

# Launch with the local CDP port and the shipping policy timings.
.\app\target\release\rustify.exe --cdp 9222

# Start playback, then sample the process tree.
.\bench\measure\soak.ps1 -RootId <pid> -Minutes 35 -Port 9222 -OutDir bench\soak-app-final

# Pause, hide to tray, wait for the pause trim, then:
.\bench\measure\hidden-soak.ps1 -RootId <pid> -Minutes 10 -OutDir bench\soak-hidden
```

The raw soak output contains local process data and Spotify metadata. It is kept out of the public repository by `.gitignore`; only the aggregated, title-free numbers are published.

## Appendix A - Historical evaluation: Spicetify and ad blocking

> **Superseded.** This appendix records the 0.2.0-era evaluation. Adblocking is now a first-class requirement on `feature/adblocking`. The risks below still matter, but they are no longer a prohibition.

These were evaluated at the user's request and were **not supported in 0.2.0**. The Phase 0-2 allowlist and acceptance criteria required the official web player, intact advertisements, and no general page injection, extensions, proxies, or modified clients. The shipped upsell CSS was a single documented cosmetic exception, not a precedent for ad blocking.

### A.1 Spicetify

Spicetify is a CLI that customizes the **official desktop client** by patching its installed `xpui.spa` bundle on Windows, macOS, and Linux, and it must be re-applied after each Spotify update ([getting started](https://spicetify.app/docs/getting-started), [FAQ](https://spicetify.app/docs/faq)). Rustify hosts `https://open.spotify.com` in WebView2; there is no `xpui.spa` and no desktop-client UI to patch.

"Support" would therefore require one of:

- switching Rustify to launch or patch the desktop client instead of hosting the web player - a different product, with the measured 673-773 MB PSS baseline and recurring update breakage; or
- reimplementing Spicetify-style injection against the web player - still out of scope for the web-player shell.

Spicetify's Marketplace also offers the `adblockify` extension. It is not a code source for Rustify; adblocking work is independently implemented under the current product direction.

### A.2 Ad blocking

WebView2 has no single supported ad-block API. The technical routes and their current status are:

| Route | Risk and current status |
|---|---|
| `WebResourceRequested` host filtering | Ads share Spotify CDN and playback infrastructure with tracks; filtering is brittle, detectable, and can break audio or DRM. Under evaluation for native filtering with a fail-open path. |
| Script/CSS injection | Hides ad UI but does not stop audio ads; general injection remains out of scope. The existing nav-bar rule is the only stable cosmetic change. |
| Edge extension (uBlock Origin) | Available through WebView2 extensions; shipped as the 0.3.0 uBOL experiment. GPL-3.0; not proven to remove current audio ads. |
| DNS/proxy/MITM filtering | Machine-wide, breaks other applications, weakens security; still prohibited. |

The free tier is ad-supported. Spotify's terms restrict modifying the client and circumventing restrictions, and modified or blocking setups carry account risk. Ad blocking also does not help the memory budgets: an extension or filter list is a few MB, while the page and Chromium dominate.

**Historical decision:** ads played untouched in 0.2.0, as specified. **Current decision (2026-11-26):** adblocking is a first-class requirement. Work is on `feature/adblocking`, must preserve playback/DRM/auth, must fail open, and requires a new certification. The 0.2.0 numbers above do not apply to it.

> **Update for 0.3.0-pre.1:** WebView2 supports unpacked extensions, so the decision was revisited. The experimental uBOL build adds uBlock Origin Lite and is not covered by the 0.2.0 certification above. See [UBOL_INTEGRATION_RESEARCH.md](UBOL_INTEGRATION_RESEARCH.md) and the [0.3.0-pre.1 release notes](releases/v0.3.0-pre.1.md).

## Appendix B — Bench-only experiments

These are opt-in flags in `bench/wryhost`. They were **not** active during the Phase 4 performance soaks; the shipping app uses the polished versions of the chrome and upsell rule.

### B.1 Upsell hiding

`--hide-upsells` injects one `<style>` element at document load, main frame only:

```css
[data-testid="global-nav-bar"] button[data-testid="upgrade-button"],
[data-testid="global-nav-bar"] a[href="/download"] { display: none !important; }
```

Verified on the live page: both elements computed `display: none` at 0×0, one style tag, no observers, no network or behavior changes. The selectors use Spotify's `data-testid`/`href` hooks and fail safe: if the hooks change, the buttons simply stay visible. The shipping app applies the same scoped rule as a disclosed cosmetic default; it is not an ad blocker and has no measurable performance effect.

### B.2 Custom chrome

Frameless window (`decorations(false)`) with a 40 px child WebView bar, local HTML/CSS/SVG, and IPC commands `drag`, `minimize`, `toggle-max`, `close`.

- Verified: maximize (window zoomed), restore, minimize, drag (+120/+72 px), and close (host exits, zero profile children left behind).
- Two WebViews sharing a user-data folder must use identical `CoreWebView2EnvironmentOptions`. The bar first failed with `HRESULT 0x8007139F` until it received the same additional browser arguments as the main view. This is a wry/WebView2 requirement, not a Rustify bug.
- Cost: the bar's renderer measured 38.5 MB PSS before trims; the paused tree measured 563.8 MB. The shipping app accepts this cost because the exact Spotify-style bar is a product requirement, and the certified lifecycle totals include it.
- The shipping bar also fixes two prototype issues: only a mousedown with `detail >= 2` toggles maximize (there is no duplicate `dblclick` handler), and the bar starts with the correct maximized body class when launched maximized.

## Appendix C - Adblocking branch smoke test

> **Not certification.** These numbers are from a local synthetic page, not real Spotify playback. They compare the release `bench` builds on one machine and show the order of magnitude of the adblocking overhead. The full test evidence is in [ADBLOCKING_VALIDATION_REPORT.md](ADBLOCKING_VALIDATION_REPORT.md) §8.

The `v0.3.0-adblock-pre.1` build adds title detection, a mute fallback, and an opt-in discovery timer. The default build has none of that code. Both variants were measured with `bench/measure/tree-metrics.ps1` on a fresh temporary profile.

Method:

- Release `bench` control and `bench adblock` builds from the same commit.
- 15-second PSS/CPU samples after the page settled; the transition run was 30 seconds; the idle check was 60 seconds with the normal memory trim enabled.
- The synthetic page plays a Web Audio tone. The "ad" state changes the document title and media-session title to the ad marker; the audio keeps playing so mute can be observed.
- CPU is percent of one logical core on the 16-core test machine.

| Build and state | Processes | PSS | CPU, one core |
|---|---:|---:|---:|
| Control, idle | 8 | 134.8 MB | 0.53% |
| `adblock`, idle, discovery off | 8 | 137.7 MB | 0.51% |
| Control, music | 9 | 171.0 MB | 2.52% |
| `adblock`, music | 9 | 173.9 MB | 2.42% |
| Control, ad title | 9 | 174.4 MB | 1.30% |
| `adblock`, ad muted | 9 | 173.3 MB | 1.21% |
| `adblock`, discovery, 2 s interval | 9 | 173.8 MB | 3.13% |
| `adblock`, repeated transitions, 30 s | 9 | 178.9 MB | 2.82% |

| Build | Processes | PSS | CPU, one core |
|---|---:|---:|---:|
| Control, 60 s idle with trim | 8 | 28.1 MB | 0.29% |
| `adblock`, 60 s idle with trim | 8 | 32.5 MB | 0.31% |

The transition run performed 5 accepted skips, 4 ad breaks, and 4 mute/unmute cycles in about 31 seconds. It is an aggressive synthetic stress test, not a real listening pattern.

Observations:

- No new persistent process. Idle is 8 processes; playing states add one renderer (9).
- PSS is within about 3 MB of control in music and ad states; run-to-run noise is larger than the difference.
- Discovery adds 0-5 MB PSS depending on run and roughly 1-3% of one core for its timer and UI updates.
- The 60-second PSS is much lower than the 15-second PSS because the hydrate trim has run.

CPU samples are noisy on this machine; the control music sample (2.52%) was higher than the `adblock` music sample (2.42%), which is not physically meaningful. These are smoke numbers. They do not cover real Spotify playback, real ads, DRM, or a long soak, and they are not a release gate.

## Sources

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
