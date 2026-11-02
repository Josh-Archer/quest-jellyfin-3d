//! Android application bridge and Jellyfin session storage.
use jellyfin_client::{StoreError, StoredSession, TokenStore};
use std::{fs, path::PathBuf};

/// Session stored in the application's private, backup-excluded directory.
pub struct AppSessionStore(pub PathBuf);

impl TokenStore for AppSessionStore {
    fn load(&self) -> Result<Option<StoredSession>, StoreError> {
        match fs::read(&self.0) {
            Ok(data) => serde_json::from_slice(&data)
                .map(Some)
                .map_err(|_| StoreError::new("Cannot read saved session")),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(StoreError::new("Cannot read saved session")),
        }
    }
    fn save(&self, session: &StoredSession) -> Result<(), StoreError> {
        use std::io::Write;
        let tmp = self.0.with_extension("tmp");
        let data =
            serde_json::to_vec(session).map_err(|_| StoreError::new("Cannot encode session"))?;
        let result = (|| -> std::io::Result<()> {
            let mut options = fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&tmp)?;
            file.write_all(&data)?;
            file.sync_all()?;
            fs::rename(&tmp, &self.0)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result.map_err(|_| StoreError::new("Cannot save session"))
    }
    fn clear(&self) -> Result<(), StoreError> {
        match fs::remove_file(&self.0) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(StoreError::new("Cannot clear saved session")),
        }
    }
}

pub mod bridge;
pub mod controls;
pub mod playback_profile;
pub mod surface_size;

#[cfg(target_os = "android")]
mod android;

#[cfg(target_os = "android")]
mod cinema_atlas;
#[cfg(target_os = "android")]
mod immersive;
