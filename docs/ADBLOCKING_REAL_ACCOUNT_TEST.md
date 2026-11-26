# Real-account adblocking test procedure

**Date:** 2026-11-26  
**Branch:** `feature/adblocking`  
**Applies to:** `adblock` and `adblock ublock` builds only  
**Status:** **BLOCKED.** No approved temporary Spotify Free account has been provided. No real advertisement has been observed or tested. The synthetic results in [ADBLOCKING_VALIDATION_REPORT.md](ADBLOCKING_VALIDATION_REPORT.md) do not substitute for this procedure, and neither does the published pre-release `v0.3.0-adblock-pre.1`.

This procedure validates the experimental audio-ad mute fallback and the opt-in Automatic Ad Discovery mode against real Spotify Free playback. It is written so the test can be repeated without exposing a personal account, listening history, or track metadata.

## 1. Non-negotiable rules

1. **Do not use the owner's or any tester's personal Spotify account.** Use a temporary Spotify Free account created for this test and approved by the project owner.
2. **Do not use a real signed-in profile.** Start from a fresh, separate profile directory. Never copy the daily Rustify profile with cookies or tokens into the test directory.
3. **Do not record or publish** account names, e-mail addresses, cookies, tokens, track titles, artist names, listening history, raw SMTC metadata, or screenshots of a signed-in account. Crop or blur the account menu before capturing any screenshot.
4. **Do not test DRM circumvention, audio extraction, re-streaming, or alternate audio sources.** This procedure only observes the official player and the documented mute API.
5. **Stop immediately** on a permanent mute, a playback or sign-in failure, a DRM error, a crash, or a skip that changes a non-track page. Preserve the log, then stop the app.
6. **Do not modify, tag, or publish** `v0.2.0-pre.1` or any release artifact during this test. A passing run does not certify the build.

## 2. Preconditions

| Item | Requirement |
|---|---|
| Account | Approved temporary Spotify **Free** account, not Premium |
| Profile | Fresh directory under `%TEMP%`, or a separate Windows user; never the daily profile |
| OS | Windows 10/11 x64 with the Evergreen WebView2 Runtime |
| Network | Normal internet access; no VPN/proxy that changes Spotify's region unless the test is specifically about another region |
| Audio | A way to hear the output and confirm silence; system volume audible, Rustify volume unchanged |
| Builds | Default control, `adblock`, and `adblock ublock` release binaries; plus a `bench adblock` build for logs and scripted checks |
| Evidence | A text log, timestamps, counts, and a written observation for each case; no track metadata |

Record the environment before starting:

```powershell
Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version, BuildNumber
rustc --version
cargo --version
(Get-ItemProperty 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F1E7E2E4-1C4A-4A5B-9C3D-0B2F7E9A1C2D}').pv
```

## 3. Builds

Build all variants from the same commit. Do not mix binaries.

```powershell
cd app
New-Item -ItemType Directory -Force ..\dist-test | Out-Null
cargo build --release
Copy-Item .\target\release\rustify.exe ..\dist-test\rustify-default.exe
cargo build --release --features adblock
Copy-Item .\target\release\rustify.exe ..\dist-test\rustify-adblock.exe
cargo build --release --features "adblock ublock"
Copy-Item .\target\release\rustify.exe ..\dist-test\rustify-adblock-ublock.exe
cargo build --release --features "bench adblock"
Copy-Item .\target\release\rustify.exe ..\dist-test\rustify-adblock-bench.exe
```

The `adblock` builds pass `--enable-features=MediaSessionService` to WebView2. This is a disclosed, build-scoped change so the WebView2 player publishes a Windows media session. It must be verified not to break playback, sign-in, DRM, or background playback.

## 4. Isolated profile and launch

For a release build, override `LOCALAPPDATA` for the test process so the default profile stays untouched:

```powershell
$env:LOCALAPPDATA = Join-Path $env:TEMP 'rustify-real-account-2026-11-26'
.\rustify-adblock.exe
```

For an instrumented run, use the bench build and an explicit profile and log:

```powershell
.\rustify-adblock-bench.exe `
  --profile (Join-Path $env:TEMP 'rustify-real-account-bench-2026-11-26') `
  --log (Join-Path $env:TEMP 'rustify-real-account-bench-2026-11-26\rustify.log')
```

Do not pass `--adblock-autostart` for the manual sign-in phase. Discovery must remain off until the tester explicitly starts it.

## 5. Sign-in and baseline

