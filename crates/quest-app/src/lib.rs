//! Android application bridge and Jellyfin session storage.
mod accounts;
pub use accounts::AppSessionStore;

pub mod bridge;
pub mod controls;
pub mod playback_profile;
mod server_api;
pub mod surface_size;

#[cfg(target_os = "android")]
mod android;

#[cfg(target_os = "android")]
mod cinema_atlas;
#[cfg(target_os = "android")]
mod immersive;
