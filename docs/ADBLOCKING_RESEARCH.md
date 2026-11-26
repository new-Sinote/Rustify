# Adblocking research and implementation plan

**Date:** 2026-11-26  
**Branch:** `feature/adblocking`  
**Status:** Research complete. First proof of concept implemented; simulated title tests and a short performance smoke test pass. Real-account ad-cycle, sign-in, DRM, and background-playback tests are still pending.  
**Owner decision:** [ADR-0001](ADR-0001-adblocking-direction.md) makes adblocking a first-class requirement.

## 1. Constraints

- Keep the Rust + `tao` + `wry` + WebView2 architecture.
- Preserve Spotify sign-in, normal playback, Widevine playback, and background playback.
- Do not circumvent DRM, extract audio, re-stream audio, extract credentials or tokens, or use undocumented Spotify APIs.
- Do not copy Blockify code. The current Blockify extension is PolyForm Noncommercial 1.0.0; the older Python Blockify is MIT but is a Linux DBus/PulseAudio tool and is not a suitable code source for this shell.
- Do not modify the published v0.2.0 release or its certification.
- Use temporary profile copies for testing.

### 1.1 Requirements that blocked adblocking

These were the active constraints before 2026-11-26. They are listed here so the change is explicit and cannot be silently reverted.

| Location | Old requirement | Current status |
|---|---|---|
| `README.md` | The official player keeps ads; ads are untouched | Historical for the stable 0.2.0 build only; adblocking builds deliberately change ad behaviour |
| `CONTRIBUTING.md` | Ad blocking, skipping, muting, and request filtering were not accepted | Replaced by the adblocking scope and fail-open requirements |
| `docs/PHASE-0-2-REPORT.md` | Ads-intact acceptance criteria and allowlist | Historical; must not be used to reject adblocking work |
| `docs/PHASE-3-SPIKE.md` | Ad playback was a PASS condition | Historical baseline, not a current requirement |
| `docs/CERTIFICATION_0.2.0.md` | Certification scope excluded ad changes | Valid for v0.2.0 only; invalid for any adblocking build |
| `docs/PERFORMANCE_RESEARCH.md` Appendix A | Ad blocking was rejected | Historical evaluation; risks remain but are not a prohibition |
| `docs/UBOL_INTEGRATION_RESEARCH.md` | uBOL was only an experimental exception | uBOL is now a candidate filtering layer on the adblocking branch |
| `docs/releases/*.md` | Ads-untouched release claims | Historical release records; banners added |

No other current requirement in the repository blocks adblocking. Spotify's own terms still prohibit ad circumvention; the owner accepted that product and account risk on 2026-11-26.

## 2. How Spotify serves ads

There are two different ad surfaces:

| Surface | How it is delivered | What can affect it |
|---|---|---|
| Display, banners, upsells, trackers | Page requests, scripts, images, XHR/fetch | Extension DNR rules, WebView2 request filtering, cosmetic filters |
| Audio ads between tracks | Spotify's playback session and audio CDN; current web player fetches audio from `spotifycdn.com` | Network filtering is brittle; page/player logic or a mute fallback is more reliable |

The uBOL research already showed the core limitation: its Spotify-specific media rule targets `akamaized.net/audio/`, while the tested player fetched audio from `audio-fa-quic.spotifycdn.com` over Fetch. The rule did not fire. Blocking the shared audio CDN would also risk blocking music. Server-side ad insertion can make network-level audio blocking ineffective.

This means a realistic first implementation should not promise "all audio ads blocked". It should combine:

1. Request filtering for display/tracker requests.
2. Native ad detection for audio ad breaks.
3. A fail-open mute fallback when an audio ad cannot be removed safely.

## 3. Options evaluated

### 3.1 Official uBlock Origin Lite integration

**What it is:** A pinned, unmodified uBOL Chromium build loaded as an unpacked MV3 extension through WebView2's `AddBrowserExtension`.

**Evidence from the existing research:**

- Installed successfully; service worker started; six DNR rulesets enabled.
- Telemetry and tracker requests were blocked.
- Spotify stayed signed in and playback worked in the short sample.
- The Spotify audio rule did not match the current Fetch-based audio path.
- WebView2 has no extension popup, badge, or dashboard host.
- The payload is 31.7 MB unpacked; install took roughly 0.3-0.9 s and did not block window creation.
- uBOL is GPL-3.0. Distribution requires the licence and corresponding source.

**Assessment:** Strong primary layer for display and tracker filtering. Not sufficient by itself for audio ads. GPL obligations are manageable but must be deliberate. It is already implemented behind the `ublock` feature and shipped as the 0.3.0 experiment.

### 3.2 Blockify's general approach

There are two unrelated projects with this name:

