//! Jellyfin endpoints not exposed by jellyfin-client 0.1. Reuses its identity and token scoping.
use jellyfin_client::{
    authorization_header, BaseItem, ItemPage, ItemsQuery, JellyfinClient, StoredSession,
};
use reqwest::Method;
use serde_json::{json, Value};
use std::time::Duration;

async fn request(
    client: &JellyfinClient,
    method: Method,
    path: &str,
    query: &[(String, String)],
    body: Option<Value>,
    authenticated: bool,
) -> Result<Value, String> {
    let token = if authenticated {
        Some(
            client
                .access_token()
                .ok_or("Sign in to this account again.")?,
        )
    } else {
        None
    };
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Could not connect to Jellyfin.")?;
    let mut req = http
        .request(method, format!("{}{path}", client.base_url()))
        .header(
            "Authorization",
            authorization_header(client.identity(), token.as_deref()),
        )
        .header("Accept", "application/json")
        .query(query);
    if let Some(body) = body {
        req = req.json(&body);
    }
    // Never expose remote bodies or reqwest URLs: authentication errors may echo credentials.
    let mut response = req
        .send()
        .await
        .map_err(|_| "Could not reach Jellyfin. Try again.")?;
    if !response.status().is_success() {
        return Err(match response.status().as_u16() {
            401 | 403 => "Jellyfin did not authorize this account. Check your credentials or use Quick Connect.".into(),
            code => format!("Jellyfin request failed ({code}). Try again."),
        });
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Could not read Jellyfin's response.")?
    {
        if bytes.len() + chunk.len() > 4 * 1024 * 1024 {
            return Err("Jellyfin returned too much data.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| "Jellyfin returned an unreadable response.".into())
}

pub async fn public_users(client: &JellyfinClient) -> Result<Value, String> {
    let users = request(client, Method::GET, "/Users/Public", &[], None, false).await?;
    let users = users
        .as_array()
        .ok_or("Jellyfin returned an unreadable user list.")?;
    Ok(
        json!({"users":users.iter().filter_map(|u| Some(json!({"id":u.get("Id")?.as_str()?,"name":u.get("Name")?.as_str()?}))).collect::<Vec<_>>()}),
    )
}
pub async fn authenticate(
    client: &JellyfinClient,
    name: &str,
    password: &str,
) -> Result<StoredSession, String> {
    let result = request(
        client,
        Method::POST,
        "/Users/AuthenticateByName",
        &[],
        Some(json!({"Username":name,"Pw":password})),
        false,
    )
    .await?;
    let field = |v: &Value, key| {
        v.get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .ok_or("Jellyfin omitted account information.")
    };
    Ok(StoredSession {
        server_url: client.base_url().into(),
        device_id: client.identity().device_id().into(),
        access_token: field(&result, "AccessToken")?,
        user_id: field(&result["User"], "Id")?,
        user_name: field(&result["User"], "Name")?,
        server_id: field(&result, "ServerId")?,
    })
}
pub async fn validate_user(client: &JellyfinClient) -> Result<(), String> {
    let user = request(client, Method::GET, "/Users/Me", &[], None, true).await?;
    if user["Id"].as_str() != client.session().as_ref().map(|s| s.user_id.as_str()) {
        return Err("Jellyfin returned a different account. Sign in again.".into());
    }
    Ok(())
}
pub async fn stereo_items(
    client: &JellyfinClient,
    q: &ItemsQuery,
    resume: bool,
) -> Result<ItemPage, String> {
    let user = client
        .session()
        .ok_or("Sign in to browse your library.")?
        .user_id;
    let mut query = vec![
        ("UserId".into(), user),
        ("Is3D".into(), "true".into()),
        ("Recursive".into(), "true".into()),
        ("StartIndex".into(), q.start_index.to_string()),
        ("Limit".into(), q.limit.to_string()),
        (
            "Fields".into(),
            "Video3DFormat,MediaStreams,Path,Overview,PrimaryImageAspectRatio".into(),
        ),
        ("EnableImages".into(), "true".into()),
        ("EnableUserData".into(), "true".into()),
        ("EnableImageTypes".into(), "Primary,Backdrop".into()),
        ("IncludeItemTypes".into(), q.include_item_types.join(",")),
    ];
    for (name, value) in [
        ("ParentId", q.parent_id.as_ref()),
        ("SearchTerm", q.search_term.as_ref()),
        ("SortBy", q.sort_by.as_ref()),
    ] {
        if let Some(value) = value {
            query.push((name.into(), value.clone()));
        }
    }
    query.push((
        "SortOrder".into(),
        if q.descending {
            "Descending"
        } else {
            "Ascending"
        }
        .into(),
    ));
    if let Some(favorite) = q.is_favorite {
        query.push(("IsFavorite".into(), favorite.to_string()));
    }
    if resume {
        query.push(("Filters".into(), "IsResumable".into()));
    }
    let data = request(client, Method::GET, "/Items", &query, None, true).await?;
    let items: Vec<BaseItem> = serde_json::from_value(data["Items"].clone())
        .map_err(|_| "Jellyfin returned unreadable titles.")?;
    Ok(ItemPage {
        items,
        start_index: q.start_index,
        total_record_count: data["TotalRecordCount"]
            .as_u64()
            .ok_or("Jellyfin omitted the title count.")?,
    })
}

/// Jellyfin Tags carry explicit projection hints; absent hints do not turn ordinary movies into VR.
pub async fn spatial_tags(client: &JellyfinClient, item: &str) -> Result<Vec<String>, String> {
    let user = client
        .session()
        .ok_or("Sign in to read video metadata.")?
        .user_id;
    let mut url = reqwest::Url::parse("http://localhost/").expect("constant URL");
    url.path_segments_mut()
        .expect("constant hierarchical URL")
        .extend(["Users", &user, "Items", item]);
    let data = request(client, Method::GET, url.path(), &[], None, true).await?;
    Ok(data["Tags"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default())
}
