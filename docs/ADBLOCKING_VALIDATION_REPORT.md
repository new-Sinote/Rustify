# Adblocking validation report

**Date:** 2026-11-26  
**Branch:** `feature/adblocking`  
**Base commit:** `83a7f3d`  
**App version:** 0.3.0 (uncertified experimental branch)  
**Pre-release:** [v0.3.0-adblock-pre.1](releases/v0.3.0-adblock-pre.1.md)  
**Status:** **Synthetic validation passed. Real-account validation is BLOCKED. Published as an uncertified pre-release. Not certified.**

This report records what was actually tested for the experimental audio-ad mute fallback and the opt-in Automatic Ad Discovery mode. It separates synthetic evidence from real Spotify evidence. No real advertisement has been observed, so no real-account pass is claimed.

## 1. Summary

| Area | Result |
|---|---|
| Build checks (default, `adblock`, `adblock ublock`) | **PASS** |
| Unit tests (`cargo test --features adblock`) | **PASS**, 15/15 |
| Clippy with `-D warnings` (default and combined features) | **PASS** |
| Release builds (default, `adblock`, `adblock ublock`) | **PASS** |
| Synthetic SMTC skip and title-match path | **PASS** |
| Synthetic ad lifecycle, pause/resume, reload | **PASS** |
| Synthetic consecutive ads and repeated cycles | **PASS** |
| Synthetic bounded watchdog and fail-open | **PASS** |
| Synthetic ambiguous-session fail-closed | **PASS** |
| Synthetic title-mismatch fail-closed | **PASS** |
| Synthetic max-skip stop | **PASS** |
| Synthetic Start/Stop UI and opt-in behavior | **PASS** |
| Real Spotify Free sign-in and playback | **BLOCKED** |
| Real ad detection, mute, unmute, cycle | **BLOCKED** |
| Real consecutive/long/non-English ads | **BLOCKED** |
| Real DRM, background playback, pause/resume/skip/reload during an ad | **BLOCKED** |
| Real advertisements detected | **0** |
| New certification for an adblocking build | **Not started** |
| Installer | **Not changed** |
| Pre-release | **Published as `v0.3.0-adblock-pre.1` (portable exe, uncertified)** |

The synthetic tests prove the state machine, WebView2 mute calls, Windows media-session selection, and fail-closed paths on a local test page. They do **not** prove that Spotify's real document title, media session, ad timing, or DRM path behaves the same way.

## 2. Environment

| Item | Value |
|---|---|
| OS | Windows 11 Home, 10.0.26300, build 26300 |
| CPU | Intel Core i5-14400F |
| Logical cores | 16 |
| Rust | 1.100.0 (`0ef1bc386`, 2026-11-09) |
| Cargo | 1.100.0 (`3acc79c0c`, 2026-11-05) |
| WebView2 Runtime | 158.0.4370.33 |
| App | Rustify 0.3.0, `feature/adblocking` |
| Account used | None. No Spotify account was signed in for this report. |
| Profiles used | Fresh temporary directories under `%TEMP%\opencode`; no daily profile, cookies, or tokens |
| Test page | Local `http://127.0.0.1` page with Web Audio and a synthetic media session |

## 3. Builds and static checks

All commands were run from `app`.

```powershell
cargo check
cargo check --features adblock
cargo check --features "adblock ublock"
cargo test --features adblock
cargo clippy --features "adblock ublock" -- -D warnings
cargo clippy -- -D warnings
cargo build --release
cargo build --release --features adblock
cargo build --release --features "adblock ublock"
```

Observed result: every command exited 0. The default build contains no adblocking code. The `adblock` feature adds the `windows/Media_Control` feature; no new crate was added.

Release binary sizes:

| Binary | Bytes |
|---|---:|
| Default release | 1,178,112 |
| `adblock` release | 1,233,920 |
| `adblock ublock` release | 1,242,112 |
| `bench` control | 1,186,816 |
| `bench adblock` | 1,244,160 |

