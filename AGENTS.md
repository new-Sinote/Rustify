# Rustify project instructions

These instructions apply to all contributors and automation working in this repository.

## Product direction

On **2026-11-26** the project owner changed the product requirements:

- Spotify advertisements no longer have to remain untouched.
- Integrated adblocking is now a first-class product requirement.
- The old "ads untouched" contract is historical. Do not reintroduce it as a current requirement.
- The stable **v0.2.0-pre.1** release and its tag/assets remain unchanged. Adblocking work belongs on the `feature/adblocking` branch until the owner approves a release.

## Adblocking scope

Accepted work:

- Network and cosmetic request filtering through a legally compliant engine or extension.
- Spotify-specific ad detection using documented platform APIs.
- A mute fallback when an audio ad cannot be blocked without breaking playback.
- Measurement, logging, and documentation of overhead, false positives, and failures.

Required properties:

- Normal Spotify playback, authentication, DRM, and background playback must keep working.
- The implementation must fail open: if detection or filtering fails, audio must not be left muted or blocked.
- Filtering must not intercept or modify DRM licence traffic, audio decryption, credentials, or tokens.
- Any page-level change must be disclosed and scoped to the feature.
- A build that changes ad delivery must not claim the 0.2.0 certification.

## Prohibited

- DRM circumvention, audio extraction, re-streaming, or alternate audio sources.
- Credential or token extraction, session scraping, or undocumented Spotify APIs.
- Copying or redistributing Blockify code without permission. The current Blockify extension is **PolyForm Noncommercial 1.0.0**, not an OSI open-source licence. Do not copy it.
- Shipping third-party binaries without their licence, source information, and any required notices.
- Modifying the published `v0.2.0-pre.1` tag, assets, or certification.
- Using a real signed-in profile for experiments. Use a temporary copy.

## Licensing

- Rustify's own code is MIT.
- uBlock Origin Lite is GPL-3.0. If it is bundled, include the full licence and corresponding source, and keep the GPL-covered files separable from MIT code.
- `adblock-rust` is MPL-2.0 if it is added. Keep its file-level notices and publish modifications to MPL-covered files as required.
- Do not add a dependency or payload whose licence has not been reviewed.

## Testing and evidence

- Use a copied profile and a free account when testing ad behaviour.
- Do not commit or publish cookies, profiles, screenshots of a signed-in account, track titles, listening history, or raw SMTC logs with metadata.
- Measure memory and CPU overhead against a control build.
- Test sign-in, playback, DRM, background playback, ad detection, mute, unmute, pause/resume, and a full ad cycle.
- Document failures and false positives honestly.

## Documentation

- Active documents must describe the new direction.
- Historical documents keep their measured results but must carry a clear "superseded" banner where they state the old ads-untouched contract.
- The 0.2.0 certification remains valid for 0.2.0 only. It is not valid for any adblocking build.
- A new certification is required before an adblocking build can be called stable.
