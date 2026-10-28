//! QuickConnect authentication flow models and helpers.

use serde::{Deserialize, Serialize};

/// Result of initiating a QuickConnect pairing request (`POST /QuickConnect/Initiate`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct QuickConnectInitiateResponse {
    /// Secret string used for checking authorization status.
    pub secret: String,
    /// Human-readable code displayed on the Quest screen (e.g. "123456").
    pub code: String,
    /// Whether the code has been authorized by the user.
    #[serde(default)]
    pub authenticated: bool,
}

/// State response when polling QuickConnect status (`GET /QuickConnect/Connect`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct QuickConnectStateResponse {
    /// Secret string identifying the pairing request.
    pub secret: String,
    /// Code associated with the request (if still available).
    #[serde(default)]
    pub code: Option<String>,
    /// Set to true once the user has approved the PIN in their browser/mobile app.
    pub authenticated: bool,
}

/// Request body sent to finalize authentication (`POST /Users/AuthenticateWithQuickConnect`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct QuickConnectAuthRequest {
    /// Secret token returned during initiate.
    pub secret: String,
}

/// User details inside authentication response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct QuickConnectUser {
    /// Jellyfin user GUID.
    pub id: String,
    /// Jellyfin username.
    pub name: String,
}

/// Session information inside authentication response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct QuickConnectSessionInfo {
    /// Active Jellyfin session ID.
    pub id: Option<String>,
}

/// Final authentication response from `/Users/AuthenticateWithQuickConnect`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct QuickConnectAuthResponse {
    /// Issued user access token.
    pub access_token: String,
    /// Authenticated Jellyfin user info.
    pub user: QuickConnectUser,
    /// Jellyfin server ID.
    pub server_id: Option<String>,
    /// Session information.
    pub session_info: Option<QuickConnectSessionInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_initiate_response() {
        let json = r#"{
            "Secret": "sec-998877",
            "Code": "654321",
            "Authenticated": false
        }"#;
        let res: QuickConnectInitiateResponse = serde_json::from_str(json).unwrap();
        assert_eq!(res.secret, "sec-998877");
        assert_eq!(res.code, "654321");
        assert!(!res.authenticated);
    }

    #[test]
    fn test_deserialize_state_response() {
        let json = r#"{
            "Secret": "sec-998877",
            "Code": "654321",
            "Authenticated": true
        }"#;
        let res: QuickConnectStateResponse = serde_json::from_str(json).unwrap();
        assert_eq!(res.secret, "sec-998877");
        assert!(res.authenticated);
    }

    #[test]
    fn test_deserialize_auth_response() {
        let json = r#"{
            "AccessToken": "jellyfin-token-xyz",
            "ServerId": "srv-001",
            "User": {
                "Id": "user-guid-1",
                "Name": "josh"
            },
            "SessionInfo": {
                "Id": "sess-123"
            }
        }"#;
        let res: QuickConnectAuthResponse = serde_json::from_str(json).unwrap();
        assert_eq!(res.access_token, "jellyfin-token-xyz");
        assert_eq!(res.user.id, "user-guid-1");
        assert_eq!(res.user.name, "josh");
        assert_eq!(res.server_id.as_deref(), Some("srv-001"));
        assert_eq!(res.session_info.unwrap().id.as_deref(), Some("sess-123"));
    }
}