Sizes are informational only; they are not a certification metric.

## 4. Unit tests

`cargo test --features adblock` ran 15 tests, all passing:

- `detects_plain_advertisement`
- `detects_advertisement_suffix`
- `ignores_track_titles`
- `classifies_titles`
- `smtc_title_match_normalizes_and_fails_closed`
- `discovery_start_unknown_times_out`
- `discovery_schedules_track_and_stops_on_ad`
- `discovery_resumes_after_ad`
- `discovery_does_not_skip_unknown_or_ad`
- `discovery_pause_cancels_pending_skip`
- `discovery_max_skips_stops`
- `discovery_rejected_skip_stops`
- `discovery_not_playing_waits_without_counting`
- `discovery_disabled_has_no_deadline`
- `discovery_counts_accepted_skip_after_title_change`

The tests cover the parser, the fail-closed title match, the discovery state machine, skip accounting, pause cancellation, max skips, rejected skips, unknown-title timeout, and the disabled-state deadline. They do not cover real WebView2/COM behavior; that is what the synthetic runs below add.

## 5. Synthetic functional tests

Method: a local page exposes a Web Audio element and a synthetic media session. The `bench adblock` build loads the page with a fresh profile. Test scripts drive the page through CDP and read the bar UI. The page's `setTrack` changes both the media-session metadata and the document title without JavaScript injection into Spotify. The page's own `setTitle` is used only for the watchdog case.

Each run used a unique server port, CDP port, and profile. Only one media session was active except in the ambiguity test.

| Test | What it proves | Result | Counts / evidence |
|---|---|---|---|
| SMTC skip | Session discovery, WebView2 AUMID match, title match, `TrySkipNextAsync`, accepted skip | **PASS** | 3 accepted skips, then `max skips reached` |
| Lifecycle | Skip, ad stop, mute, unmute, resume, pause, resume, reload | **PASS** | 8 accepted skips, 1 ad, 1 cycle |
| Cycles | Three ad breaks, consecutive ad titles, repeated mute/unmute | **PASS** | 5 accepted skips, 4 ads, 4 cycles |
| Watchdog | Bounded rechecks and fail-open | **PASS** | extensions at ~3/5/7 s, fail-open at ~9 s after mute |
| Max skips | Stop at the configured limit | **PASS** | 3 skips, state `Error`, `max skips reached` |
| Title mismatch | Fail closed if the media title does not match the player | **PASS** | 0 skips, state `Error`, `media session title does not match the player` |
| Ambiguous sessions | Fail closed if another media session exists | **PASS** | 0 skips, state `Error`, `ambiguous media sessions: 2` |
| Start/Stop UI | Opt-in start, counters, Stop, no skip after Stop | **PASS** | 2 skips, then no further skips after Stop |

### 5.1 Lifecycle evidence

Filtered log, synthetic page, 2-second interval, 20-skip limit:

```text
discovery=start interval=2 max_skips=20
discovery=page_load_reset
adblock=page_load unmuted
audio=playing
discovery=skip_requested
discovery=smtc_match aumid=msedgewebview2.exe
discovery=smtc_sessions total=1 webview=1
discovery=smtc_title_match matched=true
discovery=skip_accepted count=1
discovery=skip_confirmed
discovery=ad_detected count=1
adblock=ad_start muted
adblock=ad_end unmuted
discovery=skip_accepted count=2
...
audio=paused; trim scheduled after debounce
audio=playing
discovery=skip_accepted count=4
discovery=page_load_reset
adblock=page_load unmuted
discovery=skip_accepted count=6
```

Observed UI states: `Playing` -> `Muted` during the ad -> `Playing` after unmute -> `Waiting` while paused -> `Playing` after resume -> `Playing` after reload. No skip was issued during the ad or while paused.

### 5.2 Consecutive-ad evidence