- **Modern Blockify extension:** Manifest V3, packaged DNR rules plus filter-derived rulesets, targeted content scripts for Spotify/YouTube/Twitch/Hulu, and an automatic mute fallback when an audio interruption cannot be removed. Its public source snapshot is **PolyForm Noncommercial 1.0.0**, which is source-available but not OSI open source. Commercial use is not granted. We will not copy it.
- **Older Python Blockify:** MIT, Linux-only, uses DBus to read Spotify metadata and PulseAudio to mute commercials. It is a useful confirmation that metadata-driven mute is a real approach, but its code is not applicable to a Windows WebView2 shell.

**General approach we can independently implement:** detect an ad from a documented platform signal, mute the WebView2 audio, and unmute when the ad ends. This is a fallback, not network blocking. It preserves the audio/DRM path and is low overhead.

### 3.3 Native Rust filtering

**Candidate:** `adblock-rust` (Brave, MPL-2.0). It supports EasyList/uBlock syntax, network rules, cosmetic rules, hosts syntax, and resource replacement. It is the engine behind Brave's native adblocker.

**Integration path:** Run the engine in the Rust host, feed it request URLs/resource types from WebView2 `WebResourceRequested`, and cancel or redirect matching requests. This avoids GPL obligations and gives full control over rules and logging.

**Limitations:**

- WebView2 request interception can block or redirect requests, but it is not a full browser network stack. Response-body modification is not available through the basic `WebResourceRequested` event.
- Spotify audio and music share delivery infrastructure. A rule that blocks an ad may block a track, and a rule that is too narrow may miss ads.
- Cosmetic filtering still requires injecting CSS into the page, which is a disclosed page modification.
- Filter-list distribution and update policy need their own licensing and integrity work.
- The engine and rule set add binary size, memory, and CPU. These must be measured.

**Assessment:** A good second layer and a likely long-term replacement for the uBOL dependency, but too large and risky for the first PoC.

### 3.4 WebView2 request interception

**API:** `ICoreWebView2::AddWebResourceRequestedFilter` plus the `WebResourceRequested` event. The event args can expose the request and allow the host to set a response (block/redirect). `WebResourceResponseReceived` is read-only for inspection.

**Limitations:**

- `wry` does not expose a high-level request-interception API; this needs raw `webview2-com` COM code.
- The host must handle all matching requests, including service-worker/fetch traffic, and must not stall the UI thread.
- It cannot inspect or rewrite response bodies in the basic event.
- Blocking a request can trigger Spotify error/retry loops or break playback.
- It is a broad capability that needs careful allowlisting and fail-open behavior.

**Assessment:** Worth a dedicated spike after the mute PoC, especially for display/tracker filtering with `adblock-rust`. Not the first choice for audio ads.

### 3.5 Spotify-specific ad detection

| Signal | Availability | Reliability | Privacy |
|---|---|---|---|
| WebView2 document title (`Spotify - Advertisement`) | Documented `DocumentTitleChanged` event; observed in the 0.2.0 soak snapshots | Good for current web player; localized titles may differ | Only the title string; do not log track titles |
| SMTC metadata (`Advertisement`) | Documented Windows `GlobalSystemMediaTransportControlsSessionManager`; used in the Phase 3 spike | Good when the page registers a session; polling and session selection add complexity | Metadata can include track/artist; must not persist it |
| DOM markers | Page DOM; requires injection | Brittle, undocumented selectors | Page access; more invasive |
| Network URL patterns | WebView2 request events | Brittle; ads share CDN with music | URLs can contain identifiers |
| Audio session/volume | Windows audio APIs | Can mute the app, but not identify ads by itself | Process-level only |

**Chosen first signal:** WebView2 document title. It is documented, requires no page injection, is already observable during ad breaks, and is sufficient to prove the mute/unmute lifecycle. SMTC can be added later as a second signal if the title proves unreliable.

## 4. Chosen first proof of concept

**Name:** `adblock` feature (experimental, not default).

**Behavior:**

1. Subscribe to `DocumentTitleChanged`.
2. Treat a title that equals `Advertisement` or ends with `- Advertisement` / en-dash `Advertisement` as an ad break.
3. Mute the player WebView through the documented `ICoreWebView2_8::SetIsMuted(true)` API.
4. Unmute when the title changes away from the ad marker.
5. Reset and unmute on page load.
6. Add a 120-second watchdog that unmutes if the title never changes, so a detection bug cannot leave the app silent.
7. Log only ad-state transitions, never track titles.

**Why this is the smallest legal PoC:**

- No third-party code is copied.
- No network requests are blocked, so DRM and audio delivery stay untouched.
- No page JavaScript is injected for ad detection.
- It directly addresses the audio-ad surface that uBOL could not prove it blocks.
- It is small enough to measure and easy to remove if it fails.

**What it does not do:**

- It does not block ad delivery. The ad still plays silently.
- It does not guarantee detection if Spotify changes its title or localization.
- It does not replace uBOL or native filtering for display/tracker ads.
- It is not certified and must not be presented as a stable feature.

