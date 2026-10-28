//! Authentication headers, client identity, and session tokens.

use serde::{Deserialize, Serialize};

/// Identifies the client application and device to the Jellyfin server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientInfo {
    /// Friendly name of the client app (e.g. "quest-jellyfin-3d").
    pub client_name: String,
    /// Friendly name of the client device (e.g. "Meta Quest 3").
    pub device_name: String,
    /// Unique and stable identifier for the device installation.
    pub device_id: String,
    /// Semantic version of the client application (e.g. "0.1.0").
    pub version: String,
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self {
            client_name: "quest-jellyfin-3d".to_string(),
            device_name: "Meta Quest 3".to_string(),
            device_id: "quest-jellyfin-3d-headset".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

impl ClientInfo {
    /// Creates a new `ClientInfo` with specified details.
    pub fn new(
        client_name: impl Into<String>,
        device_name: impl Into<String>,
        device_id: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            client_name: client_name.into(),
            device_name: device_name.into(),
            device_id: device_id.into(),
            version: version.into(),
        }
    }

    /// Formats the standard Jellyfin authorization header value (`X-Emby-Authorization`).
    ///
    /// Formatted as:
    /// `MediaBrowser Client="...", Device="...", DeviceId="...", Version="..."[, Token="..."]`
    pub fn authorization_header(&self, token: Option<&str>) -> String {
        let mut header = format!(
            r#"MediaBrowser Client="{}", Device="{}", DeviceId="{}", Version="{}""#,
            self.client_name, self.device_name, self.device_id, self.version
        );
        if let Some(tok) = token {
            header.push_str(&format!(r#", Token="{}""#, tok));
        }
        header
    }
}

/// Active authenticated user session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// Jellyfin API / user access token.
    pub access_token: Option<String>,
    /// Authenticated Jellyfin User ID.
    pub user_id: Option<String>,
    /// Jellyfin Server ID.
    pub server_id: Option<String>,
}

impl Session {
    /// Creates an authenticated session with an access token.
    pub fn new(
        access_token: impl Into<String>,
        user_id: Option<String>,
        server_id: Option<String>,
    ) -> Self {
        Self {
            access_token: Some(access_token.into()),
            user_id,
            server_id,
        }
    }

    /// Returns `true` if an access token is present.
    pub fn is_authenticated(&self) -> bool {
        self.access_token
            .as_ref()
            .is_some_and(|t| !t.trim().is_empty())
    }

    /// Clears the session credentials.
    pub fn clear(&mut self) {
        self.access_token = None;
        self.user_id = None;
        self.server_id = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_info_default() {
        let client = ClientInfo::default();
        assert_eq!(client.client_name, "quest-jellyfin-3d");
        assert_eq!(client.device_name, "Meta Quest 3");
        assert_eq!(client.version, "0.1.0");
    }

    #[test]
    fn test_authorization_header_without_token() {
        let client = ClientInfo::new("test-app", "Quest 3", "device-123", "1.0.0");
        let header = client.authorization_header(None);
        assert_eq!(
            header,
            r#"MediaBrowser Client="test-app", Device="Quest 3", DeviceId="device-123", Version="1.0.0""#
        );
    }

    #[test]
    fn test_authorization_header_with_token() {
        let client = ClientInfo::new("test-app", "Quest 3", "device-123", "1.0.0");
        let header = client.authorization_header(Some("secret-token-xyz"));
        assert_eq!(
            header,
            r#"MediaBrowser Client="test-app", Device="Quest 3", DeviceId="device-123", Version="1.0.0", Token="secret-token-xyz""#
        );
    }

    #[test]
    fn test_session_state() {
        let mut session = Session::default();
        assert!(!session.is_authenticated());

        session.access_token = Some("abc".to_string());
        assert!(session.is_authenticated());

        session.clear();
        assert!(!session.is_authenticated());
        assert_eq!(session.access_token, None);
    }
}
