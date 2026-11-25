# Contributing

Rustify is a research project first. Changes are judged on whether they preserve the measured configuration, not only on whether they compile.

## Scope

Accepted:

- Native shell features: window chrome, tray behaviour, media keys, SMTC, single-instance, installer, updater.
- WebView2 lifecycle and memory policy work that leaves the default build's official player playback, DRM, and advertising path intact.
- Measurement harness improvements, portability fixes, and documentation.
- Build, packaging, accessibility, and reliability fixes.

Not accepted:

- Ad blocking, skipping, muting, request filtering, DNS/proxy filtering, or any change that alters ad delivery in the default build. The isolated `ublock` feature and its experimental release are the only exception; do not make it the default.
- DRM circumvention, audio extraction, re-streaming, or alternate audio sources.
- Credential or token extraction, session scraping, or undocumented Spotify APIs.
- General page injection, extensions, or modified Spotify clients outside the disclosed uBOL experiment. The default build ships exactly one scoped cosmetic rule: a stylesheet that hides the `Explore Premium` button and `Install App` link in the nav bar. Expanding that rule, injecting into the player, or touching playback, ads, DRM, or network traffic in the default build is out of scope.
- Security weakening, such as disabling site isolation or the GPU process, to win a benchmark.

Benchmark-only flags and the `ublock` feature may exist in the tree when they are clearly opt-in, off by default, and documented as outside the measured configuration. The `--hide-upsells` and `--custom-chrome` flags in `bench/wryhost` are examples; the default app in `app/` uses the same chrome and a scoped upsell rule as product defaults. The experimental uBOL installer is a separate pre-release and is not the default download.

## Before opening a pull request

1. Build the affected crate on Windows with the MSVC toolchain. The default shell is `app/Cargo.toml`; build it with `cargo build --release --manifest-path app/Cargo.toml`. The experimental uBOL variant adds `--features ublock`.
2. If the change touches the shell or the memory policy, run the relevant harness in `bench/measure/` and include before/after numbers.
3. Keep the measured build free of the benchmark-only flags unless the test is explicitly about them.
4. Update the relevant document in `docs/` if a measured claim changes.
5. Do not commit browser profiles, cookies, logs with track metadata, screenshots of a signed-in account, or build output. `.gitignore` covers the usual cases; check `git status` before committing.

## Reporting security issues

Do not open a public issue for a session leak, credential exposure, or a way to escape the navigation allowlist. Use GitHub's private security advisory flow for this repository instead.

## Licence

By contributing, you agree that your contribution is licensed under the repository's MIT licence.
