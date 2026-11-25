# uBlock Origin Lite in Rustify: Integration Research

**Research branch:** `research/ubol-webview2`  
**Date:** 2026-11-26  
**Status:** Isolated experiment. Not merged to `main`, not included in the installer, not certified, and not recommended for the shipping product.

---

## Executive summary

**Technically, yes: uBlock Origin Lite (uBOL) can be loaded into Rustify's existing `wry` + `tao` + WebView2 shell without replacing the architecture.** wry 0.57.4 already exposes the two WebView2 features this needs: `AreBrowserExtensionsEnabled` at environment creation and `ICoreWebView2Profile7::AddBrowserExtension`. The installed WebView2 runtime (158.0.4370.33) is well above the API's 120.0.2210.55 minimum.

The runtime test was successful:

- uBOL 2026.1124.1811 installed into the Spotify profile.
- Its MV3 service worker started.
- Six declarativeNetRequest rulesets were enabled.
- DNR matching worked: tracking scripts were redirected or blocked, and Spotify telemetry requests were blocked.
- Spotify loaded, stayed signed in, and a short playback sample progressed normally.
- No new browser engine, Tauri layer, or memory-lifecycle change was required.

**It should still not ship.** The blockers are product, policy, licensing, and maintenance—not the WebView2 API:

1. **It reverses Rustify's contract.** Rustify promises the official player with ads untouched. uBOL exists to block ads and trackers; loading it is an intentional behavioral change, even if modern Spotify audio ads turn out to be server-side inserted.
2. **It carries Spotify ToS and account risk.** Rustify's current positioning depends on not interfering with playback or ads. A bundled or silently enabled blocker removes that distinction.
3. **uBOL is GPL-3.0.** Bundling it changes distribution obligations. The MIT app can potentially remain MIT if uBOL is a separate work, but that is a legal question, not an engineering shortcut.
4. **WebView2 has no extension UI.** uBOL's popup, dashboard, badge, and permission surfaces have no host. Default filtering works; configuration does not, unless Rustify builds its own settings surface.
5. **The ad-blocking result is unproven and brittle.** uBOL's default rules contain a Spotify-specific rule that redirects `akamaized.net/audio/` media to a one-second no-op video. The current player fetched audio from `audio-fa-quic.spotifycdn.com` over Fetch, so that rule did not fire in this test. A future Spotify routing change could turn it into silent playback.
6. **The 0.2.0 certification no longer applies.** Any build that loads a content blocker needs a fresh 35-minute playing soak, 10-minute hidden/paused run, CPU and startup gates, and an ad-row check.

**Recommendation:** keep `main` clean. If this is pursued, keep it as a user-supplied, explicitly enabled experiment behind a non-default feature, with no bundled extension, no default profile changes, and no support claims. Do not present it as part of Rustify 0.2.0.

---

## What the spike changes

The prototype is deliberately small and isolated:

- `app/Cargo.toml` gains a non-default `ublock` feature.
- `app/src/main.rs`:
  - reads `--ublock-ext <path>` or `RUSTIFY_UBLOCK_EXT` (the flag wins if both are set);
  - validates that the folder contains `manifest.json`;
  - enables WebView2 browser extensions only when a path is supplied;
  - installs the extension through `ICoreWebView2Profile7::AddBrowserExtension`;
  - logs the async install result;
  - makes the chrome WebView share the player's WebView2 environment.
- No extension binary is committed. The user supplies an unpacked uBOL folder.
- The default build does not enable extensions, does not parse the flag, and behaves exactly as 0.2.0 does.

The essential calls:

```rust
// Only when the ublock feature is enabled and a path was supplied.
builder = builder.with_browser_extensions_enabled(true);

let profile = webview
    .webview()
    .cast::<ICoreWebView2_13>()?
    .Profile()?
    .cast::<ICoreWebView2Profile7>()?;

profile.AddBrowserExtension(&HSTRING::from(extension_path), &handler)?;

// The chrome WebView must use the same environment, not a second one
// with different extension options and the same data directory.
bar_builder = bar_builder.with_environment(webview.environment());
```

### Why not wry's `with_extensions_path`?

wry 0.57.4 has `with_extensions_path`, but it iterates every entry in the directory and calls `AddBrowserExtension` on each one, ignoring completion errors. That API expects a parent directory of extension folders, not the extension root itself. The spike installs one explicit folder through `Profile7` so failures are visible in `rustify.log`.

