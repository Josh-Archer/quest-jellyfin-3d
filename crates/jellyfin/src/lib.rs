//! Headless Jellyfin HTTP client for the Quest 3D player.
//!
//! Pairing is `POST /QuickConnect/Initiate` and `GET /QuickConnect/Connect`.
//! On Jellyfin 10.8 and later the poll only flips `Authenticated`. This client
//! then exchanges the secret with `POST /Users/AuthenticateWithQuickConnect`
//! and hands the access token to a [`TokenStore`].
//!
//! Library browse is `GET /Items?Fields=Video3DFormat,MediaStreams,Path` with
//! `StartIndex` and `Limit`. Every request sends
//! `Authorization: MediaBrowser` with the client, device, device id, version,
//! and token once one exists.
//!
//! ```
//! # fn main() -> jellyfin_client::JellyfinResult<()> {
//! use std::sync::Arc;
//! use jellyfin_client::{ClientIdentity, JellyfinClient, MemoryTokenStore};
//!
//! let identity = ClientIdentity::new("DeskQuest", "Quest 3", "device-1", "0.1.0")?;
//! let client = JellyfinClient::new(
//!     "http://127.0.0.1:8096",
//!     identity,
//!     Arc::new(MemoryTokenStore::new()),
//! )?;
//! assert!(client.session().is_none());
//! # Ok(())
//! # }
//! ```

mod client;
mod error;
mod identity;
mod json;
mod library;
mod quick_connect;
mod store;

pub use client::JellyfinClient;
pub use error::{Error, JellyfinResult};
pub use identity::{authorization_header, ClientIdentity};
pub use library::{
    BaseItem, ItemPage, ItemsQuery, MediaStream, MediaStreamType, UserData, Video3DFormat,
    ITEM_FIELDS, TICKS_PER_SECOND,
};
pub use quick_connect::{QuickConnectChallenge, QuickConnectPoll};
pub use store::{MemoryTokenStore, StoreError, StoredSession, TokenStore};
