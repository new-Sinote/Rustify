# Adblocking research and implementation plan

**Date:** 2026-11-26  
**Branch:** `feature/adblocking`  
**Status:** Research complete. The experimental mute proof of concept and the opt-in Automatic Ad Discovery mode are implemented. Synthetic lifecycle, consecutive-ad, max-skip, watchdog, title-match, ambiguous-session, and UI tests pass. Published as an uncertified pre-release (`v0.3.0-adblock-pre.1`). Real-account ad-cycle, sign-in, DRM, background-playback, and discovery-on-real-tracks tests are **BLOCKED** pending an approved temporary Spotify Free account. Zero real advertisements have been detected. The build is not certified.  
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
2. Classify the title before acting:
   - **Ad** when the whole title is `Advertisement`, or the final segment after a separator (`-`, en/em dash, `•`, `·`, `|`) is `Advertisement`.
   - **Track** when at least two non-empty segments are present and the title is not player chrome or an ad-like unknown.
   - **Unknown** for empty titles, player chrome (`Spotify`, `Spotify - Web Player...`, sign-in pages), and any title that mentions an advertisement but does not match the confirmed parser. Unknown is never treated as a track.
3. Mute the player WebView through the documented `ICoreWebView2_8::SetIsMuted(true)` API when an ad is detected.
4. Unmute when the title changes away from the ad marker.
5. Reset and unmute on page load; re-detect and re-mute if the loaded page is still an ad.
6. Use a bounded watchdog instead of an unconditional 120-second unmute:
   - first recheck at 120 seconds;
   - if the title still reads as an ad, recheck every 30 seconds;
   - at 15 minutes of continuous mute, fail open: unmute, suppress remute for the same ad until the page reloads or the user starts discovery again, and stop discovery with `watchdog fail-open`;
   - retry a failed unmute every 5 seconds.
7. Add an **opt-in** Automatic Ad Discovery test mode. It is off by default, never starts automatically in a release build, and is controlled from the `AD` panel:
   - skips only confirmed normal tracks while audio is playing;
   - one skip in flight at a time, at least the configured interval apart (2-60 s, default 5 s);
   - stops skipping immediately when an ad is detected, an unknown title appears, or playback pauses; an unknown title fails after 30 seconds;
   - stops with an error when a skip is rejected or unavailable, or the configured maximum (1-500, default 50) is reached;
   - cancels a pending skip on pause or page load;
   - stops with an error after 30 seconds of unknown title;
   - never bypasses Spotify's skip limit; a disabled Next stops discovery.
8. Perform test-mode skips through the documented Windows media-session API (`GlobalSystemMediaTransportControlsSessionManager` and `TrySkipNextAsync`) instead of injecting JavaScript into Spotify.
   - The `adblock` build passes `--enable-features=MediaSessionService` to WebView2 so the player publishes a Windows media session. This is a disclosed, build-scoped change.
   - Session selection fails closed: exactly one media session, a WebView2 AUMID, `Playing` status, Next enabled, and a normalized media-title match against the player document title. A mismatch, ambiguity, or missing session stops discovery without a skip.
9. Log only ad-state transitions, discovery state, counts, failure reasons, and (debug only) the AUMID and a boolean title-match result. Never log track titles.

**Why this is the smallest legal PoC:**

- No third-party code is copied.
- No network requests are blocked, so DRM and audio delivery stay untouched.
- No page JavaScript is injected into Spotify for ad detection.
- It directly addresses the audio-ad surface that uBOL could not prove it blocks.
- It is small enough to measure and easy to remove if it fails.

**What it does not do:**

- It does not block ad delivery. The ad still plays silently when detection works.
- It does not guarantee detection if Spotify changes its title, localizes it, or publishes a different media-session title.
- It does not handle non-English ad markers; they remain `Unknown` and are not muted.
- It does not replace uBOL or native filtering for display/tracker ads.
- It does not bypass Spotify skip restrictions.
- It is not certified and must not be presented as a stable feature.

## 5. Test plan

### Functional

The full procedure is in [ADBLOCKING_REAL_ACCOUNT_TEST.md](ADBLOCKING_REAL_ACCOUNT_TEST.md). It requires an approved temporary Spotify Free account and a fresh/copied profile.

