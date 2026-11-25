# Rustify Phase 3 — Windows DRM / Background Playback Spike

**Date:** 2026-11-25  
**Status:** Spike run complete. Phase 4 addendum at the end supersedes the memory verdict.  
**Scope:** Windows 10/11 x64 only. This is a throwaway spike, not V1.

## 1. Question

Can an unmodified official Spotify web player run in a Tauri 2 / WebView2 window on a **free** account, with ads, and continue playing when minimized or hidden to the tray, at acceptable RAM/CPU and installer size?

## 2. Build under test

- Tauri `2.12.7`, `tauri-build` `2.7.6`, `@tauri-apps/cli` `2.12.6`.
- One remote webview: `https://open.spotify.com/`.
- `app.withGlobalTauri = false`.
- **No capabilities directory at all.** The remote origin is not in any capability and no `remote.urls` are granted.
- Top-level navigation allowlist: `open.spotify.com`, `accounts.spotify.com`, `www.spotify.com`, `challenges.cloudflare.com`. Everything else is cancelled and logged.
- Tray: Show / Hide to tray / Quit. Close button hides to tray.
- Native SMTC poller (Windows `GlobalSystemMediaTransportControlsSessionManager`) logs playback status, position, title and artist every 5 s.
- Diagnostic probe (spike only): a fixed `eval` runs `navigator.requestMediaKeySystemAccess('com.widevine.alpha', …)` once after the Spotify page loads, then reports UA and the result by attempting a navigation to `/__rustify_probe__`. The navigation handler logs it and cancels the navigation, so the page is not modified and no page→native IPC exists.
- Release profile: `lto = true`, `codegen-units = 1`, `opt-level = "s"`, `strip = true`, `panic = "abort"`.

Build command:

```powershell
npx tauri build --bundles nsis
```

## 3. Measurement definitions

- **Working set / private bytes:** sum over the app process, every descendant process, and any `msedgewebview2.exe` whose command line references this app's user-data directory. This includes WebView2 browser, GPU, renderer, utility and network processes.
- **Private working set:** `Win32_PerfFormattedData_PerfProc_Process.WorkingSetPrivate`, summed over the same set. This excludes shared pages, so it is a **lower bound** on PSS; total working set is an upper bound. The true PSS lies between them.
- **CPU:** total process CPU time delta over the sample window, divided by elapsed time. Reported as % of one logical core and as % of all logical cores (16 cores on this machine).
- **Shell idle:** window open with no Spotify page loaded.
- **Paused:** signed in, player loaded, playback stopped.
- **Playing:** a track playing.
- **Startup:** time from `main()` entry to the Tauri window being created, logged by the spike. Excludes process creation by Explorer.
- **Installer size:** the NSIS `.exe` produced by `tauri build`.

## 4. Procedure

1. Delete `%LOCALAPPDATA%\com.rustify.spike` and `%LOCALAPPDATA%\RustifySpike` for a clean profile.
2. Launch the release executable.
3. Wait for the probe; record UA, `isSecureContext`, EME presence, Widevine key-system result and robustness.
4. Measure the landing-page state.
5. Sign in manually with a free account (password is never handled by the harness).
6. Measure paused, then playing.
7. Minimize for 30 s; pause with a media key and read the SMTC position delta.
8. Hide the window completely for 30 s; pause and read the SMTC position delta.
9. Keep playing until Spotify inserts an ad; record SMTC metadata and timestamps.
10. Measure installer size.

## 5. Pass criteria

- Widevine `requestMediaKeySystemAccess` succeeds with `com.widevine.alpha`.
- A free-account track plays and position advances.
- An inserted ad plays.
- Position advances while minimized and while hidden to the tray.
- Working set within the revised budgets: ~120 MB shell idle, ~300 MB paused, ~450 MB playing; CPU ≤2% paused.
- Installer ≤30 MB.

## 6. Results

### 6.1 Build artifacts

| Artifact | Size |
|---|---|
| `rustify-spike.exe` | 3.05 MiB (3,193,344 bytes) |
| `Rustify Spike_0.1.0_x64-setup.exe` (NSIS) | **1.08 MiB** (1,137,686 bytes) |
| Bundled Chromium | none (system WebView2 158.0.4370.33) |

