# Rustify

**A native Windows host for the official Spotify Web Player.**

Rustify loads `https://open.spotify.com/` inside a Rust-controlled WebView2 window. The official web player keeps ownership of sign-in, playback, DRM, the queue, and advertising; Rustify supplies only the desktop shell around it.

> **Status: research prototype.** Phase 4 is complete; V1 has not been built or approved. This repository contains the Phase 3 Tauri spike, the Phase 4 measurement harness, and the research reports.

## Scope and principles

- **Official player, unmodified playback path.** Spotify serves the page, the audio, the ads, and the DRM licence. Rustify does not proxy, rewrite, or re-stream any of it.
- **Free accounts work, with ads intact.** Advertisements are not blocked, skipped, muted, or filtered.
- **No ad blocking, no DRM circumvention, no audio extraction, no credential handling.**
- **No page injection in the measured build.** The only injection in the tree is an opt-in benchmark flag that hides two Spotify upsell elements; it is off by default and outside the measured configuration.
- **Windows 10/11 x64 only** for now. macOS and Linux are out of scope for V1.

Rustify is an independent project. It is not affiliated with, endorsed by, or supported by Spotify AB.

## Measured result

All numbers are PSS (proportional set size) for the full process tree, on one Windows 11 machine, with a signed-in free account. Lower is better. The budgets were paused ≤300 MB and playing ≤400 MB.

| Host | Paused | Playing | Notes |
|---|---:|---:|---|
| Official Spotify desktop client | 754.6 MB | 773.4 MB | CEF UI, native audio; not embeddable |
| Spotify Web Player in an Edge `--app` window | 754.2 MB | 732.7 MB | 12 processes; fails both budgets |
| Phase 3 Tauri 2 spike | 558.0 MB | 594.8 MB | No memory policy applied |
| **Bare wry + WebView2 host, `Low` policy** | **60.5 MB** after re-trim | **206 MB** after re-trim; 319–430 MB in the one-shot soak | Paused passes; playing is conditional on periodic re-trims |

The 35-minute soak with a one-shot trim peaked at 429.8 MB at an ad boundary. The breach is an ad-creative/cache artifact, not a leak: JS heap stayed at 58–73 MB and the DOM at 4.7–4.9k nodes. Re-applying `MemoryUsageTargetLevel.Low` on pause reclaimed 387.3 → 60.5 MB, and a mid-playback re-trim reclaimed 259.0 → 206.0 MB while the playhead advanced.

The method, per-process tables, ads observed, caveats, and the V1 gate are in **[docs/PERFORMANCE_RESEARCH.md](docs/PERFORMANCE_RESEARCH.md)**. The earlier feasibility, DRM, and background-playback work is in **[docs/PHASE-0-2-REPORT.md](docs/PHASE-0-2-REPORT.md)** and **[docs/PHASE-3-SPIKE.md](docs/PHASE-3-SPIKE.md)**.

## Repository layout

```
bench/
  wryhost/          Minimal wry + tao + WebView2 host. The measured baseline.
  tauri-shell/      Blank Tauri 2 shell used for the framework comparison.
  measure/          PowerShell and CDP measurement harness.
  results/          Sanitized per-sample memory data from the Phase 4 soaks.
spike/
  src-tauri/        Phase 3 Tauri 2 spike: navigation allowlist, tray, SMTC.
  ui/               Unused local placeholder page required by the Tauri config.
  scripts/          Spike measurement script.
docs/               Phase 0–4 reports and the V1 gate.
```

## Building

### Prerequisites

- Windows 10/11 x64
- Rust stable with the MSVC toolchain (`rustup default stable-x86_64-pc-windows-msvc`)
- Microsoft Edge WebView2 Runtime (Evergreen)
- Node.js 20+ and npm for the Tauri spike

### Bare wry host

```powershell
cd bench\wryhost
cargo build --release
.\target\release\wryhost.exe --url https://open.spotify.com/ --data-dir "$env:LOCALAPPDATA\RustifyBench"
```

The executable is a single small binary that uses the system WebView2 runtime. `--data-dir` is the **parent** of the `EBWebView` folder that WebView2 creates.

Useful benchmark flags:

| Flag | Purpose |
|---|---|
| `--memory-low` | Set `MemoryUsageTargetLevel.Low` at launch |
| `--cdp <port>` | Enable the Chrome DevTools Protocol on `127.0.0.1:<port>` |
| `--toggle-file <path>` | Watch a file for `low` / `normal` and apply the memory level |
| `--browser-args "<args>"` | Pass additional Chromium/WebView2 arguments |
| `--hide-on-close` | Hide instead of exiting when the window is closed |
| `--hide-upsells` | Benchmark-only: hide the nav-bar upgrade button and download link |
| `--custom-chrome` | Benchmark-only: frameless window plus a second WebView title bar |

The last two are experiments, not product defaults. The custom chrome adds a second renderer (~38.5 MB PSS); V1 should use a native-painted caption. The upsell flag is page injection and requires an explicit scope decision before it can ship.

### Tauri spike

```powershell
cd spike
npm install
npx tauri build --bundles nsis
```

The spike has no Tauri capabilities for the remote origin, a top-level navigation allowlist, a tray menu, and an SMTC poller. Its installer was 1.08 MiB in the Phase 3 run.

## Measurement harness

`bench/measure/` contains the scripts used for the reports:

- `tree-metrics.ps1` — PSS, working set, private commit, GPU memory, and CPU for a process tree.
- `cdp-metrics.mjs` — JS heap, DOM counters, and page state over the DevTools Protocol.
- `smtc.ps1` — Windows media-session status and position.
- `policy-launch.ps1`, `soak.ps1`, `postsoak.ps1` — hydration, trim, 35-minute soak, and post-soak checks.
- `edge-launch.ps1`, `edge-measure.ps1` — Edge `--app` baseline.
- `cdp-*.mjs` — navigation and click helpers for the benchmark page.

The scripts were written for the original test machine and contain `C:\Rustify\...` paths. Adjust those paths (or run from `C:\Rustify`) before using them elsewhere. They measure; they do not modify the Spotify page or the audio path.

## Data and privacy

Browser profiles, cookies, session storage, screenshots of the signed-in account, and the original SMTC logs are **not** published. Those files contain account-specific listening history. `bench/results/` contains sanitized per-sample memory data with page titles and playlist URLs removed; the measured numbers are unchanged. Raw logs and profiles stay local and are covered by `.gitignore`.

## Legal and account notes

- Rustify is an unofficial host. “Spotify” and the Spotify logo are trademarks of Spotify AB. The MIT licence covers this repository's code, not Spotify's trademarks, content, or services.
- Using Rustify requires a Spotify account and remains subject to Spotify's terms. A free account will play advertisements.
- The project does not circumvent DRM, extract audio, block ads, or use undocumented Spotify APIs. The reports document why those paths were rejected.
- The reports are technical research, not legal advice.

## Contributing

See **[CONTRIBUTING.md](CONTRIBUTING.md)**. In short: keep the official playback path intact, keep experiments behind benchmark flags, and do not submit ad-blocking, DRM-circumvention, or credential-handling changes.

## Licence

MIT. See **[LICENSE](LICENSE)**.
