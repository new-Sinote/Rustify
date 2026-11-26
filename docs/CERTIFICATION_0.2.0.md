# Rustify 0.2.0 - Certification

> **Historical certification - 0.2.0 only.** This report certifies the published v0.2.0-pre.1 build under the old "ads untouched" contract. The owner changed the product direction on 2026-11-26: adblocking is now a first-class requirement. This report does **not** certify any adblocking build. A new certification is required before an adblocking build can be called stable. See [ADR-0001](ADR-0001-adblocking-direction.md) and [ADBLOCKING_RESEARCH.md](ADBLOCKING_RESEARCH.md).

**Build:** Rustify 0.2.0 (`wry` 0.57.4 + `tao` 0.37.3, WebView2 Evergreen)  
**Date:** 2026-11-26  
**Machine:** Windows 11 x64 (build 26300), 16 logical cores, 2560x1440  
**WebView2 Runtime:** 158.0.4370.33  
**Verdict:** PASS. All release gates are met on the tested Windows 11 x64 machine.

This document records the release gate for the shipping shell in [`app/`](../app/). It is the follow-up to the Phase 4 research in [PERFORMANCE_RESEARCH.md](PERFORMANCE_RESEARCH.md).

## 1. What was measured

The measured binary was the shipping shell built with the `bench` feature, which only adds the local CDP port and development tools used by the harness. The shipping binary contains the same shell, policy, page, browser arguments, and window code; it has no CDP port and no development tools. The WebView2 profile was a signed-in account that receives Spotify's normal ad breaks. Track titles are deliberately omitted from this report.

