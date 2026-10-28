# quest-jellyfin-3d

Free immersive 3D Jellyfin player for Meta Quest 3+ (SBS/OU, 180/360, MV-HEVC) written in Rust, built on top of reusable XR crates from [DeskQuest](https://github.com/Josh-Archer/DeskQuest).

Tracking roadmap: [DeskQuest #199](https://github.com/Josh-Archer/DeskQuest/issues/199).

---

## Features (Milestones M1 & M2)

### Milestone M1: Jellyfin Client & Direct Play
- **QuickConnect Authentication**:
  - `POST /QuickConnect/Initiate`: initiate pairing code.
  - `GET /QuickConnect/Connect?secret=...`: poll authorization state.
  - `POST /Users/AuthenticateWithQuickConnect`: exchange secret for user access token and session ID.
- **Keycloak OpenID Connect (OIDC)**:
  - Standard Keycloak authorization code flow with PKCE (`S256`).
  - Extensible `OidcProvider` trait with `KeycloakOidcClient` and offline `MockOidcProvider`.
  - Token exchange, user info claims, and token refresh.
- **Library Browse**:
  - `GET /Items?Fields=Video3DFormat,MediaStreams,Path`: browse movies/episodes with 3D metadata.
  - Full stream parsing (codecs, resolutions, framerates, container).
- **Meta Quest 3+ Device Profile**:
  - Optimized for Snapdragon XR2 Gen 2 capabilities:
    - HEVC / H.265 Direct Play up to **8192x4320@30fps (~8K30 HEVC cap)** or 4096x4096@60fps.
    - AV1 Main Direct Play up to 8K30 / 4K60.
    - AVC / H.264 Direct Play up to 4K60.
    - Audio Direct Play: Opus, AAC, FLAC, AC-3, E-AC-3, Vorbis, PCM.
    - Supported Containers: MKV, MP4, M4V, MOV, WebM, TS.
    - Streaming bitrates up to 120 Mbps.
- **Direct Play Stream URL**:
  - Generates `/Videos/{id}/stream?static=true&api_key={token}` fed directly to the hardware decoder.
- **Session Reporting**:
  - `POST /Sessions/Playing`: report playback initiation.
  - `POST /Sessions/Playing/Progress`: report playback position ticks and pause/unpause events.
  - `POST /Sessions/Playing/Stopped`: report playback termination and flush decoder.

### Milestone M2: Stereoscopic SBS / OU Virtual Screen
- **Stereo Layouts**:
  - **Half Side-by-Side (HSBS)**: Horizontally compressed frame with 2x aspect stretch compensation.
  - **Full Side-by-Side (FSBS)**: Double-width frame with 1:1 pixel aspect ratio.
  - **Half Over-Under (HOU / TAB)**: Vertically compressed frame with 2x aspect stretch compensation.
  - **Full Over-Under (FOU / TAB)**: Double-height frame with 1:1 pixel aspect ratio.
  - **Eye Ordering**: Left-Right (`_LR`) vs Right-Left (`_RL`), Top-Bottom (`_TB`) vs Bottom-Top (`_BT`).
- **Format Detection Precedence**:
  1. Jellyfin `Video3DFormat` (`HalfSideBySide`, `FullSideBySide`, `HalfTopAndBottom`, `FullTopAndBottom`, `Mvc`)
  2. Filename tags (`3D`, `HSBS`, `FSBS`, `OU`, `TAB`, `180`, `360`, `fisheye`, `_LR`, `_TB`, `_RL`, `_BT`, `HALF-SBS`, `FULL-SBS`, `HOU`, `FOU`, etc.)
  3. Container metadata (MKV `StereoMode` integer, stream `stereo_mode` tag)
  4. Fallback: 2D Monoscopic
- **Virtual Screen Presentation**:
  - **Passthrough Mode**: Blends virtual screen with real-world camera feed (`XR_FB_passthrough`).
  - **Dark Room Mode**: Theatrical black void / dark cinema room.
  - **Display Shapes**: Flat Quad (`openxr::CompositionLayerQuad`) and Curved Cinema Cylinder (`openxr::CompositionLayerCylinderKHR`).
  - Independent left and right eye OpenXR composition layer construction with exact sub-image pixel regions.

---

## Workspace Architecture

```
quest-jellyfin-3d/
├── .cargo/
│   └── config.toml                  # CMake policy configuration
├── .github/
│   └── workflows/ci.yml             # GitHub Actions CI (fmt, clippy, test, Android check)
├── Cargo.toml                       # Cargo workspace declaration
├── crates/
│   ├── jellyfin-client/             # Pure Rust Jellyfin REST API client (no Android deps, host-testable)
│   │   ├── src/
│   │   │   ├── auth.rs              # ClientInfo and Session headers
│   │   │   ├── client.rs            # JellyfinClient async API
│   │   │   ├── device_profile.rs    # Quest 3+ device profile & evaluation
│   │   │   ├── format_detection.rs  # Video3DFormat & filename tag detection
│   │   │   ├── models.rs            # Item, Stream, and Browse models
│   │   │   ├── oidc.rs              # Keycloak OpenID Connect stub & client
│   │   │   ├── playback.rs          # PlaybackInfo & direct play URL builder
│   │   │   ├── quick_connect.rs     # QuickConnect pairing API
│   │   │   └── sessions.rs          # Playing, Progress, Stopped reporting
│   │   └── tests/                   # Fixtures & Wiremock integration tests
│   └── quest-jellyfin-player/       # Stereo rendering & XR playback engine
│       ├── src/
│       │   ├── stereo.rs            # SBS/OU UV math & aspect ratio calculations
│       │   ├── virtual_screen.rs    # Flat quad & cylinder OpenXR layer construction
│       │   └── playback_engine.rs   # Hardware decoder pipeline & session coordination
│       └── tests/                   # UV math & mock decoder integration tests
└── README.md
```

### Dependencies on `xr-core` and `xr-video`
These reusable XR foundation crates are consumed from DeskQuest PR #200 (commit `1866493`):
```toml
xr-core = { git = "https://github.com/Josh-Archer/DeskQuest", rev = "1866493" }
xr-video = { git = "https://github.com/Josh-Archer/DeskQuest", rev = "1866493" }
```
- `xr-core`: Reusable OpenXR session, passthrough, quad/cylinder composition layers, and spatial models.
- `xr-video`: Zero-copy hardware video decoder abstraction (`HardwareVideoDecoder`, `MockHardwareDecoder`), stereo layouts, and A/V synchronizer (`AvSynchronizer`).

---

## Building & Testing

### Prerequisites
- Rust stable (1.75+ / 2021 edition)
- CMake 3.5+ (configured via `.cargo/config.toml` with `CMAKE_POLICY_VERSION_MINIMUM = "3.5"`)
- OpenSSL / libasound2 development packages on Linux

### Target Directory
Set the shared target directory:
```bash
export CARGO_TARGET_DIR=/workspace/.cargo-target-deskquest
```

### Running Tests
Execute all unit and integration test suites:
```bash
cargo test --workspace
```

### Linting & Formatting
```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## Roadmap

- [x] **M1**: Jellyfin client (QuickConnect, Keycloak OIDC stub, browse with 3D fields, Quest device profile, direct play URL, session reporting)
- [x] **M2**: SBS/OU (half & full) stereo on virtual screen (passthrough or dark room, format detection precedence)
- [ ] **M3**: 180° / 360° equirectangular and fisheye playback
- [ ] **M4**: Subtitles with adjustable depth, audio track selection, resume & progress tracking
- [ ] **M5**: MV-HEVC spatial video decoding
- [ ] **M6**: Server-side Blu-ray MVC to full SBS HEVC conversion
- [ ] **M7**: Tailscale remote playback & optimized transcode profiles
