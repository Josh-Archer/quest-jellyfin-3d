# Jellyfin 3D for Quest

A work-in-progress native Jellyfin client for Quest, using the Jellyfin HTTP API and the reusable `xr-core` and `xr-video` crates from [DeskQuest](https://github.com/Josh-Archer/DeskQuest). This project is independent of Jellyfin; it does not use a web player.

## Current capabilities

The native Quest panel pairs with Jellyfin using Quick Connect and restores the saved account on launch. Home shows your libraries, Continue Watching, and Recently Added. Movies, shows, episodes, favorites and search use the authenticated Jellyfin API. Poster artwork is loaded from Jellyfin in a decoder-compatible format, cached in a bounded memory cache, and falls back to a labeled placeholder if unavailable. Title details show the overview, year, rating and duration, with resume, play-from-beginning and favorite actions. The interface uses a landscape Quest panel with large hand/controller targets, scrolling content, loading indicators, cancellation, empty states and retry actions.

Settings save the default player, streaming bitrate ceiling, server-transcoding permission, cinema width and distance, passthrough, resume behavior, poster density and sorting. The default bitrate negotiation remains 120 Mbps. Selecting a lower ceiling can trigger server conversion and change quality. Disabling transcoding requires a direct-play or remux source. Account settings provide server switching, sign-out and artwork-cache clearing.

Playback uses Android's media player to demux and decode into a DeskQuest OpenXR Android surface swapchain. The cinema opens in the tracked viewing direction, stays anchored in the room, and recenters when a thumbstick is pushed upward. An animated loading panel reuses DeskQuest's textured compositor overlays until the decoder produces a frame, and during buffering. Mono, half/full side-by-side and half/full over-under video use appropriate per-eye quad layers. Jellyfin metadata and filename tags determine packing and eye order. Trigger toggles playback, a horizontal thumbstick deflection seeks ten seconds, and grip cycles audio tracks. Use system Back to leave cinema. The Quest panel player also offers audio and supported embedded-subtitle selection.

The app resumes from Jellyfin's saved position and reports playback start, periodic progress and stop. It prefers direct play, then a server-provided remux or HLS transcode. When decoded dimensions differ from source metadata, the OpenXR surface is rebound before the decoder starts. The app displays persistent startup errors and returns from an initial frame timeout. The device profile remains a negotiation proposal, not a guarantee for every advertised format or bitrate.

A connected-Quest smoke suite verifies artwork reaching image views, home and library navigation, details, favorites, panel-video first-frame events, settings persistence and series navigation using an isolated test installation and mock server. It captures screenshots for UI inspection and removes its test packages afterward. This does not establish real-library picture quality or immersive visibility. The user's real server has working panel playback; the earlier immersive view had audio with a black screen. The new head-relative placement and loading overlay still need physical-headset confirmation.

This is an independent native client with familiar Jellyfin browsing flows, not full feature parity with Jellyfin's official clients. Live TV, downloads, casting, playlists, 360/180 and fisheye projection, MVC/MV-HEVC, on-screen immersive transport controls and immersive subtitles remain unimplemented. Unsupported projection and multiview formats are not offered in cinema. Ordinary HEVC support does not imply multiview decoding. Keycloak/OIDC code is experimental and not connected to the app.

Quick Connect tokens are kept in the application's private, backup-excluded storage and bound to the server URL and device identity. Video requests use an authorization header rather than placing the token in the URL. Prefer HTTPS when connecting outside a trusted network; HTTP is supported for local servers and sends credentials without transport encryption.

## Layout

- `crates/quest-app`: Android panel, Rust API bridge, private session store, immersive activity, and APK build script.
- `crates/jellyfin`: typed Jellyfin API client, device profile and session models.
- `crates/quest-jellyfin-player`: format detection, per-eye geometry, and playback primitives.
- `crates/jellyfin-player`: original XR crate scaffold.
- `xr-core` and `xr-video`: pinned Git dependencies from DeskQuest, not copied into this repository. The Android build uses the packaging harness from the resolved `xr-core` revision.

## Build and test

Install Rust stable, the Android `aarch64-linux-android` target, `cargo-ndk`, JDK 17, Android SDK platform 34, build tools, and NDK 27.2.12479018. Set `ANDROID_SDK_ROOT` if the SDK is not at `~/Android/Sdk`.

```sh
cargo fmt --all -- --check
cargo build --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
crates/quest-app/android/tests/run.sh
crates/quest-app/android/build.sh
# Optional, with an awake developer-mode Quest attached; requires ffmpeg:
crates/quest-app/android/tests/device-smoke.sh
```

The debug APK is written to `crates/quest-app/target/android-apk/jellyfin-3d-debug.apk`. Add `--release` to build a release-mode APK using the harness's debug signing key. Connect a Quest with developer mode and ADB enabled and add `--install` to install it. Open the app, enter your Jellyfin server URL, and authorize its Quick Connect code from a signed-in Jellyfin client.

The automated tests include a mock API flow for pairing, library browsing, playback preparation and session reporting. They do not replace testing an authenticated Jellyfin server or inspecting the immersive view in a headset.

## Roadmap

The [project roadmap](https://github.com/Josh-Archer/quest-jellyfin-3d/issues/2) tracks Jellyfin integration, stereoscopic playback, immersive projections, subtitles and audio, MV-HEVC, server conversion, and remote playback.

The shared DeskQuest repository is private. CI uses the read-only `DESKQUEST_DEPLOY_KEY` repository secret to fetch the pinned Cargo dependencies.