### Why the chrome WebView must share the environment

wry creates a WebView2 environment per WebView unless one is passed with `with_environment`. Enabling extensions changes the environment options. The player and chrome bar share a data directory, and WebView2 rejects or misbehaves when two environments use the same data directory with different options. Passing the player's environment to the bar keeps one browser process and one profile.

---

## Test environment

| Item | Value |
|---|---|
| OS | Windows 11 x64 |
| WebView2 runtime | 158.0.4370.33 |
| Rustify | 0.2.0 source, `bench + ublock` features |
| wry | 0.57.4 |
| tao | 0.37.3 |
| webview2-com | 0.39.1 |
| uBOL | 2026.1124.1811, Chromium build |
| uBOL manifest | MV3, `minimum_chrome_version` 122.0 |
| uBOL permissions | `activeTab`, `alarms`, `declarativeNetRequest`, `offscreen`, `scripting`, `storage`, `unlimitedStorage`, `userScripts`, `<all_urls>` |
| uBOL size | 9.17 MB zip, 31.7 MB unpacked |
| License | GPL-3.0 |
| Test profile | Copy of the existing signed-in WebView2 profile, outside the repo |

The copied profile retained the signed-in session. The real profile was not used for the extension test.

---

## Runtime results

### Extension installation

The first run logged:

```text
main_webview_ready
ublock_install_requested
ublock_chrome_environment_shared
chrome_bar_ready
ublock_install_ok
```

The extension was present in the profile after a restart, and installing the same folder again succeeded. The observed extension ID was:

```text
cdkkimgbflfmlffihblkpkhkcmhfdlgc
```

That ID is derived from the unpacked path. A different install folder means a different ID and a fresh extension storage area. Shipping code would need a stable path and a versioned payload strategy.

### Service worker and rulesets

CDP showed a live target:

```text
service_worker  chrome-extension://cdkkimgbflfmlffihblkpkhkcmhfdlgc/js/background.js
```

Querying the worker returned:

```json
{
  "name": "uBlock Origin Lite",
  "version": "2026.1124.1811",
  "manifestVersion": 3,
  "minimumChromeVersion": "122.0",
  "rulesetCount": 6,
  "rulesets": [
    "ublock-filters",
    "easylist",
    "easyprivacy",
    "pgl",
    "ublock-badware",
    "urlhaus-full"
  ]
}
```

`declarativeNetRequest.testMatchOutcome` confirmed actual matching:

| Test URL | Result |
|---|---|
| `https://www.google-analytics.com/analytics.js` | matched `ublock-filters` rule 4738; redirect to a local no-op script |
| `https://ad.doubleclick.net/ddm/ad/123` | matched `pgl` rule 1; block |
| `https://connect.facebook.net/en_US/fbevents.js` | matched `easyprivacy` rule 1336; block |
| `https://open.spotify.com/` (document) | matched only a `modifyHeaders` rule that appends `Permissions-Policy` values |
| `https://audio-ak-spotify-com.akamaized.net/audio/...` (media) | matched `ublock-filters` rule 4103; redirect to `noop-1s.mp4` |
| `https://audio-fa-quic.spotifycdn.com/audio/...` (Fetch) | no match in the tested rulesets |

Rule 4103 is the risky one:

```json
{
  "id": 4103,
  "action": {
    "type": "redirect",
    "redirect": {
      "extensionPath": "/web_accessible_resources/noop-1s.mp4"
    }
  },
  "condition": {
    "initiatorDomains": ["open.spotify.com"],
    "resourceTypes": ["media"],
    "urlFilter": "||akamaized.net/audio/"
  }
}
```

It deliberately targets Spotify media on `akamaized.net/audio/`. The current player did not use that path, so playback was not affected. That is not a guarantee; it is a compatibility landmine.

### Playback comparison

Two short runs were made with separate copies of the same signed-in profile and the same page:

- **Control:** no extension loaded.
- **uBOL:** extension loaded from the unpacked folder.

Both runs:

- loaded `open.spotify.com`;
- kept the existing sign-in;
- rendered the player controls;
- advanced playback and accepted a pause;
- used `audio-fa-quic.spotifycdn.com` Fetch requests for audio.

uBOL-specific network behavior:

- `gae2-spclient.spotify.com/gabo-receiver-service/v3/events` was blocked with `net::ERR_BLOCKED_BY_CLIENT`.
- One audio Fetch was aborted during seeking. It was a seek cancellation, not evidence of a blocker failure.
- No `noop-1s.mp4` request and no `akamaized.net/audio/` request appeared in either run.

