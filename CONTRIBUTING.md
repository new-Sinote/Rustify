# Contributing

Rustify is a research project first. Changes are judged on whether they preserve the measured configuration, not only on whether they compile.

## Scope

Accepted:

- Native shell features: window chrome, tray behaviour, media keys, SMTC, single-instance, installer, updater.
- WebView2 lifecycle and memory policy work that preserves authentication, normal playback, DRM, and background playback.
- Adblocking work on `feature/adblocking`: network/cosmetic filtering, Spotify-specific ad detection, a fail-open mute fallback, and measurement.
- Legally compliant third-party filtering components with their licences, source information, and notices.
- Measurement harness improvements, portability fixes, and documentation.
- Build, packaging, accessibility, and reliability fixes.

Not accepted:

- DRM circumvention, audio extraction, re-streaming, or alternate audio sources.
- Credential or token extraction, session scraping, or undocumented Spotify APIs.
- Copying or redistributing Blockify code. The current Blockify extension is PolyForm Noncommercial 1.0.0; the older Python Blockify is MIT but is a Linux DBus/PulseAudio tool. Neither is a code source for this shell.
- Shipping third-party binaries without a reviewed licence, corresponding source where required, and notices.
- Hard-blocking that breaks playback or DRM and has no fail-open path.
- Modifying the published v0.2.0 release, its tag, assets, or certification.
- Security weakening, such as disabling site isolation or the GPU process, to win a benchmark.

Adblocking is a first-class requirement as of 2026-11-26. The old "ads untouched" rule is historical and must not be reintroduced as a current requirement. Work belongs on `feature/adblocking` until the owner approves a release. The stable 0.2.0 build remains ad-supported.

Benchmark-only flags, `ublock`, and `adblock` may exist in the tree when they are clearly opt-in, documented as outside the measured configuration, and not presented as stable. The `--hide-upsells` and `--custom-chrome` flags in `bench/wryhost` are examples; the stable app in `app/` uses the same chrome and a scoped upsell rule as product defaults.

## Before opening a pull request

1. Build the affected crate on Windows with the MSVC toolchain. The stable shell is `app/Cargo.toml`; build it with `cargo build --release --manifest-path app/Cargo.toml`. The adblocking proof of concept adds `--features adblock`; the uBOL layer adds `--features ublock`.
2. If the change touches the shell, memory policy, or adblocking, run the relevant harness in `bench/measure/` and include before/after numbers. Adblocking tests must use a temporary profile copy and must not log track titles.
3. Keep the measured build free of benchmark-only flags unless the test is explicitly about them.
4. Update the relevant document in `docs/` if a measured claim or product requirement changes.
5. Do not commit browser profiles, cookies, logs with track metadata, screenshots of a signed-in account, or build output. `.gitignore` covers the usual cases; check `git status` before committing.
6. Do not publish or retag a release without the owner's approval. The stable v0.2.0 release is frozen.

## Reporting security issues

Do not open a public issue for a session leak, credential exposure, or a way to escape the navigation allowlist. Use GitHub's private security advisory flow for this repository instead.

## Licence

By contributing, you agree that your contribution is licensed under the repository's MIT licence.