Installer budget ≤30 MB: **PASS with ~28× headroom**. The WebView2 runtime is a separate system component and was already installed; it is not counted in the installer, per the budget.

### 6.2 Widevine and user agent

Probe at 16:26:28 UTC, 6 s after the Spotify page finished loading:

| Field | Value |
|---|---|
| `navigator.userAgent` | `Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/158.0.0.0 Safari/537.36 Edg/158.0.0.0` |
| `isSecureContext` | `true` |
| EME present | `true` |
| `requestMediaKeySystemAccess('com.widevine.alpha')` | **success** |
| `keySystem` | `com.widevine.alpha` |
| robustness | `""` (empty; no robustness level requested/returned) |
| `window.__TAURI_INTERNALS__` | `object` (Tauri IPC bootstrap is injected) |
| `window.__TAURI__` | `undefined` (`withGlobalTauri=false`) |

No capability file exists, so no command is granted to the remote origin. The bootstrap object being present is expected; the ACL has no entry for this origin.

### 6.3 Free-account playback

- Signed in through `accounts.spotify.com` with email/password.
- The UI showed **Explore Premium**, confirming a free account.
- Track played: an ordinary Spotify track, position advanced on screen, pause button visible.
- SMTC reported `status=Playing`, with title and artist populated.
- Multiple subsequent tracks played automatically.

**Free-account playback: PASS.**

### 6.4 Ad playback

The ad appeared after ~28 minutes of listening:

| Time (UTC) | SMTC |
|---|---|
| 16:57:40 | `status=Playing pos=138.3 title=<track> artist=<artist>` |
| 16:57:45 | `status=Playing pos=0.1 title=Advertisement artist=` |
| 16:57:45–16:58:40 | `title=Advertisement`, status `Playing`, position 0.1–0.2 |
| 16:58:45 | `status=Playing title=<next track>` |

- The ad ran for approximately **60 seconds** and was followed by the next track.
- No blocked navigations were logged (`blocked_count=0`), so the allowlist did not intercept ad frames.
- No hosts-file, DNS or proxy blocking of Spotify ad domains was present.
- The app has no ad-blocking code, extension, CSS or JS injection.

**Ad playback: PASS.** Ads are served and played untouched.

### 6.5 Background playback

SMTC position is not updated continuously while playing (it often reports `0.0`), but it updates on pause. Background playback was therefore measured by pausing with a media key and reading the position:

| State | Position before | Position after 30 s | Delta | Verdict |
|---|---|---|---|---|
| Minimized | 94.5 s | 126.5 s | **+32.0 s** | PASS |
| Hidden (tray-equivalent `SW_HIDE`) | 126.5 s | 158.5 s | **+32.0 s** | PASS |

The hide used `ShowWindow(SW_HIDE)`, the same OS call Tauri’s `window.hide()` performs; the window reported `visible=False` for the whole interval. Playback continued in both states.

**Background playback: PASS** (minimized and hidden).

### 6.6 Memory and CPU

All samples are over the full WebView2 process tree. “Private WS” is the lower bound; total WS double-counts shared pages.

| State | Processes | Total WS | Private WS | Private commit | CPU (% of one core) |
|---|---|---|---|---|---|
| Landing page (not shell idle) | 9 | 782.7 MB | not sampled | 524.0 MB | 8.91% (page animations) |
| Player loaded, paused | 10 | 1102.7 MB | **567.1 MB** | 798.3 MB | **0.16%** (10 s sample) |
| Playing | 10 | 1105.8 MB | **564.3 MB** | 743.1 MB | **6.46%** |
| Hidden, paused, 5 min | 10 | not sampled | **343.9 MB** | **787.8 MB** | **0.00%** |

The 5-minute hidden row is the post-trim state after the window had been hidden for five minutes: the page itself fell to 343.9 MB private working set and 535.9 MB PSS, while commit stayed at 787.8 MB. Trimming lowered residency, not committed memory.