The test did **not** prove that audio ads are blocked. Spotify's current playback path is Fetch-based and shares the audio CDN with normal playback. Server-side ad insertion can make network-level blocking ineffective, and forcing it can break playback. A full free-account ad cycle was not run.

### Startup

Debug-build window-ready times:

| Run | Window ready |
|---|---:|
| Control 1 | 274 ms |
| Control 2 | 283 ms |
| Control 3 | 302 ms |
| uBOL, warm install | 298 ms |
| uBOL, warm install | 364 ms |
| uBOL, first cold install | 492 ms |

The asynchronous extension install completed around 0.89–0.95 s after launch. It did not block window creation.

These are debug-build measurements, not release certification. The first cold install is the only run close to the 500 ms startup budget; warm starts overlapped the control range, with the slowest warm run 62 ms behind the slowest control run.

### Memory and CPU

A short hidden/paused comparison used `--trim-sec 30 --pause-sec 10 --gap-sec 5` so the Low cycle ran quickly. Each sample was 10 seconds of process-tree PSS:

| Build | Hidden/paused PSS | Processes |
|---|---:|---:|
| Control | 109.8 MB | 11 |
| uBOL | 91.8 MB | 11 |

uBOL did not add a persistent process in the hidden state, and it did not increase this short sample. In one visible/paused sample uBOL measured 326.1 MB against a 435.0 MB control, but the two pages were not in an identical content state, so that comparison is not valid. CPU samples in the debug build (1.34–2.21% of one core over 10 seconds, including trim work) are not comparable to the certified release result of 0.24–0.39%.

**No certification claim is made.** A shipping decision would require the full release gate: 35 minutes playing with every five-minute sample under 400 MB, 10 minutes hidden/paused under 300 MB, idle/paused CPU, startup, and a fresh ad-row check.

---

## Why the extension is hard to ship

### 1. Product contract and Spotify terms

Rustify's current contract is explicit: official Spotify Web Player, ads play untouched, no ad blocking or filtering. uBOL is a content blocker. Even if it fails to remove Spotify's audio ads today, it blocks telemetry and display requests and is intended to block ads. Shipping it changes the product's identity and its Spotify ToS risk.

The existing certification recorded ad rows during the 35-minute soak. Those rows are part of the evidence that the official playback path is untouched. A uBOL build invalidates that evidence.

### 2. GPL-3.0

uBOL is GPL-3.0. Bundling its files in the MIT installer requires GPL compliance for those files: source availability, license text, and no additional restrictions. A user-supplied extension avoids redistribution, but a bundled or auto-downloaded extension needs a deliberate licensing review. Do not commit the uBOL payload to this repository.

### 3. No extension UI in WebView2

Microsoft's `AddBrowserExtension` documentation notes that extensions with icons, badges, popups, or other UI can load, but the UI entry points are missing because WebView2 has no browser chrome to host them.

uBOL's default rules work without its popup. Its dashboard, per-site switches, filter-mode selection, and allowlist do not. A real integration would need:

- a Rustify-owned settings window;
- a way to navigate a child WebView to `chrome-extension://<id>/dashboard.html`;
- permission/consent UX;
- a fallback when the dashboard changes between uBOL versions.

None of that exists in this spike.

### 4. Persistence and updates

`AddBrowserExtension` persists the extension in the WebView2 profile. The documentation also states that changing the extension folder's contents removes the extension from the profile. That means:

- Every app update that changes uBOL must reinstall it.
- The extension folder must be stable and readable after reboot.
- A versioned folder changes the unpacked extension ID unless the manifest carries a fixed key, which uBOL's release build does not.
- A temp folder is not a valid long-term location.

The spike re-added the extension at startup and logged success, but it does not implement update or integrity handling.

### 5. Security and trust

A browser extension loaded into the Spotify profile runs with page access and a broad permission set (`<all_urls>`, `scripting`, `userScripts`). A tampered extension folder could read page content or interfere with the session. Any bundled/downloaded payload needs:

- a pinned version;
- SHA-256 verification;
- a trusted update channel;
- a clear user consent step;
- a removal path.

A user-supplied `--ublock-ext` path is acceptable for an experiment only because the user is explicitly choosing the code.

### 6. Ad-blocking efficacy is not established