The test ran the official `https://open.spotify.com/` page. No Spotify API, network response, DRM path, audio path, or ad path was changed. The only page modification was the documented cosmetic stylesheet that hides the two nav-bar upsell elements. See [Scope](#7-scope).

## 2. Policy under test

- **Hydration trim:** 30 s after the page-load-finished event, set `Normal`, then `Low` 300 ms later.
- **Pause trim:** 25 s after `IsDocumentPlayingAudioChanged` reports that audio stopped.
- **Periodic trim:** every 120 s, including while paused and hidden.
- **Post-gap trim:** 15 s after audio resumes within 20 s of a pause (track and ad boundaries). The certification run passed `--gap-sec 10` to exercise boundaries more aggressively than the shipping default; this can only trim sooner, never later.
- Both the player WebView and the 40 px chrome WebView receive the same level changes.

`Low` is a one-shot trim request, so the shell deliberately returns to `Normal` and re-applies `Low` instead of assuming the level is still active.

## 3. Playing soak

Method: 35 minutes of continuous playback with the window visible at 1280x800 (1294x814 outer) and kept topmost so Windows did not occlude it and release GPU memory. The soak started from the hydrated state, about four minutes after process launch and after the 30 s hydration trim, matching the Phase 4 soak method. Every five minutes the harness sampled the full process tree for 10 s (PSS, working set, private commit, CPU), read the JS heap and DOM counters over CDP, and polled SMTC every 12 s. The threshold is **every sample <= 400 MB PSS**.

| sample | elapsed | PSS (MB) | Private WS (MB) | CPU (% one core) | SMTC | Gate |
|---|---:|---:|---:|---:|---|---|
| soak_00m | 0:00 | 243.6 | 204.9 | 10.56 | Playing | PASS |
| soak_05m | 5:00 | 263.3 | 222.1 | 5.90 | Playing | PASS |
| soak_10m | 10:00 | 221.6 | 186.2 | 5.24 | Playing | PASS |
| soak_15m | 15:00 | 216.2 | 179.9 | 5.68 | Playing | PASS |
| soak_20m | 20:00 | 215.9 | 181.7 | 3.83 | Playing | PASS |
| soak_25m | 25:00 | 174.8 | 133.5 | 4.26 | Playing | PASS |
| soak_30m | 30:00 | 252.9 | 205.9 | 8.99 | Playing | PASS |

**Result:** max 263.3 MB, mean 226.9 MB PSS; every sample is more than 136 MB below the 400 MB limit. SMTC recorded 10 advertisement rows across the run, and playback continued normally at the end. No ad, playback, or DRM path was touched by the host; the only page change is the cosmetic nav-bar rule described in [Scope](#7-scope).

## 4. Paused and hidden

Method: pause from the player control, hide the window with the custom close button (the same path as close-to-tray), wait 75 s for the 25 s pause debounce and the trim to take effect, then sample once a minute for 10 minutes with the window hidden. The threshold is **every sample <= 300 MB PSS** and **paused CPU <= 1% of one core**.

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

**Result:** max 95.1 MB, mean 60.0 MB PSS; every sample is more than 200 MB below the 300 MB limit. The ten 10-second CPU windows averaged 0.32% of one core. One window reported 1.05%, at the resolution limit of a 10-second sample, so two 60-second steady-state windows were measured after the run: **0.24%** and **0.39%** of one core. The sustained figure meets both the paused (<= 1%) and idle (<= 0.5%) budgets; the single 10-second 1.05% reading is recorded here rather than hidden.

## 5. Startup and hydration

- **Window ready:** 272 ms from `main()` entry to the visible window on the shipping build (1280x800 client, warm profile). A separate launch with a brand-new app WebView2 profile measured **271 ms**, so the 500 ms budget is met for normal starts.
- An earlier run on a WebView2 runtime that had never been initialized measured 652 ms. That is a one-time Windows/WebView2 runtime cost; once the runtime has started normally, a fresh app profile does not reproduce it. It is recorded here rather than hidden.
- The player hydrates for roughly the first minute after launch. The hydration trim is applied 30 s after page load, and WebView2 reclaims the memory asynchronously. The 35-minute soak therefore starts from the hydrated state; the launch transient is not part of the playing-budget samples.
- The 40 px chrome WebView is a second small renderer. It remains at single-digit PSS after a trim and is included in every process-tree total.

## 6. Artifact

- **File:** `Rustify-0.2.0-x64-setup.exe`
- **Size:** 534,512 bytes
- **SHA-256:** `89b38eaf712806017bd73c5dd8da8cd1def864dcf25f2eea2376c4307a4e1d5c`
- **Signing:** unsigned, as in the v0.1.0-pre.1 spike. SmartScreen will warn; verify the SHA-256 before running.
- **Install test:** silent install, Start Menu and desktop shortcuts, `HKCU` uninstall entry, installed launch, and silent uninstall all passed. The silent uninstaller keeps the WebView2 profile and settings.

## 7. Scope

> **Historical scope.** The ads-untouched statements below describe v0.2.0 only. They are not current product requirements and do not apply to any adblocking build.

Rustify hosts the official web player. It does not block, skip, mute, filter, proxy, or re-stream advertisements; it does not circumvent DRM, extract audio, extract credentials or tokens, or call undocumented Spotify APIs. The only page change is a stylesheet that hides the `Explore Premium` button and the `Install App` link in the nav bar. Playback, ads, DRM, and network traffic are untouched.

## 8. Reproduction

The harness is in [`bench/measure/`](../bench/measure/):

```powershell
# Build the measured binary.
cargo build --release --features bench --manifest-path app\Cargo.toml

# Launch with the local CDP port and the shipping policy timings.
.\app\target\release\rustify.exe --cdp 9222

# Start playback, then sample the process tree.
.\bench\measure\soak.ps1 -RootId <pid> -Minutes 35 -Port 9222 -OutDir bench\soak-app-final

# Pause, hide to tray, wait for the pause trim, then:
.\bench\measure\hidden-soak.ps1 -RootId <pid> -Minutes 10 -OutDir bench\soak-hidden
```

The raw soak output contains local process data and Spotify metadata. It is kept out of the public repository by `.gitignore`; only the aggregated, title-free numbers are published here.
