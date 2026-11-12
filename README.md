# Jellyfin 3D for Quest

A work-in-progress native Jellyfin client for Quest, built on the reusable `xr-core`, `xr-video` and `jellyfin-client` crates from [questkit](https://github.com/Josh-Archer/questkit) (extracted from [DeskQuest](https://github.com/Josh-Archer/DeskQuest)). This project is independent of Jellyfin; it does not use a web player.

## Current capabilities

The native Quest panel pairs with Jellyfin using Quick Connect and restores the saved account on launch. Home shows your libraries, Continue Watching, and Recently Added. Movies, shows, episodes, favorites and search use the authenticated Jellyfin API. Poster artwork is loaded from Jellyfin in a decoder-compatible format, cached in a bounded memory cache, and falls back to a labeled placeholder if unavailable. Title details show the overview, year, rating and duration, with resume, play-from-beginning and favorite actions. The interface uses a landscape Quest panel with large hand/controller targets, scrolling content, loading indicators, cancellation, empty states and retry actions.

Settings save the default player, streaming bitrate ceiling, server-transcoding permission, cinema width, distance and curvature, environment (dark cinema, room passthrough or furnished loft), resume behavior, poster density and sorting. The default bitrate negotiation remains 120 Mbps. Selecting a lower ceiling can trigger server conversion and change quality. Disabling transcoding requires a direct-play or remux source. Account settings provide server switching, sign-out and artwork-cache clearing.

Playback uses Android's media player to demux and decode into a DeskQuest OpenXR Android surface swapchain. Cinema opens in a separate fullscreen task on the headset display, anchored in the current viewing direction. A view-relative fallback keeps the picture and controls visible when positional tracking is unavailable. Mono, half/full side-by-side and half/full over-under video use per-eye quad or cylinder layers; Jellyfin metadata and filename tags determine packing and eye order. The movie's decoded resolution is preserved when moving, sizing or curving the screen.

Cinema embeds its transport overlay into the bottom of the movie. It stays attached when the movie moves, resizes or tilts, and follows the same cylinder when curved. A translucent gradient, vector playback icons, title, elapsed/duration display, seek bar and pointer highlights replace the separate floating panel. Play/pause, ten-second seeking, audio, subtitles, Exit and Hide are in the main bar; Screen expands size, curvature and recenter tools above it. Controls fade out after six idle seconds during playback and return when paused or when the movie is clicked or pinched. Screen tools and active pointer interaction keep them visible. It remains available while loading or buffering, with animated progress feedback. Point and press a controller trigger or pinch to select. Grip a screen edge to move it, or pinch an edge with hand tracking; drag a corner to resize. DeskQuest's grab state machine, hand aiming, hit testing and handle artwork are reused. Click or pinch the movie to reveal hidden controls. Settings and screen size/curvature changes persist for the next video; Recenter brings the current screen in front of you. Manipulation handles appear while editing the screen or hovering an edge, keeping the movie clear during playback. The loft uses fixed room surfaces and a sofa, independently of the movable movie screen. System Back also leaves cinema. Cinema renders text subtitle cues on a screen-attached strip above the transport bar (including while controls are hidden), using embedded MediaPlayer timed text when available and Jellyfin text subtitle delivery for external SRT-like tracks when the decoder exposes none. Bitmap subtitle formats and ASS styling are not drawn. The Quest panel player offers audio and supported embedded-subtitle selection.

Immersive playback waits for OpenXR focus before starting. Losing XR focus, losing Android window focus, screen-off, or activity pause pauses or stops the player. Returning leaves playback paused until Play is selected, including after an activity pause. Progress is reported to Jellyfin with the paused state. These paths supplement one another because removing a headset does not necessarily pause the Android activity immediately.

The app resumes from Jellyfin's saved position and reports playback start, periodic progress and stop. It prefers direct play, then a server-provided remux or HLS transcode. When decoded dimensions differ from source metadata, the OpenXR surface is rebound before the decoder starts. The app displays persistent startup errors and returns from an initial presentation timeout. Immersive playback is considered started only after the decoder produces video and OpenXR successfully submits a video layer. The device profile remains a negotiation proposal, not a guarantee for every advertised format or bitrate.

A connected-Quest smoke suite verifies artwork reaching image views, home and library navigation, details, favorites, panel-video first-frame events, settings persistence and series navigation using an isolated test installation and mock server. It also verifies a distinct VR task on the headset display, successful video-layer submission, and return to the library panel. It captures screenshots for UI inspection and removes its test packages afterward. This does not establish real-library picture quality or immersive visibility. The user's real server has working panel playback; the earlier immersive view had audio with a black screen. A diagnostic run reproduced zero submitted layers while tracking was unavailable; the VR activity also shared a resizable panel task and remained in the VISIBLE session state. After separating the tasks and adding the view-space fallback, the connected-Quest test reached FOCUSED and submitted video layers. The user subsequently confirmed that the picture appears. This release adds controls and screen manipulation; physical gesture, loft appearance, stereo-quality and headset-removal checks remain separate from the automated submission tests.

The [player design notes](docs/player-design.md) describe the Horizon-style controls, measured hardware decode tier and neutral binocular buffering presentation.

This is an independent native client with familiar Jellyfin browsing flows, not full feature parity with Jellyfin's official clients. Live TV, downloads, casting, playlists, and native MVC/MV-HEVC decoding remain unimplemented. Fisheye cinema uses an equidistant UV-mesh remap path (hemispherical Equirect2 carriers; OpenXR has no fisheye layer). Unsupported multiview formats are not offered in cinema. Ordinary HEVC support does not imply multiview decoding. Keycloak/OIDC code is experimental and not connected to the app.

Quick Connect tokens are kept in the application's private, backup-excluded storage and bound to the server URL and device identity. Video requests use an authorization header rather than placing the token in the URL. Prefer HTTPS when connecting outside a trusted network; HTTP is supported for local servers and sends credentials without transport encryption.

## VR180, VR360 and MVC sources

Equirectangular VR180 and VR360 use native OpenXR spherical layers with independent
left/right eye crops. The decoded resolution is preserved. Playback offers Cinema
screen, VR180 and VR360 choices and remembers the choice per title. Filename hints
and explicit Jellyfin `VR180`/`VR360` tags select coverage. An `Equirectangular` tag
uses per-eye aspect as an initial coverage estimate; choose 180/360 if necessary.
Ordinary movies are never classified as VR from resolution alone.

In VR, click or pinch to reveal transport controls. **View** provides tilt,
180/360 coverage and Recenter. Moving the controls does not rotate the panorama.
Headset/focus loss pauses playback and requires an explicit Play to resume.
Flat movies retain their movable, resizable and curved cinema screens.

Blu-ray MVC and MV-HEVC require a prepared packed-stereo alternate; ordinary server
transcoding cannot reconstruct the dependent eye, and Quest MediaCodec / `xr-video`
only decode a single HEVC view. The [preparation tool](tools/README.md) decodes both
MVC views and verifies every losslessly encoded stereo frame. The app selects a
packed SBS/OU alternate over the original multiview stream when Jellyfin offers one,
and refuses raw MV-HEVC with an error that names the missing dual-view decode path.
Preparation retains the original file. Native MVC and MV-HEVC decoding remain
unsupported.

Jellyfin's 3D filter requires server metadata. For a confirmed full-SBS source,
a file-specific NFO can set `<fileinfo><streamdetails><video><format3d>FSBS</format3d>`
(with matching closing elements) and `<tag>Equirectangular</tag>`. Refresh the item
in Jellyfin after adding the sidecar. Do not label a file stereo solely from its
name or dimensions.

## Library and accounts

The library header uses the Jellyfin service logo, the current page, and a user picker.
The **3D only** toggle stays visible beside navigation and remembers its setting. It uses
Jellyfin's `Is3D` filter before pagination across libraries, search, favorites, recent titles
and Continue Watching. In Shows it lists matching 3D episodes. Titles need their 3D format
set in Jellyfin to appear; the toggle does not convert ordinary video into stereo.

Choose your name in the header or **Settings → Switch user**. Public server users can sign
in with a username/password or Quick Connect; hidden users can enter their username.
Authenticated accounts are remembered on this Quest and can be switched with one selection.
A saved token is checked with Jellyfin before switching. Sign out removes the current saved
account without removing the others. Passwords are never stored. Library requests and
playback reports remain bound to the user who opened them, including while switching.

Panel playback uses an overlaid top bar that fades after five idle seconds while playing.
Tap the movie or use controller input to reveal it; pause, buffering and errors keep it visible.
Hovering or interacting with the bar keeps its buttons available. Hiding it does not resize
the video surface. The immersive player retains its movie-attached controls and idle fade.

## Layout

- `crates/quest-app`: Android panel, Rust API bridge, private session store, immersive activity, and APK build script.
- `crates/quest-jellyfin-player`: format detection, per-eye geometry, and playback primitives.
- `xr-core`, `xr-video` and `jellyfin-client`: versioned crates from [questkit](https://github.com/Josh-Archer/questkit), not copied into this repository. The Jellyfin API client lives there (it was `crates/jellyfin`). The Android build uses the packaging harness from the resolved `xr-core` package.

## Build and test

Install Rust stable, the Android `aarch64-linux-android` target, `cargo-ndk`, JDK 17, Android SDK platform 34, build tools, and NDK 27.2.12479018. Set `ANDROID_SDK_ROOT` if the SDK is not at `~/Android/Sdk`.

```sh
cargo fmt --all -- --check
cargo build --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python3 -m unittest discover -s tools -p 'test_*.py'
crates/quest-app/android/tests/run.sh
crates/quest-app/android/build.sh
# Optional, with an awake developer-mode Quest attached; requires ffmpeg:
crates/quest-app/android/tests/device-smoke.sh
```

The debug APK is written to `crates/quest-app/target/android-apk/jellyfin-3d-debug.apk`. Add `--release` to build a release-mode APK using the harness's debug signing key. Connect a Quest with developer mode and ADB enabled and add `--install` to install it. Open the app, enter your Jellyfin server URL, and authorize its Quick Connect code from a signed-in Jellyfin client.

The automated tests include a mock API flow for pairing, library browsing, playback preparation and session reporting. They do not replace testing an authenticated Jellyfin server or inspecting the immersive view in a headset.

## Roadmap

The [project roadmap](https://github.com/Josh-Archer/quest-jellyfin-3d/issues/2) tracks Jellyfin integration, stereoscopic playback, immersive projections, subtitles and audio, MV-HEVC, server conversion, and remote playback.

Device verification: `crates/quest-app/android/tests/device-smoke.sh` uses isolated packages and a local fixture server. It checks all three environments, stereo cylinder and VR180/VR360 equirectangular submission, transport submission, play/pause/precise seek/audio/exit, and real MediaPlayer response to injected focus callbacks. The Rust tests simulate controller and hand screen dragging, corner resizing, tracking loss, transport hit testing and curvature geometry. These checks do not replace wearer validation of gestures, visible composition, comfort or the physical headset-removal sensor path.