The test produced four ad breaks in one run, including a break with two consecutive ad titles and no track in between. The ad count increased once per contiguous ad break, the audio stayed muted across the break, and the cycle count increased once when normal playback returned.

```text
discovery=ad_detected count=1
adblock=ad_start muted
adblock=ad_end unmuted
...
discovery=ad_detected count=2
adblock=ad_start muted
adblock=ad_end unmuted
...
discovery=ad_detected count=3
adblock=ad_start muted
discovery=skip_result_stale
adblock=ad_end unmuted
...
discovery=ad_detected count=4
adblock=ad_start muted
adblock=ad_end unmuted
```

The `skip_result_stale` lines are expected: an ad title cancels the pending skip, and the late SMTC result is ignored. No skip was accepted after the ad started.

### 5.3 Watchdog evidence

Shortened durations were used only in the `bench` build: `--adblock-watchdog-secs 3`, `--adblock-watchdog-extend-secs 2`, `--adblock-watchdog-max-secs 9`.

```text
discovery=ad_detected count=1
adblock=ad_start muted
adblock=watchdog_extend muted_ms=3009
adblock=watchdog_extend muted_ms=5020
adblock=watchdog_extend muted_ms=7030
adblock=watchdog_fail_open unmuted
discovery=error reason=watchdog fail-open
```

After fail-open, the title was held at the ad marker for another 4 seconds. The UI stayed `Error`, the audio stayed unmuted, and no remute occurred. When the title changed to a track, discovery remained stopped and no skip was issued.

### 5.4 Fail-closed evidence

Title mismatch:

```text
discovery=skip_requested
discovery=smtc_sessions total=1 webview=1
discovery=smtc_title_match matched=false
discovery=error reason=media session title does not match the player
```

Ambiguous sessions (Rustify plus a second WebView2 host playing the same page):

```text
discovery=skip_requested
discovery=smtc_match aumid=msedgewebview2.exe
discovery=smtc_match aumid=msedgewebview2.exe
discovery=smtc_sessions total=2 webview=2
discovery=error reason=ambiguous media sessions: 2
```

In both cases the page's `next` counter stayed at 0. No skip was issued.

### 5.5 UI evidence

The `AD` button opened the panel. Baseline: state `Off`, ON/OFF badge `OFF`, 0 skips, detail `Opt-in test mode. Never starts automatically.`, Start enabled, Stop disabled, no `discovery-on` class. After Start with a 2-second interval: state `Playing`, badge `ON`, 2 accepted skips, Start disabled, Stop enabled, `discovery-on` present. After Stop: state `Off`, badge `OFF`, detail `stopped by user`, Start enabled, Stop disabled. Four seconds later the player's `next` counter was unchanged.

## 6. Real-account tests

**Status: BLOCKED.** No approved temporary Spotify Free account was available. No real Spotify page was signed in. The following cases were not run and must not be inferred from the synthetic results.

| Case | Required coverage | Status | Reason |
|---|---|---|---|
| R1 | Free-account sign-in and normal playback, no false mute | **BLOCKED** | No approved account |
| R2 | Single real ad: detect, mute, unmute, one cycle | **BLOCKED** | No approved account |
| R3 | Consecutive ads in one break | **BLOCKED** | No approved account |
| R4 | Long ad and watchdog extension/fail-open | **BLOCKED** | No approved account |
| R5 | Non-English or alternate ad title | **BLOCKED** | No approved account |
| R6 | Pause/resume during a real ad | **BLOCKED** | No approved account |
| R7 | Skip during a real ad | **BLOCKED** | No approved account |
| R8 | Reload during a real ad | **BLOCKED** | No approved account |
| R9 | Background playback during a real ad | **BLOCKED** | No approved account |
| R10 | Sign-out/sign-in recovery | **BLOCKED** | No approved account |
| R11 | Discovery with real tracks and a real ad | **BLOCKED** | No approved account |
| R12 | Spotify skip limit / rejected skip | **BLOCKED** | No approved account |
| R13 | DRM/Widevine playback with `adblock` | **BLOCKED** | No approved account |
| R14 | `adblock ublock` on a real account | **BLOCKED** | No approved account |

