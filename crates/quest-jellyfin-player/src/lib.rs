//! Immersive 3D video player for Meta Quest 3+ consuming `xr-core` and `xr-video`.
//!
//! Provides:
//! - Stereoscopic SBS/OU (half and full) layout partitioning and UV math
//! - Virtual screen presentation (flat quad and curved cinema cylinder)
//! - Passthrough and dark room theatrical environments
//! - Hardware video decoding pipeline integration with `xr-video`
//! - Jellyfin session reporting and direct play coordination

pub mod error;
pub mod playback_engine;
pub mod stereo;
pub mod virtual_screen;

pub use error::{PlayerError, Result};
pub use playback_engine::{PlaybackEngine, PlaybackState};
pub use stereo::{
    calculate_display_aspect_ratio, calculate_eye_pixel_rect, calculate_eye_uv, Eye, EyeRegion,
    NormalizedUvRect, StereoPixelRect,
};
pub use virtual_screen::{
    build_stereo_cylinder_layers, build_stereo_eye_layer_specs, build_stereo_quad_layers,
    EyeLayerSpec, VirtualScreenConfig, VirtualScreenEnvironment, VirtualScreenShape,
};
