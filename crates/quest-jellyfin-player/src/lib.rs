//! Immersive 3D video player for Meta Quest 3+ consuming `xr-core` and `xr-video`.
//!
//! Provides:
//! - Stereoscopic SBS/OU (half and full) layout partitioning and UV math
//! - Virtual screen presentation (flat quad and curved cinema cylinder)
//! - Passthrough and dark room theatrical environments
//! - Hardware video decoding pipeline integration with `xr-video`
//! - Jellyfin session reporting and direct play coordination

pub mod error;
pub mod format;
pub mod playback_engine;
pub mod stereo;
pub mod virtual_screen;

pub use error::{PlayerError, Result};
pub use format::{detect_item_format, DetectedStereoFormat, EyeOrder, Projection, StereoFormat};
pub mod projection;
pub use playback_engine::{PlaybackEngine, PlaybackState};
pub use stereo::{
    aspect_correction_scale, calculate_display_aspect_ratio, calculate_eye_pixel_rect,
    calculate_eye_uv, calculate_eye_uv_crop, is_packed_stereo, stereo_layout, Eye, EyeRegion,
    EyeUvCrop, NormalizedUvRect, StereoLayout, StereoPixelRect,
};
pub use virtual_screen::{
    build_stereo_cylinder_layers, build_stereo_cylinder_layers_at_pose,
    build_stereo_eye_layer_specs, build_stereo_quad_layers, build_stereo_quad_layers_at_pose,
    plan_stereo_virtual_screen, EyeLayerSpec, StereoScreenPlan, VirtualScreenConfig,
    VirtualScreenEnvironment, VirtualScreenShape,
};

pub mod cinema;
pub mod environment;
pub mod screen_controls;
