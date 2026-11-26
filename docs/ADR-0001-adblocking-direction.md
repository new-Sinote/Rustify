# ADR-0001: Adblocking is a first-class product requirement

**Status:** Accepted  
**Date:** 2026-11-26  
**Owner decision:** Explicit product-direction change

## Context

Rustify was originally built around an "ads untouched" contract. The README, contributing guide, Phase 0-2 report, Phase 3 spike, 0.2.0 certification, performance research, and older release notes all repeat that constraint. The uBOL 0.3.0 experiment was an explicit exception, but it was still described as non-default and uncertified.

The owner has now changed the product requirements. Spotify advertisements do not have to remain untouched. Integrated adblocking is in scope, and the old contract must not be silently kept in active documentation.

## Decision

1. Adblocking becomes a first-class feature on the `feature/adblocking` branch.
2. The default/stable **v0.2.0-pre.1** release stays unchanged. It remains an ad-supported build and its certification remains valid only for that build.
3. Adblocking is evaluated as a multi-layer problem:
   - network/cosmetic filtering for page and tracker requests;
   - Spotify-specific detection for audio ad breaks;
   - a fail-open mute fallback when an audio ad cannot be removed without breaking playback.
4. The first experimental implementation will use documented WebView2 APIs and independently written Rust code. No Blockify code will be copied.
5. Any adblocking build must pass a new certification before it can be called stable. The 0.2.0 certification does not apply.

## Consequences

- Active documentation must stop claiming that ads are untouched as a current product requirement.
- Historical documents keep their measurements but get explicit superseded banners.
- The 0.2.0 certification is invalidated for adblocking builds, not for 0.2.0 itself.
- GPL-3.0 obligations apply if uBOL is distributed; MPL-2.0 obligations apply if `adblock-rust` is added.
- Spotify terms and account risk change. The owner accepts that risk for this direction.
- Testing must cover free accounts, playback, DRM, background playback, ad detection, mute/unmute, false positives, and memory/CPU overhead.

## Non-goals

- DRM circumvention.
- Audio extraction or re-streaming.
- Credential or token extraction.
- Undocumented Spotify APIs.
- Copying Blockify or other unlicensed third-party code.
- Changing the published 0.2.0 release or its certification record.