The procedure is in [ADBLOCKING_REAL_ACCOUNT_TEST.md](ADBLOCKING_REAL_ACCOUNT_TEST.md). A tester must use an approved temporary Free account and a fresh/copied profile, and must not publish account data, track titles, listening history, or raw SMTC metadata.

## 7. Counts

| Metric | Value |
|---|---:|
| Real advertisements detected | **0** |
| Real ad mute/unmute cycles | **0** |
| Real-account sign-ins | **0** |
| Synthetic accepted skips (final runs) | 8 lifecycle + 5 cycles + 3 max + 2 UI + 1 watchdog = 19 |
| Synthetic ad breaks (final runs) | 1 lifecycle + 4 cycles + 1 watchdog = 6 |
| Synthetic mute/unmute cycles (final runs) | 1 lifecycle + 4 cycles = 5 |
| Synthetic fail-closed runs | 2 (mismatch, ambiguity) |
| Synthetic watchdog fail-open runs | 1 |
| Unit tests | 15 passed, 0 failed |

Synthetic counts are not evidence of real Spotify behavior.

## 8. Performance

### 8.1 Method

- Release `bench` builds: control (no adblock) and `bench adblock`.
- Fresh temporary profile per run.
- `bench/measure/tree-metrics.ps1` sampled the full process tree for PSS, working set, private commit, GPU memory, and CPU.
- 15-second samples after the page settled, except the transition run (30 seconds) and the idle check (60 seconds).
- The synthetic page plays a Web Audio tone. The "ad" state changes the document title and media-session title to the ad marker; the audio itself keeps playing so mute can be observed.
- CPU is reported as percent of one logical core; the machine has 16 logical cores.

### 8.2 15-second state comparison

| Build and state | Processes | PSS | CPU, one core |
|---|---:|---:|---:|
| Control, idle | 8 | 134.8 MB | 0.53% |
| `adblock`, idle, discovery off | 8 | 137.7 MB | 0.51% |
| Control, music | 9 | 171.0 MB | 2.52% |
| `adblock`, music | 9 | 173.9 MB | 2.42% |
| Control, ad title | 9 | 174.4 MB | 1.30% |
| `adblock`, ad muted | 9 | 173.3 MB | 1.21% |
| Control, background music | 9 | 173.7 MB | 1.41% |
| `adblock`, background music (run 1) | 9 | 172.3 MB | 2.62% |
| `adblock`, background music (run 2) | 9 | 171.9 MB | 1.20% |
| `adblock`, discovery, 2 s interval | 9 | 173.8 MB | 3.13% |
| `adblock`, repeated transitions, 30 s | 9 | 178.9 MB | 2.82% |

The transition run performed 5 accepted skips, 4 ad breaks, and 4 mute/unmute cycles in about 31 seconds. It is an aggressive synthetic stress test, not a real listening pattern.

CPU samples are noisy on this machine; the control music sample (2.52%) is higher than the `adblock` music sample (2.42%), which is not physically meaningful. The repeatable observations are:

- No new persistent process. Idle is 8 processes; playing states add one renderer (9).
- PSS is within about 3 MB of control in music and ad states; run-to-run noise is larger than the difference.
- Discovery adds 0-5 MB PSS depending on run and roughly 1-3% of one core for its timer and UI updates.
- The transition run averages about 2.8% of one core after the `--no-trim` busy-wait fix.

### 8.3 60-second idle with memory trim

The normal policy trims memory at 30 seconds and every 120 seconds. With trimming enabled, a 60-second idle sample measured:

| Build | Processes | PSS | CPU, one core |
|---|---:|---:|---:|
| Control | 8 | 28.1 MB | 0.29% |
| `adblock`, discovery off | 8 | 32.5 MB | 0.31% |

