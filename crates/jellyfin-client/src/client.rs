//! Jellyfin HTTP API client.

use crate::auth::{ClientInfo, Session};
use crate::device_profile::{quest_device_profile, DeviceProfile};
use crate::error::{JellyfinError, Result};
use crate::format_detection::{detect_stereo_format, DetectedStereoFormat};
use crate::models::{ItemsQuery, ItemsResponse, MediaItem};
use crate::playback::{build_direct_play_url, PlaybackInfoRequest, PlaybackInfoResponse};
use crate::quick_connect::{
    QuickConnectAuthRequest, QuickConnectAuthResponse, QuickConnectInitiateResponse,
    QuickConnectStateResponse,
};
use crate::sessions::{PlaybackProgressRequest, PlaybackStartRequest, PlaybackStopRequest};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE};
use std::sync::Arc;
use tokio::sync::RwLock;
use url::Url;

/// Asynchronous Jellyfin API client tailored for Quest 3+ playback.
#[derive(Debug, Clone)]
pub struct JellyfinClient {
    base_url: Url,
    client_info: ClientInfo,
    session: Arc<RwLock<Session>>,
    http: reqwest::Client,
}

impl JellyfinClient {
    /// Creates a new unauthenticated Jellyfin client.
    pub fn new(base_url: Url, client_info: ClientInfo) -> Self {
        Self {
            base_url,
            client_info,
            session: Arc::new(RwLock::new(Session::default())),
            http: reqwest::Client::new(),
        }
    }

    /// Sets the access token on this client.
    pub async fn set_token(&self, token: impl Into<String>) {
        let mut session = self.session.write().await;
        session.access_token = Some(token.into());
    }

    /// Sets the authenticated user ID on this client.
    pub async fn set_user_id(&self, user_id: impl Into<String>) {
        let mut session = self.session.write().await;
        session.user_id = Some(user_id.into());
    }

    /// Gets current access token if authenticated.
    pub async fn token(&self) -> Option<String> {
        self.session.read().await.access_token.clone()
    }

    /// Gets current user ID if known.
    pub async fn user_id(&self) -> Option<String> {
        self.session.read().await.user_id.clone()
    }

    /// Returns the client info.
    pub fn client_info(&self) -> &ClientInfo {
        &self.client_info
    }

