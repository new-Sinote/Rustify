# Contributing

Rustify is a research project first. Changes are judged on whether they preserve the measured configuration, not only on whether they compile.

## Scope

Accepted:

- Native shell features: window chrome, tray behaviour, media keys, SMTC, single-instance, installer, updater.
- WebView2 lifecycle and memory policy work that leaves the official player's playback, DRM, and advertising path intact.
- Measurement harness improvements, portability fixes, and documentation.
- Build, packaging, accessibility, and reliability fixes.

Not accepted:

- Ad blocking, skipping, muting, request filtering, DNS/proxy filtering, or any change that alters ad delivery.
- DRM circumvention, audio extraction, re-streaming, or alternate audio sources.
- Credential or token extraction, session scraping, or undocumented Spotify APIs.
- Shipping page injection, extensions, or modified Spotify clients as part of the product.
- Security weakening, such as disabling site isolation or the GPU process, to win a benchmark.

Benchmark-only flags may exist in the tree when they are clearly opt-in, off by default, and documented as outside the measured configuration. The `--hide-upsells` and `--custom-chrome` flags in `bench/wryhost` are examples.

## Before opening a pull request

1. Build the affected crate on Windows with the MSVC toolchain.
2. If the change touches the shell or the memory policy, run the relevant harness in `bench/measure/` and include before/after numbers.
3. Keep the measured build free of the benchmark-only flags unless the test is explicitly about them.
4. Update the relevant document in `docs/` if a measured claim changes.
5. Do not commit browser profiles, cookies, logs with track metadata, screenshots of a signed-in account, or build output. `.gitignore` covers the usual cases; check `git status` before committing.

## Reporting security issues

Do not open a public issue for a session leak, credential exposure, or a way to escape the navigation allowlist. Use GitHub's private security advisory flow for this repository instead.

## Licence

By contributing, you agree that your contribution is licensed under the repository's MIT licence.
