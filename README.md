# Jellyfin 3D for Quest

A work-in-progress native Jellyfin client for Quest, using the Jellyfin HTTP API and the reusable `xr-core` and `xr-video` crates from [DeskQuest](https://github.com/Josh-Archer/DeskQuest). This project is independent of Jellyfin; it does not use a web player.

## Current capabilities

The Android panel pairs with Jellyfin using Quick Connect, browses and searches movies and episodes, requests `PlaybackInfo`, and plays a server-approved source. Playback uses Android's media player to demux and decode into a DeskQuest OpenXR Android surface swapchain. The immersive activity submits separate flat quad layers for mono, half/full side-by-side and half/full over-under video. Jellyfin `Video3DFormat` and filename tags determine the packing and eye order. It resumes from Jellyfin's saved position and reports playback start, periodic progress, and stop. There is also a 2D playback preview.

The Quest app has been exercised with a mock Jellyfin server and a small H.264 test clip. This does not establish picture quality, stereo comfort, actual library compatibility, or reliable playback on a real server. The device profile is a negotiation proposal, not a verified guarantee that every advertised codec, resolution, container, audio track or bitrate will play. The app prefers direct play, then a server-provided remux or HLS transcode. Immersive remux and transcode are experimental: when Android reports new decoded dimensions, the app replaces the OpenXR surface and recomputes eye regions. The output needs focused-headset validation; an unattended test logged decoder allocation failures. Jellyfin video transcoding may reduce resolution or change HDR, so the app labels it before playback. The immersive view offers a dark background or passthrough. Trigger toggles playback, a horizontal thumbstick deflection seeks ten seconds, and grip cycles Android-detected audio tracks; use the system Back action to leave it. Controller actions and audio switching need physical-headset validation.

360/180 and fisheye projection, MVC/MV-HEVC, on-screen immersive controls, subtitles and explicit audio track selection are not yet integrated. Unsupported projection and multiview formats are not offered in immersive mode. MV-HEVC decoding and MVC-to-SBS conversion are not implemented; ordinary HEVC support does not imply multiview decoding. Keycloak/OIDC code is experimental and not connected to the app. Jellyfin image art is also not yet connected.

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
```

The debug APK is written to `crates/quest-app/target/android-apk/jellyfin-3d-debug.apk`. Add `--release` to build a release-mode APK using the harness's debug signing key. Connect a Quest with developer mode and ADB enabled and add `--install` to install it. Open the app, enter your Jellyfin server URL, and authorize its Quick Connect code from a signed-in Jellyfin client.

The automated tests include a mock API flow for pairing, library browsing, playback preparation and session reporting. They do not replace testing an authenticated Jellyfin server or inspecting the immersive view in a headset.

## Roadmap

The [project roadmap](https://github.com/Josh-Archer/quest-jellyfin-3d/issues/2) tracks Jellyfin integration, stereoscopic playback, immersive projections, subtitles and audio, MV-HEVC, server conversion, and remote playback.

The shared DeskQuest repository is private. CI uses the read-only `DESKQUEST_DEPLOY_KEY` repository secret to fetch the pinned Cargo dependencies.