| Step | Action | Pass criterion |
|---|---|---|
| 5.1 | Launch the default control build with an isolated profile. | Window opens; no AD button; no `MediaSessionService` flag in the log. |
| 5.2 | Sign in with the temporary Free account. | Sign-in completes; no credential prompt loop. |
| 5.3 | Play a normal track for at least 10 minutes. | Audio plays; normal tracks never mute; ads, if any, play normally; no skip; no discovery state. |
| 5.4 | Repeat 5.1-5.3 with the `adblock` build, discovery off. | Normal tracks play without mute; an ad, if it occurs, mutes; no skip; AD button present; discovery stays off. |
| 5.5 | Repeat with `adblock ublock`. | uBOL loads; playback and sign-in still work; no ad mute without an ad. |

Record the control result first. A failure in the control build is not an adblocking result.

## 6. Real advertisement cases

Run each case with the `adblock` build, discovery **off**, then repeat with discovery **on** where stated. Use a stopwatch or the log timestamps. Do not write down the ad's creative, track, or artist.

The skips, ads, and mute-cycle counters are discovery-session counters and reset when discovery starts. With discovery off, verify the mute fallback by audio and logs, not by those counters; repeat with discovery on to check the counters.

### 6.1 Single ad break, discovery off

1. Start normal playback and wait for an audio advertisement.
2. Confirm the audio becomes silent while the player still shows the ad.
3. Confirm the UI state becomes `Muted` and the log contains `adblock=ad_start muted`.
4. Wait for the ad to end and normal audio to return.
5. Confirm the log contains `adblock=ad_end unmuted`, the UI is no longer `Muted`, and audio has returned. Repeat with discovery on to confirm the mute-cycle counter increases by one.

**Pass:** exactly one mute, one unmute, no permanent silence, no skipped track; with discovery on, one cycle.  
**Fail:** no mute, mute after the ad, double mute, no unmute, or a cycle count that does not match when discovery is on.

### 6.2 Consecutive ads in one break

1. Wait for a break with two or more consecutive ads.
2. Observe the title transitions inside the break.
3. Confirm the audio stays muted across the whole break and unmutes only when normal playback returns.
4. Confirm the audio stays muted and the log shows one `ad_start`/`ad_end` pair for the break. With discovery on, confirm the ad count increases once and the mute-cycle count increases once.

**Pass:** one contiguous mute interval, one log pair; with discovery on, one ad count and one cycle.  
**Fail:** unmute between consecutive ads, multiple ad counts for one break, or a missed unmute.

### 6.3 Long ad

1. If a long ad is available, record its duration.
2. At 120 seconds of continuous mute, confirm `adblock=watchdog_extend`.
3. Confirm a recheck every 30 seconds while the ad title is still detected.
4. At 15 minutes of continuous mute, confirm `adblock=watchdog_fail_open unmuted`; if discovery is on, also confirm `discovery=error reason=watchdog fail-open`.
5. Confirm the same ad is not muted again after fail-open.

**Pass:** bounded extensions, one fail-open, no remute; discovery stops if it was on.  
**Fail:** mute beyond 15 minutes, repeated remute, or a silent app after fail-open.

### 6.4 Alternate or non-English title

1. Change the Spotify UI language, or use a temporary account in another region/language.
2. Wait for an ad and capture only the document-title shape, with track words replaced by placeholders. Example: `<word> - <word>`.
3. Do not publish the raw title if it contains a track or artist.
4. Record whether the app detected the ad, stayed `Waiting`, or muted a normal track.

**Pass:** a confirmed ad mutes; an unrecognized ad-like title is `Unknown` and does not cause a skip; a normal track is not muted.  
**Fail:** an unknown title is treated as a confirmed track, or a normal track is muted.

### 6.5 Pause, resume, skip, and reload during an ad

| Action | Expected |
|---|---|
| Pause during the ad | Audio stays paused; mute state does not produce sound; discovery does not issue a skip while paused. |
| Resume during the ad | Audio remains muted until the ad ends. |
| Click Next during the ad | The player moves to a track; audio unmutes; discovery resumes only if it was enabled. |
| Reload the page during the ad | Page load unmutes first, then re-detects the ad if it is still present; discovery resets its in-flight skip. |
| Hide/minimize the window during the ad | Playback and mute/unmute continue in the background. |
| Sign out during the ad | Audio unmutes; no permanent mute; sign-in page is usable. |
| Sign in again | Playback works; discovery does **not** start automatically. |

**Pass:** no permanent mute, no skip while paused, no skip of a non-track page, background audio works.  
**Fail:** permanent mute, skip storm, playback error, or a crash.

