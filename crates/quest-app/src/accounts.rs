use jellyfin_client::{StoreError, StoredSession, TokenStore};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, sync::Mutex};

// Reads and atomic replacements must share one lock: auth and image requests run concurrently.
static STORE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Default, Serialize, Deserialize)]
struct Accounts {
    active: Option<(String, String, String)>,
    sessions: Vec<StoredSession>,
}
fn key(s: &StoredSession) -> (String, String, String) {
    (
        s.server_url.trim_end_matches('/').into(),
        s.device_id.clone(),
        s.user_id.clone(),
    )
}

/// Remembered accounts in app-private, backup-excluded storage. Legacy single sessions migrate
/// on the next write. Only tokens are remembered; passwords never reach this store.
pub struct AppSessionStore(pub PathBuf);
impl AppSessionStore {
    fn read(&self) -> Result<Accounts, StoreError> {
        match fs::read(&self.0) {
            Ok(data) => {
                if let Ok(accounts) = serde_json::from_slice::<Accounts>(&data) {
                    return Ok(accounts);
                }
                let session: StoredSession = serde_json::from_slice(&data)
                    .map_err(|_| StoreError::new("Cannot read saved accounts"))?;
                Ok(Accounts {
                    active: Some(key(&session)),
                    sessions: vec![session],
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Accounts::default()),
            Err(_) => Err(StoreError::new("Cannot read saved accounts")),
        }
    }
    fn write(&self, accounts: &Accounts) -> Result<(), StoreError> {
        use std::io::Write;
        if accounts.sessions.is_empty() {
            return match fs::remove_file(&self.0) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(_) => Err(StoreError::new("Cannot clear saved accounts")),
            };
        }
        let tmp = self.0.with_extension("tmp");
        let data =
            serde_json::to_vec(accounts).map_err(|_| StoreError::new("Cannot encode accounts"))?;
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
        result.map_err(|_| StoreError::new("Cannot save accounts"))
    }
    pub fn sessions(&self, server: &str, device: &str) -> Result<Vec<StoredSession>, StoreError> {
        let _guard = STORE_LOCK
            .lock()
            .map_err(|_| StoreError::new("Account store unavailable"))?;
        Ok(self
            .read()?
            .sessions
            .into_iter()
            .filter(|s| {
                s.server_url.trim_end_matches('/') == server.trim_end_matches('/')
                    && s.device_id == device
            })
            .collect())
    }
    pub fn find(
        &self,
        server: &str,
        device: &str,
        user: &str,
    ) -> Result<Option<StoredSession>, StoreError> {
        Ok(self
            .sessions(server, device)?
            .into_iter()
            .find(|s| s.user_id == user))
    }
}
impl TokenStore for AppSessionStore {
    fn load(&self) -> Result<Option<StoredSession>, StoreError> {
        let _guard = STORE_LOCK
            .lock()
            .map_err(|_| StoreError::new("Account store unavailable"))?;
        let accounts = self.read()?;
        Ok(accounts
            .sessions
            .into_iter()
            .find(|s| Some(key(s)) == accounts.active))
    }
    fn save(&self, session: &StoredSession) -> Result<(), StoreError> {
        let _guard = STORE_LOCK
            .lock()
            .map_err(|_| StoreError::new("Account store unavailable"))?;
        let mut accounts = self.read()?;
        let id = key(session);
        accounts.sessions.retain(|s| key(s) != id);
        accounts.sessions.push(session.clone());
        accounts.active = Some(id);
        self.write(&accounts)
    }
    fn clear(&self) -> Result<(), StoreError> {
        let _guard = STORE_LOCK
            .lock()
            .map_err(|_| StoreError::new("Account store unavailable"))?;
        let mut accounts = self.read()?;
        accounts
            .sessions
            .retain(|s| Some(key(s)) != accounts.active);
        accounts.active = None;
        self.write(&accounts)
    }
}