- Free-account sign-in and playback.
- Normal track playback with no false mute.
- Ad break detection, mute, and unmute.
- Consecutive ads in one break.
- Long ad and bounded watchdog fail-open.
- Non-English or alternate ad titles.
- Pause/resume during an ad and after an ad.
- Track skip during an ad.
- Page reload during an ad.
- Window hidden/minimized during an ad.
- Sign-out/sign-in recovery.
- Discovery start/stop, interval, max skips, pause, reload, unknown title, and rejected skip.
- `adblock` and `adblock ublock` builds on the same account.

### Performance

- Window-ready time.
- Memory (PSS) and CPU with `adblock` off vs on, paused and playing.
- Idle, music, ad-muted, background, discovery, and repeated transitions.
- 60-second idle with memory trim enabled.
- Startup and page-load time.
- No new persistent process.

### Security and policy

- No credential/token access.
- No DRM licence or audio interception.
- No page injection into Spotify for ad detection.
- No track titles in logs.
- No changes to the 0.2.0 release artifacts.
- `MediaSessionService` disclosed and scoped to adblock builds.

## 6. Certification impact

### 6.1 Invalidated or not applicable

| Certification | Status for adblocking builds |
|---|---|
| `CERTIFICATION_0.2.0.md` | **Invalid.** It covers the published 0.2.0 build and the old ads-untouched scope only. |
| `v0.3.0-pre.1` uBOL pre-release | **Never certified.** It remains an earlier experiment. |
| `v0.3.0-adblock-pre.1` | **Uncertified pre-release.** Portable `rustify-adblock.exe`, synthetic tests only, zero real ads. |
| Any stable claim based on 0.2.0 memory, CPU, startup, or installer results | **Not applicable.** Adblocking builds add a feature and may change network behaviour. |

### 6.2 Certification that must be repeated

Any build with `adblock` or `ublock` enabled must be treated as uncertified until a new report covers:

- free-account sign-in and playback;
- Widevine/DRM playback;
- background playback;
- ad detection and mute/unmute across a full ad cycle;
- consecutive, long, and non-English ads;
- false positives, long ads, and stuck-mute recovery;
- discovery on real tracks, including pause, reload, unknown titles, and Spotify's skip limit;
- the `MediaSessionService` media-session path and its effect on media keys;
- memory/CPU overhead against a control build;
- installer/uninstaller behaviour and licence notices.

## 7. Proof-of-concept results

### 7.1 Build and unit checks

- `cargo check` (default features): pass.
- `cargo check --features adblock`: pass.
- `cargo check --features "adblock ublock"`: pass.
- `cargo test --features adblock`: 15 tests pass, 0 fail.
- `cargo clippy -- -D warnings`: pass.
- `cargo clippy --features "adblock ublock" -- -D warnings`: pass.
- Release builds for default, `adblock`, and `adblock ublock`: pass.

The default build has no adblocking code compiled in. The feature is opt-in and is not part of the installer. The pre-release is a portable executable, not the installer. The only dependency change is the `windows/Media_Control` feature; no new crate was added.

### 7.2 Synthetic functional tests

A local page with Web Audio and a synthetic media session was used with the `bench` + `adblock` build and a fresh temporary profile. The page's `setTrack` changes both the media-session metadata and the document title without injecting JavaScript into Spotify. This is **not** a real Spotify ad test.

| Test | Result | Evidence |
|---|---|---|
| SMTC skip and title match | **PASS** | 3 accepted skips, then `max skips reached`; `matched=true` |
| Lifecycle: skip, ad, mute, unmute, pause, resume, reload | **PASS** | 8 accepted skips, 1 ad, 1 cycle; no skip while paused or during the ad |
| Consecutive ads and repeated cycles | **PASS** | 5 accepted skips, 4 ads, 4 cycles; one cycle per break |
| Bounded watchdog | **PASS** | extensions at ~3/5/7 s, fail-open at ~9 s after mute, no remute |
| Max skips | **PASS** | 3 skips, state `Error`, `max skips reached` |
| Title mismatch | **PASS** | 0 skips, state `Error`, `media session title does not match the player` |
| Ambiguous sessions | **PASS** | 0 skips, state `Error`, `ambiguous media sessions: 2` |
| Start/Stop UI | **PASS** | 2 skips, then no further skips after Stop |

Observed watchdog log with shortened bench-only durations:

```text
adblock=ad_start muted
adblock=watchdog_extend muted_ms=3009
adblock=watchdog_extend muted_ms=5020
adblock=watchdog_extend muted_ms=7030
adblock=watchdog_fail_open unmuted
discovery=error reason=watchdog fail-open
```