    /// Returns the base server URL.
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Internal helper to build request headers including Jellyfin authorization.
    async fn build_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let token = self.token().await;
        let auth_str = self.client_info.authorization_header(token.as_deref());
        if let Ok(val) = HeaderValue::from_str(&auth_str) {
            headers.insert("X-Emby-Authorization", val);
        }
        if let Some(t) = token {
            if let Ok(val) = HeaderValue::from_str(&t) {
                headers.insert("X-Emby-Token", val);
            }
        }
        headers
    }

    /// Helper to join endpoint paths to the base URL.
    fn endpoint_url(&self, relative_path: &str) -> Result<Url> {
        let trimmed = relative_path.trim_start_matches('/');
        self.base_url.join(trimmed).map_err(JellyfinError::from)
    }

    // ------------------------------------------------------------------------
    // QuickConnect API
    // ------------------------------------------------------------------------

    /// Initiates a QuickConnect pairing request (`POST /QuickConnect/Initiate`).
    pub async fn initiate_quick_connect(&self) -> Result<QuickConnectInitiateResponse> {
        let url = self.endpoint_url("QuickConnect/Initiate")?;
        let headers = self.build_headers().await;

        let res = self.http.post(url).headers(headers).send().await?;
        if !res.status().is_success() {
            let status = res.status().as_u16();
            let body = res.text().await.unwrap_or_default();
            return Err(JellyfinError::Api {
                status,
                message: format!("QuickConnect initiate failed: {}", body),
            });
        }

        let initiate_resp = res.json::<QuickConnectInitiateResponse>().await?;
        Ok(initiate_resp)
    }

    /// Checks the authorization state of a QuickConnect code (`GET /QuickConnect/Connect`).
    pub async fn check_quick_connect(&self, secret: &str) -> Result<QuickConnectStateResponse> {
        let mut url = self.endpoint_url("QuickConnect/Connect")?;
        url.query_pairs_mut().append_pair("secret", secret);
        let headers = self.build_headers().await;

        let res = self.http.get(url).headers(headers).send().await?;
        if !res.status().is_success() {
            let status = res.status().as_u16();
            let body = res.text().await.unwrap_or_default();
            return Err(JellyfinError::Api {
                status,
                message: format!("QuickConnect connect check failed: {}", body),
            });
        }

        let state_resp = res.json::<QuickConnectStateResponse>().await?;
        Ok(state_resp)
    }

    /// Finalizes QuickConnect authentication and updates the client session (`POST /Users/AuthenticateWithQuickConnect`).
    pub async fn authenticate_quick_connect(
        &self,
        secret: &str,
    ) -> Result<QuickConnectAuthResponse> {
        let url = self.endpoint_url("Users/AuthenticateWithQuickConnect")?;
        let headers = self.build_headers().await;
        let req_body = QuickConnectAuthRequest {
            secret: secret.to_string(),
        };

        let res = self
            .http
            .post(url)
            .headers(headers)
            .json(&req_body)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status().as_u16();
            let body = res.text().await.unwrap_or_default();
            return Err(JellyfinError::Api {
                status,
                message: format!("QuickConnect authentication failed: {}", body),
            });
        }

        let auth_resp = res.json::<QuickConnectAuthResponse>().await?;

        // Update active session with the newly issued access token and user ID
        {
            let mut session = self.session.write().await;
            session.access_token = Some(auth_resp.access_token.clone());
            session.user_id = Some(auth_resp.user.id.clone());
            session.server_id = auth_resp.server_id.clone();
        }

        Ok(auth_resp)
    }

    // ------------------------------------------------------------------------
    // Library Browse API
    // ------------------------------------------------------------------------

    /// Browses library items requesting Video3DFormat, MediaStreams, and Path (`GET /Items`).
    pub async fn browse_items(&self, query: &ItemsQuery) -> Result<ItemsResponse> {
        let mut url = self.endpoint_url("Items")?;
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(user_id) = self.user_id().await {
                pairs.append_pair("UserId", &user_id);
            }
            if let Some(parent) = &query.parent_id {
                pairs.append_pair("ParentId", parent);
            }
            if !query.include_item_types.is_empty() {
                pairs.append_pair("IncludeItemTypes", &query.include_item_types.join(","));
            }
            if !query.fields.is_empty() {
                pairs.append_pair("Fields", &query.fields.join(","));
            }
            if let Some(sort_by) = &query.sort_by {
                pairs.append_pair("SortBy", sort_by);
            }
            if let Some(sort_order) = &query.sort_order {
                pairs.append_pair("SortOrder", sort_order);
            }
            if let Some(start) = query.start_index {
                pairs.append_pair("StartIndex", &start.to_string());
            }
            if let Some(lim) = query.limit {
                pairs.append_pair("Limit", &lim.to_string());
            }
            if query.recursive {
                pairs.append_pair("Recursive", "true");
            }
        }

        let headers = self.build_headers().await;
        let res = self.http.get(url).headers(headers).send().await?;

        if res.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(JellyfinError::Unauthorized);
        }
        if !res.status().is_success() {
            let status = res.status().as_u16();
            let body = res.text().await.unwrap_or_default();
            return Err(JellyfinError::Api {
                status,
                message: format!("Browse items failed: {}", body),
            });
        }

        let items_resp = res.json::<ItemsResponse>().await?;
        Ok(items_resp)
    }

    /// Fetches a specific item by ID (`GET /Items/{id}`).
    pub async fn get_item(&self, item_id: &str) -> Result<MediaItem> {
        let mut url = self.endpoint_url(&format!("Items/{}", item_id))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair(
                "Fields",
                "Video3DFormat,MediaStreams,Path,Overview,UserData",
            );
        }

        let headers = self.build_headers().await;
        let res = self.http.get(url).headers(headers).send().await?;

        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(JellyfinError::NotFound(item_id.to_string()));
        }
        if res.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(JellyfinError::Unauthorized);
        }
        if !res.status().is_success() {
            let status = res.status().as_u16();
            let body = res.text().await.unwrap_or_default();
            return Err(JellyfinError::Api {
                status,
                message: format!("Get item failed: {}", body),
            });
        }

        let item = res.json::<MediaItem>().await?;
        Ok(item)
    }

    // ------------------------------------------------------------------------
    // Playback & Device Profile API
    // ------------------------------------------------------------------------

    /// Requests playback info with the Quest device profile (`POST /Items/{id}/PlaybackInfo`).
    pub async fn get_playback_info(
        &self,
        item_id: &str,
        profile_override: Option<DeviceProfile>,
    ) -> Result<PlaybackInfoResponse> {
        let url = self.endpoint_url(&format!("Items/{}/PlaybackInfo", item_id))?;
        let headers = self.build_headers().await;

        let profile = profile_override.unwrap_or_else(quest_device_profile);
        let user_id = self.user_id().await;
        let req = PlaybackInfoRequest::new(user_id, profile);

        let res = self
            .http
            .post(url)
            .headers(headers)
            .json(&req)
            .send()
            .await?;

        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(JellyfinError::NotFound(item_id.to_string()));
        }
        if res.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(JellyfinError::Unauthorized);
        }
        if !res.status().is_success() {
            let status = res.status().as_u16();
            let body = res.text().await.unwrap_or_default();
            return Err(JellyfinError::Api {
                status,
                message: format!("PlaybackInfo request failed: {}", body),
            });
        }

        let resp = res.json::<PlaybackInfoResponse>().await?;
        Ok(resp)
    }

    /// Builds the direct play stream URL (`/Videos/{id}/stream?static=true`).
    pub async fn direct_play_url(&self, item_id: &str) -> Result<Url> {
        let token = self.token().await;
        build_direct_play_url(&self.base_url, item_id, token.as_deref())
    }

    // ------------------------------------------------------------------------
    // Session Reporting API
    // ------------------------------------------------------------------------

    /// Reports that playback has started (`POST /Sessions/Playing`).
    pub async fn report_playing(&self, req: &PlaybackStartRequest) -> Result<()> {
        let url = self.endpoint_url("Sessions/Playing")?;
        let headers = self.build_headers().await;

        let res = self
            .http
            .post(url)
            .headers(headers)
            .json(req)
            .send()
            .await?;
        if !res.status().is_success() {
            let status = res.status().as_u16();
            let body = res.text().await.unwrap_or_default();
            return Err(JellyfinError::Api {
                status,
                message: format!("Report playing failed: {}", body),
            });
        }
        Ok(())
    }

    /// Reports playback progress (`POST /Sessions/Playing/Progress`).
    pub async fn report_progress(&self, req: &PlaybackProgressRequest) -> Result<()> {
        let url = self.endpoint_url("Sessions/Playing/Progress")?;
        let headers = self.build_headers().await;

        let res = self
            .http
            .post(url)
            .headers(headers)
            .json(req)
            .send()
            .await?;
        if !res.status().is_success() {
            let status = res.status().as_u16();
            let body = res.text().await.unwrap_or_default();
            return Err(JellyfinError::Api {
                status,
                message: format!("Report progress failed: {}", body),
            });
        }
        Ok(())
    }

    /// Reports playback stopped (`POST /Sessions/Playing/Stopped`).
    pub async fn report_stopped(&self, req: &PlaybackStopRequest) -> Result<()> {
        let url = self.endpoint_url("Sessions/Playing/Stopped")?;
        let headers = self.build_headers().await;

        let res = self
            .http
            .post(url)
            .headers(headers)
            .json(req)
            .send()
            .await?;
        if !res.status().is_success() {
            let status = res.status().as_u16();
            let body = res.text().await.unwrap_or_default();
            return Err(JellyfinError::Api {
                status,
                message: format!("Report stopped failed: {}", body),
            });
        }
        Ok(())
    }

    // ------------------------------------------------------------------------
    // Format Detection Helper
    // ------------------------------------------------------------------------

    /// Determines the 3D stereo packing and projection format for a media item.
    pub fn detect_stereo_format(&self, item: &MediaItem) -> DetectedStereoFormat {
        detect_stereo_format(item)
    }
}