## 5. Test plan

### Functional

- Free-account sign-in and playback.
- Normal track playback with no false mute.
- Ad break detection, mute, and unmute.
- Pause/resume during an ad and after an ad.
- Track skip during an ad.
- Page reload during an ad.
- Window hidden/minimized during an ad.
- Watchdog recovery after a simulated stuck title.

### Performance

- Window-ready time.
- Memory (PSS) and CPU with `adblock` off vs on, paused and playing.
- Startup and page-load time.
- No new persistent process.

### Security and policy

- No credential/token access.
- No DRM licence or audio interception.
- No page injection for ad detection.
- No track titles in logs.
- No changes to the 0.2.0 release artifacts.

## 6. Certification impact

### 6.1 Invalidated or not applicable

| Certification | Status for adblocking builds |
|---|---|
| `CERTIFICATION_0.2.0.md` | **Invalid.** It covers the published 0.2.0 build and the old ads-untouched scope only. |
| `v0.3.0-pre.1` uBOL pre-release | **Never certified.** It remains an earlier experiment. |
| Any stable claim based on 0.2.0 memory, CPU, startup, or installer results | **Not applicable.** Adblocking builds add a feature and may change network behaviour. |

### 6.2 Certification that must be repeated

Any build with `adblock` or `ublock` enabled must be treated as uncertified until a new report covers:

- free-account sign-in and playback;
- Widevine/DRM playback;
- background playback;
- ad detection and mute/unmute across a full ad cycle;
- false positives, long ads, and stuck-mute recovery;
- memory/CPU overhead against a control build;
- installer/uninstaller behaviour and licence notices.

## 7. Proof-of-concept results

### 7.1 Build and unit checks

- `cargo check` (default features): pass.
- `cargo check --features adblock`: pass.
- `cargo check --features "adblock ublock"`: pass.
- `cargo test --features adblock`: 3 title-parser tests pass.

The default build has no adblocking code compiled in. The feature is opt-in and is not part of the installer.

### 7.2 Simulated advertisement-title test

The functional test used a temporary profile and the `bench` + `adblock` build. It loaded `https://open.spotify.com/` without signing in and changed `document.title` through the local CDP port. This tests the WebView2 event, mute, unmute, page-load reset, and watchdog path; it is **not** a real Spotify ad test.

Observed log transitions:

```text
[  3144 ms] adblock=page_load unmuted
[ 20265 ms] adblock=ad_start muted
[ 21474 ms] adblock=ad_end unmuted
[ 22686 ms] adblock=ad_start muted
[142686 ms] adblock=watchdog_unmute
```

The final watchdog unmuted while the title still read `Advertisement`, which is the intended fail-open behaviour. The app remained responsive and the process tree stayed alive. No track title was written to the log.

After the unmute-retry hardening, a second run reproduced `ad_start` at 3664 ms and `watchdog_unmute` at 123664 ms with the title held at `Advertisement`. A separate ad-to-track run reproduced `ad_start` at 4150 ms and `ad_end` at 6276 ms.

### 7.3 Short performance smoke test

Release `bench` builds were measured with `bench/measure/tree-metrics.ps1` for 15 seconds after the page settled. Each run used a fresh temporary profile and the signed-out landing page.

| Build and state | Processes | PSS | CPU, one core |
|---|---:|---:|---:|
| Control, idle | 10 | 505.0 MB | 63.9% |
| `adblock`, idle | 10 | 505.0 MB | 57.2% |
| `adblock`, simulated ad title | 10 | 476.8 MB | 113.2% |
| Control, simulated ad title | 10 | 481.4 MB | 115.0% |

The CPU rise after changing the document title appears in the control build too, so it is page/rendering activity, not attributable to the mute feature. PSS differences are within run-to-run noise. The feature adds no process. These numbers are a smoke test only; they do not replace the 0.2.0 certification method or a real playback soak.

### 7.4 Not yet tested

- Real free-account sign-in and a real audio-ad cycle.
- Widevine/DRM playback and background playback with `adblock` enabled.
- Pause, resume, skip, reload, and hide/show during a real ad.
- Non-English player titles.
- Long ads that exceed the 120-second watchdog.
- uBOL plus `adblock` together on a real account.

## 8. Sources

- uBOL integration research: [UBOL_INTEGRATION_RESEARCH.md](UBOL_INTEGRATION_RESEARCH.md)
- WebView2 audio API: <https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2_8>
- WebView2 document title event: <https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2>
- WebView2 request interception: <https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/webresourcerequested>
- Brave `adblock-rust`: <https://github.com/brave/adblock-rust> (MPL-2.0)
- Blockify extension source snapshot: <https://github.com/dhanur2/spotify-ad-blocker-extension-blockify> (PolyForm Noncommercial 1.0.0)
- Older Python Blockify: <https://github.com/serialoverflow/blockify> (MIT, Linux DBus/PulseAudio)