The current Spotify player fetched audio from `audio-fa-quic.spotifycdn.com` over Fetch. uBOL's Spotify-specific media rule targets `akamaized.net/audio/` and did not match. That suggests uBOL may not remove current audio ads at all, while still blocking telemetry and other requests. The worst outcome is a build that changes behavior, breaks certification, and still plays ads.

The opposite outcome is also possible: if Spotify returns to a media-element path on `akamaized.net/audio/`, rule 4103 redirects it to a one-second no-op video, which could silence music. This is exactly why ad filtering inside a DRM player is brittle.

### 7. Extension API coverage

WebView2 supports unpacked extensions, but not every Chromium extension API or UI surface. The tested paths—service worker, storage, declarativeNetRequest, runtime—worked. uBOL also requests `offscreen` and `userScripts`; their full behavior was not verified. A future uBOL release could depend on an API WebView2 does not implement.

---

## Options

| Option | What it means | Assessment |
|---|---|---|
| **Keep Rustify clean** | No extension loading; preserve the official player and 0.2.0 certification. | **Recommended.** Matches the product contract and avoids ToS/GPL work. |
| **User-supplied experiment** | Keep the `ublock` feature off by default; require an explicit path; no bundling; no support claim. | Acceptable for research. Still changes behavior and carries Spotify ToS risk. |
| **Full shipping integration** | Bundle or download uBOL, add settings UI, license compliance, re-certify, document the changed contract. | Not recommended. Large scope, legal exposure, brittle filtering, and a different product. |

The branch implements the second option only. It is not wired into `main`, the installer, or the release notes.

---

## Reproduction

These steps are for an isolated machine and a copied profile. Do not use the real profile for experiments, and do not commit profiles, logs, or extension payloads.

1. Download the official Chromium build from the uBOL release page:

   ```text
   https://github.com/uBlockOrigin/uBOL-home/releases
   ```

   Use the `uBOLite_<version>.chromium.zip` asset.

2. Extract it so the folder contains `manifest.json` at its root.

3. Build the spike:

   ```powershell
   cargo build --manifest-path app/Cargo.toml --features "bench ublock"
   ```

4. Run it against a copied profile:

   ```powershell
   app\target\debug\rustify.exe `
     --profile C:\temp\rustify-ublock-profile `
     --log C:\temp\rustify-ublock.log `
     --cdp 9333 `
     --url https://open.spotify.com/ `
     --ublock-ext C:\temp\uBOLite-2026.1124.1811.chromium
   ```

5. Confirm the install:

   ```powershell
   Select-String C:\temp\rustify-ublock.log -Pattern 'ublock'
   ```

6. Inspect the extension worker with CDP:

   ```text
   http://127.0.0.1:9333/json/list
   ```

   Look for a `service_worker` target under `chrome-extension://`.

7. Query DNR from that worker:

   ```js
   chrome.declarativeNetRequest.getEnabledRulesets()
   chrome.declarativeNetRequest.testMatchOutcome({
     url: "https://www.google-analytics.com/analytics.js",
     type: "script",
     initiator: "https://open.spotify.com"
   })
   ```

A successful test looks like six enabled rulesets and a matched rule for the tracker URL. A failed test logs `ublock_install_failed` with an HRESULT.

---

## Open questions

1. **Does uBOL remove actual Spotify audio ads on a free account in 2026?** Not established. The current audio path is Fetch-based, and the tested Spotify-specific rule targets a different path.
2. **Does `userScripts` work in WebView2?** The extension loaded and DNR worked; scriptlet injection was not verified.
3. **Can the uBOL dashboard be hosted in a child WebView?** Not tested. WebView2 has no extension UI, so this would need a custom surface.
4. **What is the release-build cost over 35 minutes of playback and 10 minutes hidden?** Not measured. The short debug samples are directional only.
5. **Is GPL-3.0 compatible with the intended distribution model?** Requires legal review if uBOL is bundled or auto-downloaded.
6. **Would Spotify treat a blocker-loaded WebView as a modified client?** The account-risk answer is not knowable from the client side; the safe assumption is yes.

---

## Bottom line

The engineering question is answered: **uBOL can be loaded into Rustify's current architecture with a small, feature-gated change.** The product question is not answered in uBOL's favor. Loading an ad blocker into a client whose promise is "the official player, ads untouched" is a product and policy change, not a performance optimization. The branch is useful as a reference and a warning: the integration works, but the compatibility, licensing, UI, and certification costs are real.