## 7. Automatic Ad Discovery cases

Start discovery explicitly from the `AD` panel. Do not use `--adblock-autostart` for the manual pass; that flag exists only in the `bench` build.

| Case | Steps | Pass criterion |
|---|---|---|
| 7.1 Start | Open `AD`, keep interval 5 s and max skips 50, click **Start**. | State becomes `Playing`, the ON/OFF badge becomes `ON`, and a skip is scheduled; Start disables and Stop enables. |
| 7.2 Normal skipping | Let discovery skip normal tracks. | Skips are sequential, at least the configured interval apart; the skip count increases only after an accepted skip. |
| 7.3 Ad stop | Wait for an ad. | Discovery stops skipping immediately; the ad is muted; the ad count increases once; no skip is issued during the ad. |
| 7.4 Resume after ad | Let the ad end. | Audio unmutes; discovery resumes skipping; the cycle count increases once. |
| 7.5 Pause | Pause playback during discovery. | State becomes `Waiting`; no skip is issued while paused. |
| 7.6 Resume | Resume playback. | Skipping resumes at the configured interval. |
| 7.7 Reload | Reload the page during discovery. | In-flight skip is cancelled; discovery resets and resumes only if the new page is a confirmed track. |
| 7.8 Unknown title | Navigate to a page whose title is not a confirmed track (home, search, sign-in). | Discovery waits; it does not skip; after 30 seconds it stops with an error. |
| 7.9 Max skips | Set max skips to 3 and start. | After 3 accepted skips, discovery stops with `max skips reached`; no fourth skip. |
| 7.10 Stop | Click **Stop** during normal playback. | Discovery stops, the ON/OFF badge becomes `OFF`, no further skips occur; any active ad mute remains until the ad ends. |
| 7.11 Skip limit | Use an account near Spotify's skip limit. | A rejected or disabled Next stops discovery with a clear reason; it does not retry in a loop. |
| 7.12 Multiple sessions | Open a second media session (playing or paused) while discovery runs. | Discovery fails closed with an ambiguous-session error; it does not skip the other app. |

**Pass:** every case above behaves as described, with no permanent mute and no skip of an ad or unknown page.  
**Fail:** a skip while an ad is detected, multiple skips in flight, a skip after Stop, a skip of another app, or a mute that survives the watchdog.

## 8. `adblock ublock` cases

Repeat sections 5, 6.1, 6.2, and 7.1-7.4 with the combined build.

Additional checks:

- uBOL service worker starts and at least one ruleset is enabled.
- Spotify sign-in, playback, and DRM still work.
- The uBOL layer does not block the player's audio or DRM licence traffic.
- The combined build does not change the ad-detection result compared with `adblock` alone.
- The GPL-3.0 licence and corresponding-source notice for uBOL are present in the test package.

## 9. Evidence and privacy

Collect:

- the exact build command and commit hash;
- OS, Rust, and WebView2 versions;
- the filtered log lines `adblock=`, `discovery=`, and `audio=`;
- the UI counters after each case;
- a written pass/fail per case;
- ad durations and counts, not ad creatives or track names.

Do not collect:

- cookies, tokens, profile databases, or WebView2 cache;
- raw SMTC metadata;
- screenshots that show the account name, e-mail, track title, artist, playlist, or listening history;
- network captures that contain credentials or DRM licence data.

Store logs outside the repository. If a log must be attached to a report, filter it to the lines above and confirm that no title or account identifier remains.

## 10. Stop conditions

Stop the run and report a failure if any of these occur:

- audio remains muted after the ad ends, after Stop, after reload, or after 15 minutes;
- playback, sign-in, DRM, or background playback breaks;
- discovery skips an ad, an unknown page, or another application's media session;
- discovery issues more than one skip at a time or ignores Stop;
- a normal track is muted as an advertisement;
- the app crashes or the process tree grows without bound;
- Spotify shows a skip-limit, playback, or account warning caused by the test.

## 11. Cleanup

1. Stop Rustify.
2. Sign out of the temporary account in the test profile.
3. Delete the temporary profile directory.
4. Delete any screenshots or logs that were not filtered.
5. Ask the owner whether the temporary account should be deleted.
6. Do not commit the test profile, logs, or account details.

## 12. Reporting

Complete the real-account table in [ADBLOCKING_VALIDATION_REPORT.md](ADBLOCKING_VALIDATION_REPORT.md). Until every required case has a result, the build remains **uncertified** and the real-account section remains **BLOCKED**. A synthetic pass must never be reported as a real-account pass.