The 60-second PSS is much lower than the 15-second PSS because the hydrate trim has run. The adblock build is within 5 MB of control in this sample. Small differences are within run-to-run noise.

### 8.4 Harness artifact found and fixed

An earlier 60-second run with `--no-trim` showed about 64% of one core in both builds. The cause was not adblocking: with `--no-trim`, the policy's hydrate deadline was never consumed, so the event loop kept waking on a past deadline after 30 seconds. The same run also exposed an adblock-specific variant: when discovery was off but the page title was unknown, the discovery deadline remained armed and was never cleared.

Both paths were fixed during validation:

- `Discovery::next_deadline` returns `None` when discovery is disabled.
- `on_page_loaded` only records an unknown-title deadline when discovery is enabled.
- A `bench`-only `--no-trim` run no longer arms policy deadlines.

After the fix, the 60-second idle CPU returned to 0.2-0.8% of one core and the 30-second transition run measured 2.82%. The fixes affect the bench path and the disabled-discovery path; the shipping default build has no discovery code.

## 9. Safety and privacy review

| Check | Result |
|---|---|
| Track titles in logs | **PASS.** Logs contain state transitions, counts, failure reasons, and (debug only) a boolean title-match result and the AUMID. No document title or media title is written. |
| Credentials or tokens | **PASS.** No credential, cookie, token, or profile database is read or logged. |
| DRM or audio interception | **PASS.** No licence traffic, decryption, or audio stream is touched. Mute uses `ICoreWebView2_8::SetIsMuted`. |
| Network filtering | **Not implemented.** No request is blocked or modified in this milestone. |
| Page JavaScript injection | **PASS for the feature.** No ad-detection JavaScript is injected into Spotify. The existing nav-bar upsell stylesheet is unchanged. The synthetic test page is local and not Spotify. |
| SMTC scope | Uses documented `GlobalSystemMediaTransportControlsSessionManager` and `TrySkipNextAsync`. Requires exactly one media session, a WebView2 AUMID, `Playing`, Next enabled, and a normalized title match. Fails closed on every mismatch. |
| `MediaSessionService` flag | **Disclosed.** The `adblock` build passes `--enable-features=MediaSessionService` so WebView2 publishes a media session. This is scoped to adblock builds and must be rechecked on a real account. |
| Release artifacts | **PASS.** No installer, release note, tag, or published asset was changed. The stable `v0.2.0-pre.1` build is untouched. |
| Third-party code | **PASS.** No Blockify code was copied. No new dependency was added. uBOL remains behind the existing `ublock` feature and its GPL-3.0 obligations are unchanged. |
| Test profiles | **PASS.** All runs used fresh temporary profiles. No real signed-in profile was used. |

## 10. Limitations

1. **No real Spotify evidence.** The document-title format, SMTC session availability, ad timing, and DRM behavior on a real Free account are unverified.
2. **English-only ad marker.** The parser detects `Advertisement` as the whole title or the final separator segment. Localized titles and reversed formats such as `Advertisement - Spotify` are `Unknown`, not ads. They do not cause a skip, but they also do not mute.
3. **Strict single-session rule.** If any other Windows media session exists, even one that is paused, discovery fails closed with an ambiguous-session error. This is intentional but reduces usability.
4. **Title-match heuristic.** The player title and SMTC title must contain one another after normalization. If Spotify publishes a generic player title or a differently formatted media title, discovery stops. The reverse-containment path requires at least five characters.
5. **Ad-break counting.** A brief non-ad title between consecutive ads can be counted as a separate break and can cause an early unmute/remute. The synthetic consecutive test used contiguous ad titles.
6. **False-positive risk.** A real track whose title ends in `- Advertisement` (for example, an artist named Advertisement) would be muted. The watchdog bounds the mute but can keep it for up to 15 minutes while the title remains unchanged.
7. **Skip limit.** A rejected or disabled Next stops discovery. The app does not bypass Spotify's skip restrictions.
8. **Mute retry.** If `SetIsMuted(false)` keeps failing, the app retries every 5 seconds; there is no alternative fail-open path if the WebView API itself is broken.
9. **Page reload during an ad.** The page-load path unmutes first and then re-detects. A reload during an ad can produce a short audible gap and a new mute interval.
10. **`adblock ublock` is unverified on a real account.** The uBOL layer's effect on Spotify audio and DRM is still unproven.
11. **No installer or certification.** The adblocking build is not part of the installer and has no certification.