The fail-closed paths did not issue a skip. The ambiguity run used a second WebView2 host with its own media session; Rustify saw `total=2 webview=2` and stopped. The mismatch run saw `matched=false` and stopped before calling `TrySkipNextAsync`.

### 7.3 Performance smoke test

Release `bench` builds were measured with `bench/measure/tree-metrics.ps1` on a fresh temporary profile. Fifteen-second samples after the page settled; the transition run was 30 seconds; the idle check was 60 seconds with memory trim enabled.

| Build and state | Processes | PSS | CPU, one core |
|---|---:|---:|---:|
| Control, idle | 8 | 134.8 MB | 0.53% |
| `adblock`, idle | 8 | 137.7 MB | 0.51% |
| Control, music | 9 | 171.0 MB | 2.52% |
| `adblock`, music | 9 | 173.9 MB | 2.42% |
| Control, ad title | 9 | 174.4 MB | 1.30% |
| `adblock`, ad muted | 9 | 173.3 MB | 1.21% |
| Control, background music | 9 | 173.7 MB | 1.41% |
| `adblock`, background music | 9 | 171.9-172.3 MB | 1.20-2.62% |
| `adblock`, discovery, 2 s interval | 9 | 173.8 MB | 3.13% |
| `adblock`, repeated transitions, 30 s | 9 | 178.9 MB | 2.82% |

The transition run performed 5 accepted skips, 4 ad breaks, and 4 cycles in about 31 seconds. It is a stress test, not a real listening pattern.

A 60-second idle sample with trimming enabled measured 28.1 MB PSS / 0.29% for control and 32.5 MB PSS / 0.31% for `adblock`. The 15-second PSS is higher because the hydrate trim has not run yet.

CPU samples are noisy on this machine. The repeatable observations are: no new persistent process; PSS within about 3 MB of control in music and ad states; discovery adds 0-5 MB PSS and roughly 1-3% of one core for its timer and UI updates. These are smoke numbers, not certification.

An earlier 60-second `--no-trim` run showed about 64% of one core in both builds. The cause was a bench-harness busy-wait on an unconsumed policy deadline, not adblocking. A related discovery bug also kept an unknown-title deadline armed while discovery was off. Both were fixed during validation; the 60-second idle CPU returned to 0.2-0.8% and the transition run measured 2.82%.

### 7.4 Real-account status

**BLOCKED.** No approved temporary Spotify Free account was available. Zero real advertisements have been detected, and no real ad, DRM, background-playback, sign-in, or skip-limit case has been run. The `v0.3.0-adblock-pre.1` pre-release was published from this state; publishing does not change the blocked status. The full procedure and the blocked-case table are in [ADBLOCKING_REAL_ACCOUNT_TEST.md](ADBLOCKING_REAL_ACCOUNT_TEST.md) and [ADBLOCKING_VALIDATION_REPORT.md](ADBLOCKING_VALIDATION_REPORT.md).

### 7.5 Not yet tested

- Real free-account sign-in and a real audio-ad cycle.
- Consecutive, long, and non-English real ads.
- Widevine/DRM playback and background playback with `adblock` enabled.
- Pause, resume, skip, reload, and hide/show during a real ad.
- Sign-out/sign-in recovery.
- Discovery on real tracks and Spotify's real skip limit.
- `MediaSessionService` side effects on media keys and background playback.
- uBOL plus `adblock` together on a real account.

## 8. Sources

- uBOL integration research: [UBOL_INTEGRATION_RESEARCH.md](UBOL_INTEGRATION_RESEARCH.md)
- WebView2 audio API: <https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2_8>
- WebView2 document title event: <https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2>
- WebView2 request interception: <https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/webresourcerequested>
- WebView2 additional browser arguments: <https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2environmentoptions>
- Windows media session control: <https://learn.microsoft.com/en-us/uwp/api/windows.media.control>
- `TrySkipNextAsync`: <https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssession.tryskipnextasync>
- Brave `adblock-rust`: <https://github.com/brave/adblock-rust> (MPL-2.0)
- Blockify extension source snapshot: <https://github.com/dhanur2/spotify-ad-blocker-extension-blockify> (PolyForm Noncommercial 1.0.0)
- Older Python Blockify: <https://github.com/serialoverflow/blockify> (MIT, Linux DBus/PulseAudio)