Process breakdown while playing: renderer ~465 MB WS, browser ~150 MB, GPU ~164 MB, utilities ~200 MB, Rust shell ~31 MB.

Budget comparison:

| Budget | Target | Measured | Verdict |
|---|---|---|---|
| Shell idle, no Spotify page | ≤120 MB PSS | _pending_ | _pending_ |
| Player loaded, paused | ≤300 MB PSS | ≥567 MB private WS | **FAIL** (~1.9×) |
| Player playing | ≤450 MB PSS | ≥564 MB private WS | **FAIL** (~1.25×) |
| CPU paused, hidden | ≤2% of one core | 0.16% (10 s) | **PASS** |
| CPU playing | record only | 6.46% | recorded |
| Shell window | ≤500 ms | **401 ms** | **PASS** |

Because PSS is at least the private working set, the paused and playing results exceed the budgets even on the most favourable metric. The report’s earlier 219 MB “page” figure was the renderer content, not the full WebView2 process tree.

### 6.7 Startup

`window_created_ms=401` from `main()` entry to window creation. Budget ≤500 ms: **PASS**.

### 6.8 Security observations

- No capabilities directory; no command is granted to `open.spotify.com`.
- `withGlobalTauri=false`; `window.__TAURI__` is `undefined`.
- Navigation allowlist active; **0 blocked navigations** during the whole run (Spotify stayed on `open.spotify.com` / `accounts.spotify.com`).
- Social-login popups would be blocked by the strict top-level allowlist; email/password login worked. This should be documented for users.
- The diagnostic probe used a fixed `eval` from Rust and a cancelled navigation to return data; it did not modify page content and is not part of the proposed product.

## 7. Verdict

**GO on DRM, playback, ads and background playback. FAIL on the revised memory budgets as written.**

- Widevine works in WebView2 in practice on this machine, and the free web player plays with ads.
- Playback continues when minimized and when hidden.
- Installer and startup budgets pass easily.
- Paused and playing memory are ~1.9× and ~1.25× over the revised budgets even using private working set as a lower bound.
- CPU paused is well inside budget; playing CPU is recorded only.

The memory overrun is an engine/page cost, not a Rust-shell cost: the shell is ~31 MB working set. No Tauri-side optimization can bring a WebView2 Spotify session under 300/450 MB. The decision is therefore: accept higher memory budgets (~600 MB paused/playing), or stop. The original <150 MB target is not achievable.

## 8. Limitations

- One machine, one Windows build, one WebView2 runtime (158.0.4370.33), one free account, one network.
- SMTC timeline is unreliable while playing; background checks used pause-position deltas.
- Ad cadence was ~28 minutes; a single ad was observed.
- PSS was not measured directly; private working set is a lower bound and total working set an upper bound.
- The empty-shell baseline and 5-minute hidden CPU average were measured in Phase 4; see section 9.

## 9. Phase 4 addendum (2026-11-25)

Phase 4 re-measured the spike with direct PSS (`QueryWorkingSet`) and compared it with minimal shells.

- Phase 3's “private working set” was a pre-trim peak, not PSS, and PSS is higher. Corrected PSS for the spike: **558.0 MB paused visible**, **594.8 MB playing visible**, **603.0 MB playing hidden**, **535.9 MB hidden-paused after 5 minutes**; private commit stayed between 595 and 788 MB. The 5-minute trim lowered residency, not committed memory.
- A bare wry 0.57.4 + WebView2 shell measured **121.4 MB PSS** (282 ms to window), and a minimal Tauri 2 shell **117.7 MB PSS**. The Rust framework is not the memory cost; the Spotify page and Chromium are.
- WebView2's `MemoryUsageTargetLevel.Low` (exposed by wry as `set_memory_usage_level`) changed the picture in short samples: **34.9–52.8 MB PSS paused** and **225.5 MB PSS playing**, with the playhead advancing (3:50 → 4:21). The earlier “Low breaks playback” observation was a mis-click on the seek bar.
- The 35-minute soak, Edge/Edge PWA baselines, and the full ranked comparison are in `docs/PERFORMANCE_RESEARCH.md`.

The DRM, free-account playback, ad, background-playback, startup and installer results above are unchanged.
