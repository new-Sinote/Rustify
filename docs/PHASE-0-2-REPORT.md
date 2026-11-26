# Rustify - Phase 0-2 Feasibility Report and Design

> **Historical document - superseded constraint.** This report was written when "ads intact" was a product requirement. On 2026-11-26 the owner made adblocking a first-class requirement. The ads-intact statements below are historical and must not be used to reject adblocking work. See [ADR-0001](ADR-0001-adblocking-direction.md) and [ADBLOCKING_RESEARCH.md](ADBLOCKING_RESEARCH.md).

**Date:** 2026-11-25  
**Status:** Awaiting approval. No production code has been written.  
**Scope:** A native Rust desktop host for the official Spotify web player (`open.spotify.com`), free accounts, ads intact, no DRM circumvention, no scraping or internal Spotify APIs, with Spicetify-style customization evaluated but not assumed.

**Confirmed scope decision (2026-11-25):** v1 targets **Windows 10/11 (x64) only**. macOS and Linux are out of scope for v1; the macOS/Linux analysis is retained for context and possible later ports. This resolves approval point 3. The toolchain has been installed and a Rust MSVC build was verified on this machine.

---

## 0. Verdict

**Conditional GO, but not as specified.** An unmodified official Spotify web player can probably run on **Windows** through WebView2 (Edge's Widevine CDM is present in practice, but Microsoft does not guarantee Widevine in WebView2) and on **macOS** through WKWebView (Spotify supports Safari, and multiple 2026 WebKit browsers play Spotify via FairPlay). **Linux WebKitGTK is not viable** for embedded playback without a Widevine CDM, and **CEF/castlabs** is technically capable but violates the installer and RAM budgets by an order of magnitude. The hard **<150 MB RAM** target is already contradicted by measurements: 210 MB RSS for a WKWebView Spotify SDK spike, 381 MB PSS for a Chromium web player, and 68–89 MB for an empty WebView2 window. **Spicetify-style CSS/JS theming and extensions cannot be implemented within the stated no-injection/no-internal-API constraints** and would conflict with Spotify's User Guidelines; only native-shell theming and documented-Web-API companion features are defensible. Recommended path: **Tauri 2 host on Windows, one remote Spotify webview with zero IPC capabilities, native tray/shortcuts, SMTC media integration, documented PKCE Web API extras**, with revised performance budgets and a short Phase 3 Windows DRM/background-playback spike as the go/no-go gate. macOS and Linux are out of scope for v1.

---

## 1. Method and confidence

- The working directory was empty and no Rust toolchain was installed. Only Windows was available. **No playback experiment was run locally.**
- Findings come from current primary documentation and dated third-party reports. Each claim is labelled:
  - **Verified** — primary documentation or a direct measurement with commands/numbers.
  - **Empirical** — a third-party report with reproducible details, not independently reproduced here.
  - **Unverified** — requires the Phase 3 spike.
- All sources were accessed on **2026-11-25**. Spotify's consumer Terms of Use were last updated **2026-09-04**.
- Factual claims have inline links to primary sources where available; third-party measurements are marked **Empirical**. Design recommendations and engineering judgements are explicitly framed as recommendations.
- This report is technical and policy analysis, **not legal advice**.

---

## 2. Phase 0 — Playback and DRM feasibility

### 2.1 Matrix

| Platform | Engine | DRM path | Evidence | Free account | Verdict |
|---|---|---|---|---|---|
| Windows 10/11 | WebView2 (Chromium/Edge) | Widevine in practice; PlayReady officially | Lightweight Browser Windows spike (2026-11-21): Spotify plays in `tao + wry + WebView2`; MixMusic uses a hidden WebView2 for Spotify; Cider uses WebView2 with Widevine and PlayReady fallback. Microsoft's Widevine request is still open. | Likely; not independently verified with a free account | ⚠️ **Conditional GO** — runtime DRM probe required |
| macOS 13+ | WKWebView (WebKit) | FairPlay | Spotify supports Safari; Crest, Ardia, OpenArch and ilo use WebKit for Spotify in 2026; Slopify's WKWebView spike got a 200 from Spotify's FairPlay certificate and licence endpoints and played 36.6 s (Premium SDK). | Likely via the web player; full free-account web player not directly verified | ⚠️ **Conditional GO** — full web player and background playback spike required |
| Linux | WebKitGTK 4.1/6.0 | EME optional at build time; no Widevine CDM shipped | WebKitGTK CMake makes `ENABLE_ENCRYPTED_MEDIA` default to experimental features and requires Thunder/OpenCDM when enabled; Spotish (2026-11) says Widevine cannot be bundled and uses an installed Chromium for Linux playback. | No embedded path found | ❌ **NO-GO embedded** — external browser or companion fallback |
| Any | CEF / castlabs | Widevine + VMP signing | castlabs includes the CDM and VMP signing; CEF adds 317 MB to a 367 MB bundle, 136 MB to a 154 MB update zip, and five helper processes use ~188 MB with no browser card. | Technically yes | ❌ **NO-GO** — size, RAM and licensing |
| Any | Installed Chromium/Edge, external | Widevine | Spotish drives the installed browser; Spotify supports Chrome/Edge/Firefox/Opera. Spotish measures 381 MB PSS for the whole tree. | Yes | ⚠️ **Fallback only** — not embedded; automation is policy-grey |

### 2.2 Windows — WebView2

**What is verified**

- WebView2 uses the same multi-process model as Edge: a browser process, one or more renderers, a GPU process and helper processes ([Microsoft process model](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-model)).
- Microsoft documents Evergreen and Fixed Version distribution, and says the runtime must be present on the client machine ([distribution](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution), [Evergreen vs Fixed](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/evergreen-vs-fixed-version)).
- WebView2 supports **PlayReady**; **Widevine is an open feature request**, not a documented guarantee. The request has been open since 2024-09-21; Microsoft's only reply asked the reporter for a workaround, with no commitment ([WebView2Feedback #4828](https://github.com/MicrosoftEdge/WebView2Feedback/issues/4828)). Earlier issue #1291 was a false alarm caused by an HTTP origin, not HTTPS ([#1291](https://github.com/MicrosoftEdge/WebView2Feedback/issues/1291)).

**What is empirical**

- Cider, an Apple Music client, uses WebView2 on Windows and Widevine. Its 4.0.31 changelog says that when Widevine fails or is out of date, Cider falls back to PlayReady and retries Widevine on the next launch ([Cider 4.0.31](https://cider.sh/changelogs/4.0.31)).
- The Lightweight Browser Windows spike (2026-11-21) reports: "Netflix plays at 1080p (software Widevine) and Spotify plays" using `tao + wry + webview2-com`; an empty window took 68 MB private working set without the chrome view and 89 MB with it ([PR #6](https://github.com/TheYungStump/lightweight-browser/pull/6)).
- MixMusic reports that Spotify tracks play in a hidden WebView2 window because "Spotify's audio is encrypted and only a browser engine's decryption module can play it" ([MixMusic #930](https://github.com/fzn0x/mixmusic/issues/930)).

**Interpretation**

WebView2 + Spotify is **working today on at least some machines**, but it is **not a supported contract**. The CDM is an Edge component; it can be missing, stale, blocked by enterprise policy, or broken by a runtime update. Fixed-version WebView2 pins an old CDM; Evergreen gets updates but can change under the app. A runtime probe is mandatory: call `navigator.requestMediaKeySystemAccess('com.widevine.alpha', ...)`, request a licence for a short Spotify preview/track, and verify `MediaKeys` creation and playback. On failure, offer (a) Evergreen runtime repair, (b) open in installed Edge/Chrome, or (c) official Spotify desktop client companion.

**Memory implication**

An empty WebView2 window is already 68–89 MB private working set. A full Spotify web player adds a renderer with substantial JavaScript, artwork and media buffers. The 150 MB total budget is not credible on this engine.

### 2.3 macOS — WKWebView

**What is verified**

- Spotify's web player officially supports desktop Safari, alongside Chrome, Firefox, Edge and Opera ([Spotify web player help](https://support.spotify.com/us/article/web-player-help/)).
- Safari does not use Widevine; Spotify's DRM error guidance names Widevine for Chrome/Firefox/Edge, not Safari. The macOS Safari path is FairPlay.
- WKWebView does not ship Widevine. Older Tauri issue #7400 reported that Widevine DRM did not work in Tauri's macOS WKWebView and that Tauri cannot bolt DRM onto WKWebView ([Tauri #7400](https://github.com/tauri-apps/tauri/issues/7400)). That report is about Widevine and predates the FairPlay evidence below; it is not evidence against FairPlay.

**What is empirical**

- Slopify's 2026-09-30 spike ran Spotify's Web Playback SDK in a hidden WKWebView through `wry`. The SDK selected FairPlay (`com.apple.fps.1_0`) through WebKit's legacy prefixed `WebKitMediaKeys`; the FairPlay certificate endpoint returned 200; the licence POST returned 200; playback reached 36.6 s across four runs. Total RSS was about **210 MB** (Rust 77 MB, WebKit GPU 58 MB, WebContent 45 MB, Networking 27 MB) ([Slopify #22](https://github.com/jnsdls/slopify/issues/22), [ADR 0002](https://github.com/jnsdls/slopify/blob/main/docs/adr/0002-player-in-wkwebview-ui-in-gpui.md)).
- The SDK is **Premium-only**, so this proves FairPlay in WKWebView, not the free-account web player.
- Several 2026 macOS browsers route Spotify to WebKit for protected media: Crest says "It moves to WebKit for protected media" and plays ([Crest #577](https://github.com/crest-browser/Crest/issues/577)); Ardia says Spotify runs using the WebKit engine ([Ardia #4068](https://github.com/ardia-browser/ardia/issues/4068)); OpenArch says Spotify, Netflix and other DRM services "play with Safari's engine" and FairPlay ([OpenArch #33](https://github.com/dpfaffenbauer/OpenArch/pull/33)); ilo reports Spotify's web player opening in the system engine on Mac ([ilo #3432](https://github.com/MartinPTielemans/ilo/issues/3432)).

**Interpretation**

FairPlay in WKWebView is real and current. The remaining questions are specific:

1. Does the **full free-account web player** (not the Premium SDK) negotiate FairPlay and play in a WKWebView with a Safari-like user agent?
2. Does playback continue when the window is minimized, occluded, or on another Space? Crest reports that Spotify stops after the current song when its tab is in the background on macOS ([Crest #577](https://github.com/crest-browser/Crest/issues/577)).
3. Does Spotify's feature detection accept the WKWebView user agent, or does it show "browser not supported"?

Apple's App Nap documentation says playing music is user-initiated work and the system may still regulate timers; a hidden/occluded webview can be throttled ([App Nap](https://developer.apple.com/library/archive/documentation/Performance/Conceptual/power_efficiency_guidelines_osx/AppNap.html)). The app may need `NSProcessInfo.beginActivity` and a visible/hidden-window strategy. This is a spike item, not an assumption.

**Memory implication**

210 MB RSS for the SDK already exceeds 150 MB. A full web player with artwork, browse UI and ads will be higher. The budget cannot hold on macOS either.

### 2.4 Linux — WebKitGTK

**What is verified**

- WebKitGTK's EME support is build-time optional. In WebKit's GTK CMake, `ENABLE_ENCRYPTED_MEDIA` defaults to `ENABLE_EXPERIMENTAL_FEATURES`, and if EME and Thunder are enabled, `find_package(Thunder REQUIRED)` is used ([OptionsGTK.cmake](https://github.com/WebKit/WebKit/blob/main/Source/cmake/OptionsGTK.cmake)).
- The `WebKit.Settings.enable_encrypted_media` property only has an effect if EME was enabled at build time ([Valadoc](https://valadoc.org/webkitgtk-6.0/WebKit.Settings.enable_encrypted_media.html)).
- Google requires a licence agreement for Widevine and does not make the CDM generally redistributable ([Widevine overview](https://developers.google.com/widevine/drm/overview)).

**What is empirical**

- Spotish, a 2026 native Spotify client, states: "Spotify's free tier plays only in Spotify's web player, with DRM (Widevine), in a real desktop browser... Widevine, the DRM module that decodes Spotify's audio, cannot be bundled with Spotish; it comes with your browser (Chromium downloads it on first run)." Its Linux build uses an installed Chromium/Chrome/Edge/Brave, not WebKitGTK ([Spotish README](https://github.com/huacnlee/spotish)).
- Multiple 2026 projects note that Widevine does not work in WebKitGTK yet when planning Spotify support ([Pulse #276](https://github.com/GregHofmeyr/pulse-app/issues/276), [Syrinx #210](https://github.com/Fledge-Software/Syrinx-app/issues/210)).

**Interpretation**

There is no evidence that stock distro WebKitGTK can play Spotify. Even if a distro enables EME, it would still need a Widevine CDM and the associated licence/provisioning. WebKitGTK uses OpenCDM/Thunder rather than Chrome's CDM, so copying Chrome's `libwidevinecdm.so` into WebKitGTK is not a supported solution. **Embedded Linux playback is a no-go.**

**Linux options, in order of compliance**

1. **Companion to the official Spotify desktop client** via MPRIS. The desktop client ships its own DRM, supports free accounts and ads, and exposes `org.mpris.MediaPlayer2`. The native shell can read metadata and send play/pause/next/previous over D-Bus. This is the most defensible Linux product, but it is not a webview host.
2. **External installed Chromium in app mode** (`--app=https://open.spotify.com` with a dedicated profile). The app is a launcher plus a native companion; the browser window is not embedded. No CDP needed if the user interacts directly. Metadata/control depend on the browser's MPRIS integration.
3. **CDP-driven hidden Chromium** (Spotish-style). Integrated and works, but it is automation, reads the page, and is policy-grey; 381 MB PSS. Reject for this project unless the constraints are relaxed.

### 2.5 CEF / castlabs

- `cef-rs` is maintained (crate `cef` 156.7.0+156.0.31, 2026-11-19; Apache-2.0; repo active 2026-11-25). `tauri-runtime-cef` is still `3.0.0-alpha.7` (2026-11-06), not a production base ([cef-rs](https://github.com/tauri-apps/cef-rs), [crates.io](https://crates.io/crates/cef), [tauri-runtime-cef](https://crates.io/crates/tauri-runtime-cef)).
- castlabs' Chromium/Electron builds include the Widevine CDM and are pre-approved for VMP self-signing; a Google licence agreement is still required ([castlabs certification](https://castlabs.com/security/widevine-certification/)).
- Size is disqualifying: CEF is 317 MB of a 367 MB bundle and 136 MB of a 154 MB update zip; five helper processes use about 188 MB with no browser card ([Infiniterm #761](https://github.com/ekinertac/infiniterm/issues/761)).

**Verdict:** technically the most reliable DRM path, but it fails `<30 MB` installer and `<150 MB` RAM by a wide margin, and adds a commercial licensing/provisioning burden. Not viable for this project.

### 2.6 What this means for the original performance targets

| Target | Phase 0 finding | Status |
|---|---|---|
| <150 MB RAM with player running | 210 MB RSS for WKWebView + Spotify SDK; 381 MB PSS for Chromium + web player; 68–89 MB for an empty WebView2 window | **Not achievable** |
| <1% CPU idle | Depends on definition. A paused, hidden webview can be near 0; a visible Spotify page animates and polls. Playing audio is not idle. | **Needs definition and measurement** |
| <500 ms to window | Plausible for the native shell window; not for the Spotify page becoming interactive. | **Needs definition and measurement** |
| Installer <30 MB | Plausible for Tauri + system webview on Windows/macOS; not for CEF; Linux packaging depends on WebKitGTK. | **Plausible, measure** |
| No bundled Chromium | Tauri/wry and external-browser mode meet it; CEF does not. | **Met by recommended stack** |

### 2.7 User agent, feature detection, login and popups

- Spotify supports "baseline widely available versions" of Chrome, Firefox, Edge, Opera and Safari, updated within the last 30 months ([Spotify web player help](https://support.spotify.com/us/article/web-player-help/)). A WebView2/WKWebView user agent may not match Spotify's allowlist exactly.
- Do **not** spoof to evade a block. Set a standard desktop browser UA only if the engine's default is rejected and the page otherwise behaves correctly; document it as a compatibility measure, not a circumvention.
- Login flows may open popups (Google, Facebook, Apple) and challenge pages. The Lightweight Browser spike tested Google sign-in with opener-preserving popups in WebView2 ([PR #6](https://github.com/TheYungStump/lightweight-browser/pull/6)). A strict allowlist must still permit the auth domains in a dedicated, zero-IPC popup webview, or the app must support email/password login only and send social login to the system browser.
- External links must open in the system browser, never in the Spotify webview.

### 2.8 Reproducible test plan (Phase 3 gate)

These are the experiments that must pass before building the full app. They are deliberately small.

**Windows**

1. Install the WebView2 Evergreen runtime. Build a minimal `tao + wry` window with a persistent data directory and no IPC.
2. Load `https://open.spotify.com/`. Log the UA and the result of:
   ```js
   await navigator.requestMediaKeySystemAccess('com.widevine.alpha', [{
     initDataTypes: ['cenc'],
     audioCapabilities: [{ contentType: 'audio/mp4; codecs="mp4a.40.2"' }]
   }]);
   ```
3. Log in with a **free** account. Play a track, confirm audio, confirm an ad plays when Spotify inserts one, and confirm no ad-blocking code exists.
4. Measure private working set and commit for the app and all WebView2 child processes after 60 s of playback.
5. Repeat with the window minimized and with the window occluded. Confirm playback continues.
6. Repeat with a Fixed Version runtime and with the Widevine component removed/disabled. Record the exact failure and the fallback path.
7. Pass: free-account audio plays; ads are intact; no crash; failure is detectable and actionable. Record RAM, startup, CPU.

**macOS**

1. Build a minimal `wry` WKWebView app with a persistent data store and no IPC. Load `https://open.spotify.com/`.
2. Log in with a free account. Confirm the page does not show "browser not supported".
3. Use Web Inspector to confirm the FairPlay certificate request (`spclient.wg.spotify.com/fairplay-license/...`) and licence POST return 200, and that `WebKitMediaKeys` (or standard EME) is used.
4. Play a track; confirm audio and ads. Measure `footprint`/`vmmap` for the app, WebContent, GPU and Networking processes after 60 s.
5. Minimize, occlude, and switch Spaces. Confirm the next track still plays. If it stops, test `NSProcessInfo.beginActivity` and a hidden-but-unoccluded window.
6. Pass: free-account audio plays; background playback survives; RAM/startup recorded. If FairPlay fails only for the full web player, fall back to the official desktop client companion.

**Linux**

1. Build a minimal WebKitGTK app. Check `WebKit.Settings.enable_encrypted_media`; call `requestMediaKeySystemAccess`; expect `NotSupportedError` on stock distros.
2. If a distro build enables EME, check whether any Widevine CDM is discoverable; record the result.
3. Test the fallback: launch installed Chromium/Chrome/Edge in `--app` mode with a dedicated `--user-data-dir`; log in with a free account; confirm playback and ads.
4. Test the companion fallback: read `org.mpris.MediaPlayer2.spotify` metadata and send `PlayPause`/`Next` over D-Bus.
5. Pass for the product only if one fallback meets the agreed Linux scope; embedded WebKitGTK is not expected to pass.

---

## 3. Phase 1A — Spotify terms and policy

> **Not legal advice.** This is a conservative engineering reading of public terms. Spotify can change them or take a different view.

### 3.1 The clauses that matter

**Spotify User Guidelines** ([link](https://www.spotify.com/us/legal/user-guidelines/)) prohibit, among other things:

> "reverse-engineering, decompiling, disassembling, modifying, or creating derivative works, except where such restriction is expressly prohibited by applicable law."

> "circumventing or blocking advertisements or creating or distributing tools designed to block advertisements."

> "deleting or altering any part of the Services or Content except as expressly permitted under the Agreements..."

**Spotify Developer Terms** ([link](https://developer.spotify.com/terms)) prohibit:

> "modifying, editing, altering, creating derivative works, disassembling, decompiling, reverse-engineering, or extracting source code from the Spotify Platform (including any client libraries), Spotify Service, or Spotify Content..."

> "interfering, or attempting to interfere, with the proper functioning or performance of the Spotify Platform or Spotify Service."

> "using any robot, spider, site search/retrieval application, or other tool to retrieve, duplicate, or index" Spotify content.

**Spotify Terms of Use** ([link](https://www.spotify.com/us/legal/end-user-agreement/), last updated 2026-09-04) include:

> "You agree that you will not redistribute, sell or transfer the Spotify Service or the Content."

> Spotify may "provide advertising and other information to you."

**Spotify Design & Branding Guidelines** ([link](https://developer.spotify.com/documentation/design)) require attribution and restrict use of Spotify marks and branding. They do not grant permission to modify the player.

### 3.2 What is defensible

An **unmodified** webview that loads `open.spotify.com`, lets the user log in, and plays audio is closest to a browser. The app does not redistribute the Service, does not extract content, and does not alter the page. Spotify supports Safari, Chrome, Edge, Firefox and Opera; a system webview is an engine-level analogue. This is the only interpretation that fits all of the user's constraints.

It is still **not guaranteed**. Spotify may treat an embedded webview as an unsupported client, block it, or change feature detection. The app must fail gracefully and must not attempt to evade a block.

### 3.3 What is not defensible

| Technique | Why it conflicts |
|---|---|
| CSS injection to restyle or hide Spotify UI | "modifying/altering" the Service; creates a derivative visual work; can hide ads or premium prompts |
| JS injection to add UI, change behavior, or observe the DOM | "modifying/altering"; DOM observation is scraping; can interfere with playback/ads |
| Spicetify `Platform`, `CosmosAsync`, `GraphQL` | internal/undocumented APIs; `CosmosAsync` attaches session tokens/cookies; `GraphQL` is explicitly private |
| Ad blocking, skipping, muting, or hiding | explicitly prohibited by the User Guidelines |
| Audio capture, download, re-streaming | DRM circumvention / content copying; out of scope |
| Modified client patchers (SpotX, Spicetify ad extensions) | modify the client and/or block ads; prohibited |
| CDP automation that reads or clicks the page | "automated means"; scraping; grey at best, and brittle |

### 3.4 Web API is allowed, with limits

The documented Web API can support optional companion features (now-playing display, library browse, search, playlist metadata) if:

- OAuth uses **Authorization Code with PKCE**; no client secret is stored in the binary ([PKCE](https://developer.spotify.com/documentation/web-api/tutorials/code-pkce-flow)).
- Redirect URIs use HTTPS or a loopback IP literal; `localhost` is not allowed; dynamic ports are allowed for loopback ([Redirect URIs](https://developer.spotify.com/documentation/web-api/concepts/redirect_uri)).
- The app requests minimal scopes, respects rate limits, identifies itself, and stores tokens in the OS keychain.
- It is non-commercial and does not become a streaming integration or alter Spotify content. The Web API reference includes policy notes such as "Streaming applications may not be commercial" and "Keep audio content in its original form" ([Start/Resume Playback](https://developer.spotify.com/documentation/web-api/reference/start-a-users-playback)).
- **Playback control is Premium-only.** The same reference says: "This API only works for users who have Spotify Premium." The Web Playback SDK also "requires a Spotify Premium subscription" ([SDK](https://developer.spotify.com/documentation/web-playback-sdk)). A free account cannot be controlled through documented Web API playback endpoints.

### 3.5 Conservative recommendation

1. Host the official web player **unmodified**.
2. Do not inject CSS or JS that changes the page.
3. Do not read cookies, tokens, DOM, or internal endpoints.
4. Keep ads intact.
5. Put all customization in the **native shell** and in **documented Web API** companion features.
6. If Spotify blocks the host or the user wants Spicetify features, use Spicetify with the official desktop client and accept its separate policy risk; do not merge that into this app.

---

## 4. Phase 1B — Spicetify, SpotX and ad avoidance

### 4.1 Spicetify today

- **Version:** v2.46.7, published 2026-11-12; repo active 2026-11-20; 25.6k stars; **LGPL-2.1** ([spicetify/cli](https://github.com/spicetify/cli)).
- **Platforms:** Windows, macOS, Linux. It patches the **official desktop client**, not the web player.
- **Patching:** `spicetify apply` copies raw assets into the client's `Apps` folder, replaces colors, and injects CSS/JS/extensions/custom apps; it requires write access to the client and keeps a backup/restore path ([apply.go](https://github.com/spicetify/cli/blob/main/src/cmd/apply.go)).
- **Themes:** a theme folder contains `color.ini`, `user.css`, optional `theme.js` and assets; `user.css` manipulates/hides/moves UI elements ([themes docs](https://github.com/spicetify/docs/blob/main/docs/development/themes.md)).
- **Extensions:** single JavaScript files run alongside Spotify's main code, access the global `Spicetify` object, and can add UI, modify behavior, and control the player ([extensions docs](https://github.com/spicetify/docs/blob/main/docs/development/extensions.md)).
- **Custom apps:** JavaScript apps add pages to Spotify's sidebar, with `index.js` and `manifest.json`, often using `window.top.Spicetify` ([custom apps docs](https://github.com/spicetify/docs/blob/main/docs/customization/custom-apps.md)).
- **Marketplace:** MIT, 1.6k stars, active 2026-11-23, distributes themes/extensions/apps ([spicetify/marketplace](https://github.com/spicetify/marketplace)).

### 4.2 The API surface is the problem

Spicetify's own documentation is explicit:

- The API wrapper is for interacting with **"Spotify's internal APIs"** ([API wrapper](https://github.com/spicetify/docs/blob/main/docs/development/api-wrapper/index.md)).
- `Spicetify.Player` is "mostly a wrapper of the `Spicetify.Platform.PlayerAPI` object" ([Player](https://github.com/spicetify/docs/blob/main/docs/development/api-wrapper/methods/player.md)).
- `Spicetify.Platform` is "a vast collection of internal APIs"; "because these APIs are internal, they are not guaranteed to be stable and may drastically change" ([Platform](https://github.com/spicetify/docs/blob/main/docs/development/api-wrapper/methods/platform.md)).
- `Spicetify.CosmosAsync` "automatically add[s] the required headers and cookies (such as user session token)" ([CosmosAsync](https://github.com/spicetify/docs/blob/main/docs/development/api-wrapper/methods/cosmos-async.md)).
- `Spicetify.GraphQL` is "a private API" that "is not documented by Spotify" ([GraphQL](https://github.com/spicetify/docs/blob/main/docs/development/api-wrapper/methods/graphql.md)).
- `getAudioData` uses an internal endpoint that "doesn't require authentication" ([getAudioData](https://github.com/spicetify/docs/blob/main/docs/development/api-wrapper/functions/get-audio-data.md)).

A Spicetify-compatible shim would therefore have to:

1. Inject a `Spicetify` global into the page.
2. Reimplement `Player`, `Platform`, `CosmosAsync`, `GraphQL`, `React`, `ReactDOM`, menus, topbar, modals, and the extension lifecycle.
3. Attach the user's session token/cookies to internal requests.
4. Track the desktop client's DOM and React internals.
5. Remain compatible across Spotify client updates.

That is a large, fragile, security-sensitive project that **directly violates the user's no-injection/no-internal-API constraints** and conflicts with the User Guidelines. It is not a "small compatibility layer".

### 4.3 Companion vs API-subset shim vs both

| Approach | What runs | No page injection? | No internal APIs? | Ads intact? | Verdict |
|---|---|---|---|---|---|
| **Companion mode** — native shell + documented Web API + OS media controls; the official player stays in charge | Native UI, OAuth PKCE, SMTC/MPRIS, tray, shortcuts | Yes | Yes | Yes | **Recommended** |
| **API-subset shim** — inject a `Spicetify` object and bridge it to the page | Injected JS + internal `Platform`/`CosmosAsync`/`GraphQL` | No | No | Yes, if no ad code | **Rejected** |
| **Both** | Shim plus companion | No | No | Yes, if no ad code | **Rejected** — adds the shim's risk for no compliant benefit |

**Recommendation: companion mode only.** Define "companion" as: the app never modifies the Spotify page; it adds a native shell and, optionally, documented Web API features. If the user wants existing Spicetify themes/extensions, the correct tool is Spicetify with the official desktop client, not this project.

### 4.4 SpotX and ad avoidance (reported, not implemented)

- SpotX is an MIT-licensed Windows patcher for the official Spotify desktop client, actively maintained (2026-11-24) ([SpotX](https://github.com/SpotX-Official/SpotX)). Its feature set includes ad removal and client modifications.
- Common ad-avoidance approaches in the ecosystem are: client patchers (SpotX, Spicetify ad extensions), modified/cracked clients, DNS or network ad blocking, and Premium. Only **Premium** is sanctioned by Spotify. The User Guidelines explicitly prohibit "circumventing or blocking advertisements or creating or distributing tools designed to block advertisements."
- **This project will not implement any of them.** Ads remain. No DOM hiding, no network blocking, no audio muting, no skip automation. The only supported way to remove ads is Spotify Premium.

---

## 5. Phase 1C — Now playing and control

### 5.1 The options

| Mechanism | Read metadata | Control | Page injection? | Stability | Use |
|---|---|---|---|---|---|
| **Windows SMTC** (`GlobalSystemMediaTransportControlsSessionManager`) | Yes | Yes (play/pause/next/previous) | No | Documented WinRT API | **Primary on Windows** |
| **Linux MPRIS** (`org.mpris.MediaPlayer2`) | Yes | Yes | No | Documented D-Bus spec | **Primary on Linux if the player exposes it** |
| **macOS Now Playing** (`MPNowPlayingInfoCenter` / `MPRemoteCommandCenter`) | Own app only; no public API to read/control another app | Own app only | No | Public API is app-scoped | **Limited; do not rely on it** |
| **Media Session API** (`navigator.mediaSession`) | Yes, if the page sets it | Action handlers, if the page sets them | Requires page context | Web standard; page-dependent | Optional Tier 1 only |
| **DOM observation / clicking** | Yes | Yes | Yes | Brittle; scraping | **Reject** |
| **souvlaki** | Publishes your own metadata/controls | Publishes your own controls | No | 0.9.3 stable; 1.0 rewrite unstable | Use only if the app owns playback |

### 5.2 Windows SMTC

Microsoft's `GlobalSystemMediaTransportControlsSessionManager` "provides access to playback sessions throughout the system that have integrated with SystemMediaTransportControls to provide playback info and allow remote control" ([Microsoft docs](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessionmanager)). Chromium-based engines normally register their media sessions with SMTC; verify this with the Phase 3 spike. If WebView2 does, a native shell can read title/artist/album/artwork/playback status and send transport commands without touching the page. This is the best path on Windows and should be the default.

Caveats: the webview must actually register a media session; some Spotify pages/iframes may register a generic "Spotify" session. Poll at 500 ms while playing and 2 s while paused; debounce state changes; handle session disappearance. Test with the window minimized.

### 5.3 Linux MPRIS

MPRIS is the standard D-Bus interface for media players ([spec](https://specifications.freedesktop.org/mpris-spec/latest/)). If Chromium/Chrome exposes `org.mpris.MediaPlayer2.chromium` (or similar) while Spotify plays, the shell can read `Metadata` and call `PlayPause`, `Next`, `Previous`. If the browser does not expose MPRIS, there is no compliant metadata path; show "Now playing: Spotify" and rely on the browser's own controls. Do not fall back to DOM scraping.

### 5.4 macOS

Apple's public `MPNowPlayingInfoCenter` and `MPRemoteCommandCenter` are for the app's **own** media. There is no public API to read or control another app's Now Playing session; the private `MediaRemote` framework is out of scope. Options:

1. **Synthetic media keys** (best-effort): send the system play/pause/next/previous key events; macOS routes them to the active Now Playing app (the webview). This needs a spike and may require accessibility permissions.
2. **User's keyboard media keys**: always work at the OS level; the app's global shortcuts can be documented as "use the media keys".
3. **No in-app transport/metadata on macOS v1**: the webview's own controls remain; the tray shows show/hide/quit only.

Do not inject JS just to get metadata on macOS. If metadata is a hard requirement on macOS, the compliant fallback is the official Spotify desktop client companion, not page scraping.

### 5.5 Media Session and DOM

`navigator.mediaSession` exposes `metadata` and `playbackState`, and `setActionHandler` for play/pause/next/previous ([MDN](https://developer.mozilla.org/en-US/docs/Web/API/MediaSession)). But the page owns the session. Reading it from native requires executing JS in the page (injection or CDP), and Slopify found that Spotify's SDK media element lives in a cross-origin iframe, so top-frame `navigator.mediaSession` was ignored and WebKit's Now Playing entry was generic ([Slopify #22](https://github.com/jnsdls/slopify/issues/22)). DOM observation is worse: `data-testid` selectors are undocumented, brittle, and scraping.

**Recommendation:** Tier 0 (OS media sessions) only in v1. A read-only Media Session probe can be a later opt-in experiment, clearly labelled as page-context access, but it is not needed for the core product and should not be in the default build.

### 5.6 Tray, mini-player, global shortcuts

- **Tray:** native menu with Show/Hide, Play/Pause, Next, Previous, Settings, Quit. On Windows/Linux, transport items are enabled only when an OS media session is available. On macOS, transport items may be disabled in v1.
- **Mini-player:** a small native window with artwork, title, artist, progress (if available), and transport. It reads from the OS media session, not the page. Close it to save RAM; do not keep a second webview alive.
- **Global shortcuts:** `tauri-plugin-global-shortcut` 2.4.1 (or `global-hotkey` directly). Defaults: `Ctrl/Cmd+Alt+Space` play/pause, `Ctrl/Cmd+Alt+Right` next, `Ctrl/Cmd+Alt+Left` previous. Make them configurable; detect conflicts; never override OS-reserved media keys.
- **souvlaki 0.9.3** supports Windows, macOS and Linux/MPRIS for publishing metadata and receiving controls ([souvlaki](https://github.com/Sinono3/souvlaki)). It is useful if the app owns playback; it is not a reader for another app's session. The 1.0 rewrite is unstable; pin 0.9.x.

---

## 6. Phase 1D — Stack choice

### 6.1 Comparison

| Option | Installer | RAM | Startup | Security model | Wayland | Dev cost | Verdict |
|---|---|---|---|---|---|---|---|
| **Tauri 2.12.7 + system webview** | Small; no Chromium bundled; runtime is OS-provided | Dominated by the webview; Tauri core is small | Native window fast; webview init dominates | Capabilities/permissions; remote URLs must be explicitly granted; can give the Spotify window **zero** capabilities | GTK/WebKitGTK; works, with WebKitGTK caveats | Medium; plugins for tray, updater, shortcuts, window state, single instance | **Recommended** |
| **Raw `wry` 0.58.1 + `tao` 0.38.0** | Smallest control; no Tauri IPC at all | Similar; webview dominates | Similar | No IPC surface by construction; you implement navigation policy | Child webview under Wayland is problematic in some setups | High; reimplement tray, updater, shortcuts, window state, packaging | Alternative if Tauri's remote-content model is unacceptable |
| **CEF / castlabs** | 300+ MB | 188 MB helpers before content | Slow | Mature Chromium sandbox; complex | Yes | High; licensing | **Rejected** |
| **Electron** | 100+ MB | High | Slow | Mature | Yes | Low | **Rejected** — bundled Chromium and size |

### 6.2 Why Tauri 2

- It provides the native window, tray, global shortcuts, single-instance, window-state, updater and packaging that would otherwise be reimplemented.
- Its capability system lets us **deny all IPC to the remote Spotify window**. Tauri's config schema documents a `remote` field for capabilities, with a caution: "Make sure you understand the security implications of providing remote sources with local system access" ([schema](https://schema.tauri.app/config/2), [capabilities](https://v2.tauri.app/security/capabilities/)). We will not use it for the Spotify origin.
- It uses the system webview, so no Chromium is bundled and the installer stays small.
- Linux prerequisites are documented (`libwebkit2gtk-4.1-dev` on Debian/Ubuntu, `webkit2gtk-4.1` on Arch, etc.) ([prerequisites](https://v2.tauri.app/start/prerequisites/)). This matters for packaging, even though embedded Linux playback is out of scope.

### 6.3 Why not raw wry+tao

Raw `wry`+`tao` is attractive for a zero-IPC design and the smallest dependency surface. But it pushes tray, updater, shortcuts, window state, single-instance, code signing and packaging onto this project. Tauri's core overhead is small relative to the webview, and its capability system gives a defensible security boundary. Choose raw wry only if the Phase 3 spike shows Tauri's remote-content handling is a problem.

### 6.4 Shell frontend

- **Recommended:** a tiny **vanilla TypeScript/HTML/CSS** settings and mini-player UI, loaded from local assets in a Tauri webview. No React/Vue; no remote scripts; CSP locked down. Create the settings window on demand and close it to save RAM.
- **Native UI (egui/iced/Slint):** lower RAM and no extra webview, but integrating a child webview with a native UI is complex, especially on Wayland. Not worth it for v1. Revisit for the Linux companion, which has no webview.
- **React/Vue:** unnecessary bundle, startup and dependency cost.

### 6.5 Recommendation

**Tauri 2 + one remote Spotify webview with no capabilities + native tray/menu + vanilla TS settings/mini-player.** Windows 10/11 v1. macOS and Linux are out of scope; the earlier macOS/Linux notes describe possible later ports, not v1 deliverables.

---

## 7. Phase 1E — Security design

### 7.1 Threat model

| Threat | Impact | Mitigation |
|---|---|---|
| Spotify page is compromised or serves malicious JS | Attempts to call native APIs | Spotify window has **no Tauri capabilities**; `withGlobalTauri=false`; no custom protocol; no file/shell access; no IPC commands |
| Malicious navigation (link, redirect, OAuth) | Phishing or data exfiltration | Strict top-level allowlist; unknown links open in the system browser; block `file:`, `javascript:`, `data:`; validate redirects |
| Popup abuse | Hidden windows, credential theft | Dedicated popup webview with no capabilities; allow only auth domains; close on return; block downloads |
| OAuth interception | Account takeover | PKCE; `state` validation; loopback listener on `127.0.0.1`; random port; one-shot; no `localhost`; no token in URL |
| Token theft at rest | Account access | Refresh token in OS keychain (`keyring`); access token in memory; zeroize; no logs |
| Update compromise | Arbitrary code execution | Tauri updater signatures; HTTPS; pinned public key; staged rollout; rollback protection |
| Supply chain | Malicious dependency | `cargo-deny`, `cargo-audit`, minimal crates, lockfile, review |
| Local malware | Reads app data | OS keychain; no plaintext tokens; no cookies exported; standard OS file permissions |
| Denial of service | Crash/hang | Rate limits; timeouts; no unbounded queues; graceful degradation |

### 7.2 Navigation allowlist

Allow top-level navigation only to:

- `https://open.spotify.com/*`
- `https://accounts.spotify.com/*`
- `https://www.spotify.com/*` (help/legal)
- Auth popups, in a separate zero-IPC webview: `https://accounts.google.com/*`, `https://appleid.apple.com/*`, `https://www.facebook.com/*` (only if social login is supported)
- Challenge pages only if Spotify requires them: `https://challenges.cloudflare.com/*`

Everything else opens in the system browser. Block `file://`, `javascript:`, `data:`, `blob:` top-level navigation, and all downloads. Disable permission requests (mic, camera, notifications, geolocation) by default.

### 7.3 IPC policy

- **Remote Spotify window: zero IPC.** Do not add it to any capability. Do not enable `remote.urls` for it. Do not expose `window.__TAURI__`.
- **Local settings/mini-player window:** only the commands it needs, with typed arguments and scoped permissions.
- **No `eval` of page-provided strings.** No dynamic script construction from remote data.
- **If a read-only page probe is ever added:** inject a fixed script with `with_initialization_script`; post a fixed JSON schema; validate origin, version, size and rate; never execute page-provided code. Treat it as a separate security review.

### 7.4 CSP for the local UI

A strict CSP, for example:

```
default-src 'self';
script-src 'self';
style-src 'self' 'unsafe-inline';
img-src 'self' data: https://i.scdn.co https://mosaic.scdn.co;
connect-src 'self' https://api.spotify.com https://accounts.spotify.com;
frame-ancestors 'none';
```

The remote Spotify page has its own CSP; the app cannot and should not override it.

### 7.5 OAuth and token storage

1. Generate a PKCE `code_verifier` and `code_challenge` (S256).
2. Open the system browser (or a dedicated auth webview) to `https://accounts.spotify.com/authorize` with `client_id`, `response_type=code`, `redirect_uri=http://127.0.0.1:PORT/callback`, `state`, and minimal scopes.
3. Listen on `127.0.0.1:PORT`; accept only `GET /callback`; validate `state`; exchange the code at `https://accounts.spotify.com/api/token` with `client_id` and `code_verifier` (no secret).
4. Store the refresh token in the OS keychain via `keyring` 4.2.0; keep the access token in memory; refresh before expiry; zeroize on drop.
5. On logout, delete the keychain item and clear the Web API session; do not clear the Spotify webview cookies unless the user asks.

Spotify requires HTTPS except loopback IP literals, and `localhost` is not allowed; use `http://127.0.0.1:PORT/callback` and register the loopback without a port ([Redirect URIs](https://developer.spotify.com/documentation/web-api/concepts/redirect_uri)).

If a **localhost bridge** is ever added for a browser extension or a companion process, it must:

- Bind only to `127.0.0.1` (never `0.0.0.0`), on a random port.
- Require a random 256-bit token in a header or the first message; reject missing/incorrect tokens.
- Check the `Origin` header against an explicit allowlist (the extension's `chrome-extension://` ID or the companion's origin); reject `null` and wildcard origins.
- Accept one short-lived session, enforce a timeout, and shut down after the handshake or on app exit.
- Never expose playback tokens, cookies, or arbitrary command execution; expose only typed, minimal operations.

This is defense in depth; the default design has no bridge at all.

### 7.6 Updates and kill switch

- Tauri updater with signed manifests; HTTPS only; public key pinned in the app; `createUpdaterArtifacts` in CI.
- Channels: stable and beta. Staged rollout (e.g., 10% → 50% → 100%) with a remote manifest.
- **Kill switch:** a signed remote config that can disable optional companion features or show a maintenance message. It must not be able to enable new native capabilities.
- No auto-update of the webview runtime; WebView2 Evergreen handles itself; macOS WebKit is OS-updated.
- Keep the previous version's installer for rollback; document manual rollback.

### 7.7 Logging and privacy

- `tracing` with structured logs; redact query strings, tokens, cookies and email addresses.
- No telemetry by default. Opt-in crash reports with a clear notice.
- Log rotation and a maximum size.
- A "What we collect" page that says: nothing by default; OAuth tokens stay in the keychain; no page content is read.

### 7.8 Dependency and code policy

- `#![forbid(unsafe_code)]` in pure crates; any `unsafe` needs a justification comment and a test.
- No `unwrap()` outside tests; typed `thiserror` errors.
- `cargo-deny` for licenses and advisories; `cargo-audit`; `cargo fmt --check`; `cargo clippy -- -D warnings`.
- Unit tests for URL policy, OAuth state, config parsing, media-session state; integration tests with a mocked local page for the bridge (if added).

---

## 8. Phase 2 — Design

### 8.1 Product definition

**Rustify v1 is:**

- A Windows 10/11 and macOS 13+ desktop app that hosts the **unmodified** official Spotify web player.
- Free or Premium account; the official player handles audio, ads and DRM.
- Native tray, mini-player, global shortcuts, window-state restore and native theming.
- Optional documented Web API companion: now-playing display, library browse, search, playlist metadata, via PKCE and the OS keychain.
- OS media-session integration: SMTC on Windows; MPRIS on Linux if companion mode; best-effort on macOS.
- No bundled Chromium; no page injection; no internal APIs; no ad handling; no offline/download; no DRM circumvention; no audio capture, ripping or re-streaming; no YouTube or other audio sources.

**Rustify v1 is not:**

- A Spicetify replacement or a host for Spicetify themes/extensions.
- A Spotify Connect controller for free accounts (documented playback control requires Premium).
- A Linux embedded webview player.
- A way to remove ads or unlock Premium features.

### 8.2 Module layout

```
rustify/
  Cargo.toml                     # workspace
  crates/
    rustify-core/                # domain types, config, URL policy, errors, state machine
    rustify-platform/            # OS media sessions, keychain, autostart, single instance
    rustify-oauth/               # PKCE, loopback listener, token refresh (Web API)
    rustify-app/                 # Tauri app, windows, tray, commands, updater
    rustify-ui/                  # local settings/mini-player (vanilla TS/HTML/CSS)
  docs/
    PHASE-0-2-REPORT.md
    ARCHITECTURE.md
    SECURITY.md
    LIMITATIONS.md
    TESTING-DRM.md
  .github/workflows/ci.yml
```

Optional later: `rustify-pagebridge` for a read-only Media Session probe, behind a feature flag and a separate security review. Not in v1.

### 8.3 Threading and data flow

- **Main thread:** Tauri event loop, window creation, tray/menu, native dialogs.
- **Tokio runtime:** OAuth HTTP, Web API calls, update checks, media-session polling, config I/O.
- **Webview callbacks:** never block; send events through `tokio::sync::mpsc`/`watch`.
- **Media session:** Windows WinRT calls on a dedicated thread with the right apartment initialization; Linux `zbus` async; macOS synthetic-key task only if implemented.
- **State:** `Arc<RwLock<AppState>>` for small snapshots or an actor with channels; never hold a lock across `.await`.
- **Shutdown:** cancellation token; close webview; flush logs; save window state.

Startup sequence:

1. Single-instance check.
2. Load config and window state.
3. Create the main window with the remote Spotify webview (no capabilities).
4. Start media-session polling and tray.
5. If the user has connected Web API, refresh the token in the background.
6. Show the window; optionally restore the last URL (only allowlisted Spotify URLs).

### 8.4 Webview, session and navigation

- **Persistent data:** WebView2 user data folder, WKWebView data store, WebKitGTK website data manager. The webview owns the Spotify session; the app never extracts cookies or tokens.
- **Navigation handler:** allowlist as in §7.2; unknown top-level URLs open in the system browser.
- **New-window handler:** auth popups only, in a dedicated zero-IPC webview; all other popups go to the system browser.
- **Downloads:** denied.
- **Permissions:** denied by default.
- **DevTools:** disabled in release; enabled only in debug builds.
- **User agent:** engine default first; if Spotify rejects it, use a standard desktop UA and document it. Do not spoof to evade a block.
- **Zoom:** allow 80–150%; persist per window.
- **Theme:** follow OS dark/light; the Spotify page uses its own theme. Native chrome uses a token file.

### 8.5 IPC and bridge schemas

**v1: no remote IPC.** The Spotify webview cannot call native commands. This is the central security property.

Local UI commands (Tauri, typed):

```rust
#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<Settings, AppError>;

#[tauri::command]
async fn set_settings(state: State<'_, AppState>, settings: Settings) -> Result<(), AppError>;

#[tauri::command]
async fn connect_spotify(state: State<'_, AppState>) -> Result<OAuthStatus, AppError>;

#[tauri::command]
async fn disconnect_spotify(state: State<'_, AppState>) -> Result<(), AppError>;

#[tauri::command]
async fn get_now_playing(state: State<'_, AppState>) -> Result<Option<NowPlaying>, AppError>;

#[tauri::command]
async fn open_external(url: String) -> Result<(), AppError>; // parse + allow http/https only
```

All commands validate inputs and return `thiserror` errors. No command accepts a shell string, a file path from remote content, or a raw URL without parsing.

**Optional future page probe (not v1):**

```json
{
  "v": 1,
  "type": "media",
  "metadata": {
    "title": "string",
    "artist": "string",
    "album": "string",
    "artwork": "https://i.scdn.co/..."
  },
  "playbackState": "playing|paused|none",
  "positionMs": 0
}
```

Validation: origin must be `https://open.spotify.com`; `v` must be 1; strings capped at 512 bytes; artwork URL must be `https://i.scdn.co` or `https://mosaic.scdn.co`; rate limit 1 message/s; ignore unknown fields; never `eval`.

### 8.6 OAuth and Web API companion

- Scopes: `user-read-playback-state`, `user-read-currently-playing`, `user-library-read`, `playlist-read-private`, `playlist-read-collaborative`. No write scopes in v1.
- Endpoints: `/v1/me/player`, `/v1/me/player/currently-playing`, `/v1/me/tracks`, `/v1/me/playlists`, `/v1/search`.
- Rate limits: exponential backoff on 429; respect `Retry-After`; cache metadata for 30 s; no polling faster than 5 s when playing, 30 s when paused.
- Free accounts: read endpoints are not marked Premium-only; playback control is Premium-only. Do not show disabled controls as if they should work; explain "Playback control requires Premium; use the player or media keys."
- The Web API is optional. The app must work without it (host + OS media controls).

### 8.7 OS media integration

- **Windows:** `GlobalSystemMediaTransportControlsSessionManager` via the `windows` crate. Enumerate sessions; prefer the WebView2 session; read `MediaProperties` and `PlaybackInfo`; `TryPlayAsync`, `TryPauseAsync`, `TrySkipNextAsync`, `TrySkipPreviousAsync`. Handle session change events.
- **Linux (companion):** `zbus` to `org.mpris.MediaPlayer2.spotify`; read `Metadata` and `PlaybackStatus`; call `PlayPause`, `Next`, `Previous`. If no session, disable controls.
- **macOS:** no public cross-app API. Try synthetic media keys in a spike; otherwise disable in-app transport and document media keys. Do not use private `MediaRemote`.
- **souvlaki:** only if the app ever owns playback (it does not in host mode). Pin 0.9.3 if used.

### 8.8 Theming

- Theme tokens in `theme.json`: background, surface, text, muted, accent, border, radius, font.
- Apply to native tray/menu (where the OS allows), the local settings window and the mini-player.
- Follow OS dark/light and accent color by default.
- **No CSS injection into Spotify.** No Spotify page colors, no `user.css`, no `color.ini` compatibility.
- Optional: derive an accent color from album artwork fetched via the documented Web API or OS media session. This does not modify Spotify.

### 8.9 Update strategy

- Tauri updater, signed manifests, HTTPS, pinned public key.
- Versioned config schema with migrations; unknown fields ignored; defaults for missing fields.
- Feature flags for optional companion features; a signed remote kill switch can disable them.
- No DOM selectors, so no selector versioning is needed. If a read-only page probe is added later, version its schema and feature-detect; degrade to OS media only.
- The Spotify web player is server-updated; the app must tolerate UI changes because it does not depend on them.
- If Spotify blocks the webview, show a clear message and offer the official client companion; do not attempt to evade.

### 8.10 Performance measurement plan

| Metric | Tool | Method | Proposed budget |
|---|---|---|---|
| RAM (Windows) | `Get-Counter`/`tasklist`, ETW; sum private working set and commit for the app + WebView2 children | 60 s after playback; minimized and visible | Shell idle ≤120 MB PSS-equivalent; playing ≤450 MB |
| RAM (macOS) | `footprint`, `vmmap -summary`, `ps -o rss` | Sum app, WebContent, GPU, Networking | Playing ≤450 MB |
| RAM (Linux companion) | `smem -P`, PSS | Whole tree | ≤200 MB (no webview) |
| CPU idle | `pidstat`, `perf stat`, `powermetrics`, `typeperf` | 5 min paused, hidden; 5 min playing | Paused ≤2% of one core; playing recorded, not budgeted as idle |
| Startup | `tracing` timestamps + `hyperfine` | Process start → first window paint; cold and warm | ≤500 ms shell window; content load separate |
| Binary size | `cargo bloat --release --crates`, `size`, installer `du` | Release build | Installer ≤30 MB Win/macOS; Linux package ≤10 MB excluding system WebKitGTK |
| Leaks | `heaptrack`, `dhat`, Instruments | 30 min playback + pause/resume | No unbounded growth |
| Regression | CI benchmark on a mocked local page | Window creation + RAM | Fail on >10% regression |

**Definitions matter.** "Idle" means paused and not playing; "startup" means the native window is visible, not that Spotify is interactive. Publish both shell and content numbers.

### 8.11 CI

- Matrix: Windows, macOS, Ubuntu.
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo deny check` (licenses, advisories, bans, sources)
- `cargo audit`
- `cargo build --release` and installer build on tags
- Unit tests: URL allowlist, OAuth state/PKCE, config migrations, media-session state machine
- Integration tests: local mock page for the optional bridge (if added); no network in CI
- Manual/nightly: GUI smoke test on each OS; DRM probe on a real machine (cannot run in CI without a Spotify account and DRM)

### 8.12 Roadmap and acceptance criteria

**3.1 Workspace and CI** — workspace, crates, logging/config, CI gates.
- Accept: CI green on all three OSes; `cargo deny` clean; config schema with migrations; no `unwrap()` outside tests; every dependency has a one-line justification in `ARCHITECTURE.md`.

**3.2 Webview host, allowlist, persistent session** — Tauri window, remote webview, navigation policy, session persistence.
- Accept: free-account login persists across restart; external links open in browser; no IPC from the Spotify origin; devtools off in release; DRM probe passes on Windows.

**3.3 Playback/media integration** — SMTC/MPRIS, tray transport, global shortcuts.
- Accept: Windows SMTC reads metadata and controls playback; Linux companion MPRIS works; macOS media keys documented; no page injection; state updates within 1 s.

**3.4 Tray, mini-player, window state, native theme** — native UI.
- Accept: tray menu, mini-player, shortcuts configurable, window state restored, theme tokens applied, no extra webview when mini-player is closed.

**3.5 Theming** — **changed**: native theme tokens only; no CSS injection.
- Accept: theme JSON applies to local UI; no Spotify page modification; safe reload/revert of native theme.

**3.6 Companion features** — **changed**: documented Web API PKCE module and a sample feature (e.g., now-playing/library search); no Spicetify shim.
- Accept: PKCE connect, token in keychain, API calls with minimal scopes, 429 handling, logout clears token; no internal endpoints; no page injection.

**3.7 Packaging, updates, docs** — MSI/portable, dmg, Linux companion package (deb/Flatpak/AUR), updater, docs.
- Accept: installers built and measured; signed updates verified; README, ARCHITECTURE, SECURITY, LIMITATIONS, TESTING-DRM; rollback documented.

**Phase 3 gate:** run the Phase 0 spike (§2.8) before 3.2. If free-account playback or background playback fails on a target OS, stop and choose companion mode for that OS.

---

## 9. Performance reality check

### 9.1 Measurements found

| Measurement | Value | Source |
|---|---|---|
| WKWebView + Spotify Web Playback SDK, macOS | ~210 MB RSS total (Rust 77, GPU 58, WebContent 45, Networking 27) | [Slopify #22](https://github.com/jnsdls/slopify/issues/22) |
| Chromium + Spotify web player, Linux, PSS | 381 MB total; app 77 MB; Spotify page 219 MB; other renderers 32 MB; browser/GPU/network/CDM 52 MB | [Spotish PR #31](https://github.com/huacnlee/spotish/pull/31) |
| Chromium + Spotify web player, Linux, RSS | 806 MB total (RSS double-counts shared pages) | [Spotish README](https://github.com/huacnlee/spotish) |
| Empty WebView2 window, Windows | 68 MB private working set without chrome view; 89 MB with | [Lightweight Browser PR #6](https://github.com/TheYungStump/lightweight-browser/pull/6) |
| CEF helpers, no browser card | ~188 MB across five helper processes | [Infiniterm #761](https://github.com/ekinertac/infiniterm/issues/761) |
| CEF bundle | 317 MB of 367 MB; 136 MB of 154 MB update zip | [Infiniterm #761](https://github.com/ekinertac/infiniterm/issues/761) |

### 9.2 Conclusion

The original `<150 MB with player running` budget is **not achievable** with any browser-engine approach that plays the official web player. The web player alone is ~219 MB PSS in Chromium; a WKWebView Spotify session is ~210 MB RSS; an empty WebView2 window is 68–89 MB before Spotify loads. Even a 77 MB native shell plus a 219 MB page is 296 MB. CEF is worse.

### 9.3 Proposed revised budgets

| Metric | Proposed budget | Rationale |
|---|---|---|
| Shell idle, no Spotify page | ≤120 MB PSS | Native shell + one webview process; measure |
| Player loaded, paused | ≤300 MB PSS | Page + engine baseline |
| Player playing | ≤450 MB PSS | Page + media buffers + artwork |
| CPU paused, hidden | ≤2% of one core, 5 min average | Spotify UI may animate; 1% may be unattainable |
| CPU playing | Record only; no hard cap | Audio decoding is real work |
| Shell window visible | ≤500 ms cold/warm | Native window, not Spotify interactive |
| Spotify content interactive | Record only | Network/server dependent |
| Installer | ≤30 MB Windows x64 (WebView2 runtime counted separately) | Tauri/system webview; measure |
| No bundled Chromium | Required | Tauri/wry or external browser |

If these budgets are not acceptable, the correct decision is to **stop**, not to pretend the original targets can be met.

---

## 10. Risks and open questions

1. **WebView2 Widevine is not guaranteed.** It works in practice today but can break with a CDM/runtime update. Mitigation: runtime probe, Evergreen, fallback to external Edge/Chrome or companion.
2. **macOS full free-account web player is not directly verified.** FairPlay works in WKWebView for the Premium SDK; the full web player, UA acceptance and background playback need a spike.
3. **Background playback on macOS may stop after a track.** Crest reports this in a tabbed browser. A single hidden/occluded webview may behave differently; test with App Nap and window occlusion.
4. **Spotify may block embedded webviews or change feature detection.** Do not evade; show a clear message and fall back.
5. **The legal position is grey.** Unmodified hosting is the most defensible reading, but Spotify may disagree. No injection, no internal APIs, no ad handling.
6. **Spicetify compatibility is impossible within the constraints.** Existing themes/extensions assume internal APIs and page modification. If this is a hard requirement, the project must be repositioned and the constraints relaxed.
7. **Linux has no embedded path.** WebKitGTK lacks a Widevine CDM; external Chromium or official-client companion are the only options.
8. **macOS has no public cross-app Now Playing API.** In-app metadata/transport may be limited; do not use private frameworks.
9. **Installer size on Windows 10 may include the WebView2 runtime.** The app installer can be small, but the runtime is a separate download/install. Measure the total first-run footprint.
10. **Wayland and packaging.** Tauri/GTK works, but WebKitGTK versions and Flatpak runtimes vary. Linux companion mode avoids the webview entirely.
11. **Code signing/notarization.** Windows SmartScreen and macOS Gatekeeper require certificates/notarization for a smooth first run; budget for it.
12. **The web player is a moving target.** Because the app does not depend on DOM selectors, UI changes are tolerable; DRM/UA changes are not. Keep the fallback path.

---

## 11. Approval requested

Please decide these five points before Phase 3:

1. **Performance budget.** Accept the revised budgets in §9.3, or stop. The original `<150 MB` target cannot be met with the official web player.
2. **Customization scope.** Accept **no Spicetify-style page theming/extensions**; native-shell theming plus documented Web API companion features only. If Spicetify compatibility is required, the constraints must be relaxed and the policy risk accepted.
3. **Platform scope.** ~~Windows + macOS v1; Linux as companion/external browser.~~ **Resolved 2026-11-25: Windows 10/11 x64 only for v1.**
4. **Stack.** Tauri 2 (recommended) or raw `wry`+`tao`. Tauri gives capabilities, tray, updater, shortcuts and packaging; raw wry gives a smaller IPC surface but more custom code.
5. **Phase 3 gate.** Approve a short spike first: free-account playback, ads, background playback, DRM probe, RAM/startup on Windows. Only then build 3.1–3.7.

If approved as recommended, Phase 3 starts with 3.1 (workspace/CI) and the DRM spike in parallel; 3.5 and 3.6 are redefined as native theming and documented Web API companion features, not Spicetify injection.

---

## Appendix A — Sources

**Spotify**

- Web player help (supported browsers, Widevine guidance): https://support.spotify.com/us/article/web-player-help/
- Terms of Use (updated 2026-09-04): https://www.spotify.com/us/legal/end-user-agreement/
- User Guidelines: https://www.spotify.com/us/legal/user-guidelines/
- Developer Terms: https://developer.spotify.com/terms
- Design & Branding Guidelines: https://developer.spotify.com/documentation/design
- Web API — Start/Resume Playback (Premium-only): https://developer.spotify.com/documentation/web-api/reference/start-a-users-playback
- Web Playback SDK (Premium-only): https://developer.spotify.com/documentation/web-playback-sdk
- Authorization Code with PKCE: https://developer.spotify.com/documentation/web-api/tutorials/code-pkce-flow
- Redirect URIs (HTTPS/loopback): https://developer.spotify.com/documentation/web-api/concepts/redirect_uri

**Microsoft / WebView2**

- WebView2 process model: https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-model
- WebView2 distribution: https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution
- Evergreen vs Fixed Version: https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/evergreen-vs-fixed-version
- Widevine feature request: https://github.com/MicrosoftEdge/WebView2Feedback/issues/4828
- False-alarm issue: https://github.com/MicrosoftEdge/WebView2Feedback/issues/1291
- SMTC session manager: https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssessionmanager

**Apple / macOS**

- App Nap: https://developer.apple.com/library/archive/documentation/Performance/Conceptual/power_efficiency_guidelines_osx/AppNap.html
- Slopify FairPlay spike: https://github.com/jnsdls/slopify/issues/22
- Slopify ADR 0002: https://github.com/jnsdls/slopify/blob/main/docs/adr/0002-player-in-wkwebview-ui-in-gpui.md
- Tauri macOS DRM issue (Widevine, older): https://github.com/tauri-apps/tauri/issues/7400
- Crest background playback: https://github.com/crest-browser/Crest/issues/577
- Ardia Spotify/WebKit: https://github.com/ardia-browser/ardia/issues/4068
- OpenArch FairPlay: https://github.com/dpfaffenbauer/OpenArch/pull/33
- ilo Spotify/WebKit: https://github.com/MartinPTielemans/ilo/issues/3432

**Linux / WebKitGTK**

- WebKitGTK CMake options: https://github.com/WebKit/WebKit/blob/main/Source/cmake/OptionsGTK.cmake
- `enable_encrypted_media` (build-time): https://valadoc.org/webkitgtk-6.0/WebKit.Settings.enable_encrypted_media.html
- Spotish (external Chromium, free/Premium, ads intact): https://github.com/huacnlee/spotish
- Spotish memory benchmark: https://github.com/huacnlee/spotish/pull/31
- MPRIS spec: https://specifications.freedesktop.org/mpris-spec/latest/
- Pulse "no Widevine on WebKitGTK": https://github.com/GregHofmeyr/pulse-app/issues/276
- Syrinx Linux/WebKitGTK: https://github.com/Fledge-Software/Syrinx-app/issues/210

**CEF / castlabs**

- cef-rs: https://github.com/tauri-apps/cef-rs
- cef crate: https://crates.io/crates/cef
- tauri-runtime-cef: https://crates.io/crates/tauri-runtime-cef
- castlabs Widevine certification: https://castlabs.com/security/widevine-certification/
- Google Widevine overview (licence required): https://developers.google.com/widevine/drm/overview
- CEF size measurement: https://github.com/ekinertac/infiniterm/issues/761

**Spicetify / SpotX**

- Spicetify CLI: https://github.com/spicetify/cli
- Spicetify apply: https://github.com/spicetify/cli/blob/main/src/cmd/apply.go
- Spicetify API wrapper: https://github.com/spicetify/docs/blob/main/docs/development/api-wrapper/index.md
- Spicetify Player: https://github.com/spicetify/docs/blob/main/docs/development/api-wrapper/methods/player.md
- Spicetify Platform: https://github.com/spicetify/docs/blob/main/docs/development/api-wrapper/methods/platform.md
- Spicetify CosmosAsync: https://github.com/spicetify/docs/blob/main/docs/development/api-wrapper/methods/cosmos-async.md
- Spicetify GraphQL: https://github.com/spicetify/docs/blob/main/docs/development/api-wrapper/methods/graphql.md
- Spicetify themes: https://github.com/spicetify/docs/blob/main/docs/development/themes.md
- Spicetify extensions: https://github.com/spicetify/docs/blob/main/docs/development/extensions.md
- Spicetify custom apps: https://github.com/spicetify/docs/blob/main/docs/customization/custom-apps.md
- Spicetify Marketplace: https://github.com/spicetify/marketplace
- SpotX: https://github.com/SpotX-Official/SpotX

**Tauri / webview / Rust**

- Tauri capabilities: https://v2.tauri.app/security/capabilities/
- Tauri config schema (`remote`): https://schema.tauri.app/config/2
- Tauri prerequisites: https://v2.tauri.app/start/prerequisites/
- Tauri process model: https://v2.tauri.app/concept/process-model/
- Tauri app size: https://v2.tauri.app/concept/size/
- Tauri updater: https://v2.tauri.app/plugin/updater/
- Tauri global shortcut plugin: https://v2.tauri.app/plugin/global-shortcut/
- Tauri window-state plugin: https://v2.tauri.app/plugin/window-state/
- Tauri single-instance plugin: https://v2.tauri.app/plugin/single-instance/
- Tauri 2 crate: https://crates.io/crates/tauri
- wry: https://crates.io/crates/wry
- tao: https://crates.io/crates/tao
- souvlaki: https://github.com/Sinono3/souvlaki
- keyring: https://crates.io/crates/keyring
- Apple MPNowPlayingInfoCenter: https://developer.apple.com/documentation/mediaplayer/mpnowplayinginfocenter
- Apple MPRemoteCommandCenter: https://developer.apple.com/documentation/mediaplayer/mpremotecommandcenter
- Chromium `remote-debugging-pipe`: https://chromium.googlesource.com/chromium/src/+/refs/heads/main/content/public/common/content_switches.cc
- Media Session API: https://developer.mozilla.org/en-US/docs/Web/API/MediaSession

**Third-party empirical reports**

- Lightweight Browser Windows spike (Spotify in WebView2; memory): https://github.com/TheYungStump/lightweight-browser/pull/6
- MixMusic hidden WebView2 Spotify: https://github.com/fzn0x/mixmusic/issues/930
- Cider 4.0.31 Widevine/PlayReady: https://cider.sh/changelogs/4.0.31

---

## Appendix B — Manual DRM/playback test scripts

### B.1 Windows WebView2 probe

```powershell
# Build the spike (tao + wry), then run it with a clean data dir.
cargo run --release -- --data-dir "$env:LOCALAPPDATA\rustify-spike"
```

In the page console:

```js
(async () => {
  const ua = navigator.userAgent;
  let wv = "unknown";
  try {
    const a = await navigator.requestMediaKeySystemAccess("com.widevine.alpha", [{
      initDataTypes: ["cenc"],
      audioCapabilities: [{ contentType: 'audio/mp4; codecs="mp4a.40.2"' }]
    }]);
    wv = a.keySystem;
  } catch (e) { wv = "FAIL: " + e.name + " " + e.message; }
  console.log(JSON.stringify({ ua, wv }));
})();
```

Pass criteria: `wv === "com.widevine.alpha"`; a free-account track plays; an ad plays; no uncaught errors; RAM recorded.

### B.2 macOS WKWebView probe

```bash
cargo run --release -- --data-dir "$HOME/Library/Application Support/rustify-spike"
```

In Web Inspector, filter Network for `fairplay` and `license`. Pass criteria: certificate 200, licence 200, playback position advances, next track plays after minimizing/occluding.

### B.3 Linux WebKitGTK probe

```bash
# Check the build flag first.
pkg-config --modversion webkit2gtk-4.1
# Run the spike; expect EME to be unavailable on stock distros.
```

Pass criteria for the **fallback**: installed Chromium/Chrome/Edge in `--app` mode plays a free account with ads; MPRIS metadata/control works for the companion.

### B.4 CEF size check

```bash
du -sh path/to/cef-bundle
# Compare with the 367 MB reference and the 30 MB installer budget.
```

Pass criteria: none for this project; the check documents why CEF is rejected.

---

*End of report. Awaiting approval before Phase 3.*