## 11. Risks

| Risk | Severity | Current mitigation | Residual |
|---|---|---|---|
| Ad plays audibly because detection fails | Medium | Fail-open by design; unknown titles do not skip | Real detection is unverified |
| Normal track muted as an ad | Medium | Exact ad-marker parser; bounded watchdog | False positives possible; up to 15 minutes |
| Permanent mute | High | Watchdog rechecks, 15-minute cap, unmute retries | WebView API failure could still leave audio muted |
| Skip in another app | High | Exactly one session, WebView2 AUMID, title match, fail closed | Another WebView2 app with a matching title is a residual risk |
| Spotify changes its title or media-session format | Medium | Fail-closed parsing and matching | Discovery would stop, not skip |
| DRM or playback breakage from `MediaSessionService` | Medium | No audio/DRM interception; flag scoped to adblock builds | Unverified on a real account |
| Spotify account action due to ad avoidance | High | User-facing disclosure; no credential handling | Cannot be eliminated; owner accepted the product risk |
| uBOL GPL compliance if distributed | Medium | Kept behind `ublock`; no installer change | A release would need licence and source compliance |
| Performance regression | Low | No new process; PSS within noise; discovery timer small | Real long-run measurement pending |

## 12. Next steps

1. Obtain an approved temporary Spotify Free account and run [ADBLOCKING_REAL_ACCOUNT_TEST.md](ADBLOCKING_REAL_ACCOUNT_TEST.md) on a copied profile.
2. Record the real document-title shapes and SMTC AUMID/title format. Do not publish track metadata.
3. Run the full real ad cycle, consecutive ads, a long ad, pause/resume/skip/reload, background playback, sign-out/sign-in, and DRM playback.
4. Decide whether to add a second signal (SMTC metadata or a documented page event) if the title is unreliable.
5. Decide whether the strict single-session rule is acceptable or needs a user-selectable session.
6. Re-measure memory and CPU on a real listening session against the control build.
7. Review the `MediaSessionService` side effects on media keys and background playback.
8. Complete a new certification report for any adblocking build before calling it stable.
9. Keep the installer, release notes, and `v0.2.0-pre.1` untouched until that certification passes.

## 13. Reproducibility notes

- The synthetic test page and scripts live outside the repository under `%TEMP%\opencode\rustify-adblock-bench`; they are not shipped.
- The synthetic tests used a local page only. The earlier `discovery-title-test.mjs` that injected a title into Spotify was not used for this report.
- The `bench` flags `--adblock-autostart`, `--adblock-interval`, `--adblock-max-skips`, and the watchdog overrides are compiled only with `bench`; they are not available in the default or release `adblock` build.
- The default build has no AD button, no discovery code, and no `MediaSessionService` flag.
- No tag or release asset was created by the test runs. The code and this report are committed together on `feature/adblocking` after the checks above; the uncertified pre-release `v0.3.0-adblock-pre.1` was published afterward from that commit.

## 14. Conclusion

The synthetic evidence supports the implementation as a safe experimental prototype: it starts only when asked, skips only confirmed tracks, stops skipping on ads and unknown titles, bounds mute duration, and fails closed on ambiguous sessions or title mismatches. It does **not** establish that the feature works on real Spotify ads. The real-account section remains blocked, the real ad count is zero, and the build is not certified. It is published only as an uncertified pre-release.
