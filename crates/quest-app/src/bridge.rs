use crate::AppSessionStore;
use base64::Engine;
use jellyfin_client::{
    ClientIdentity, ItemsQuery, JellyfinClient, PlaybackProgressRequest, PlaybackStartRequest,
    PlaybackStopRequest, QuickConnectPoll,
};
use serde_json::{json, Value};
use std::sync::Arc;

fn required<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Missing {key}"))
}

fn item_json(item: &jellyfin_client::BaseItem) -> Value {
    let user = item.user_data.as_ref();
    let own_poster = item.image_tags.get("Primary");
    let (image_id, image_tag) = if own_poster.is_some() {
        (Some(item.id.as_str()), own_poster.map(String::as_str))
    } else if item.series_primary_image_tag.is_some() {
        (
            item.series_id.as_deref(),
            item.series_primary_image_tag.as_deref(),
        )
    } else {
        (Some(item.id.as_str()), None)
    };
    json!({"id":item.id,"name":item.name,"type":item.item_type,
        "format":item.video_3d_format.as_ref().map(ToString::to_string),
        "overview":item.overview,"year":item.production_year,"rating":item.official_rating,
        "runtime":item.run_time_ticks,"series":item.series_name,"season":item.parent_index_number,
        "episode":item.index_number,"image_id":image_id,"image_tag":image_tag,
        "resume":user.map_or(0, |u| u.playback_position_ticks.max(0)),
        "favorite":user.is_some_and(|u| u.is_favorite),"played":user.is_some_and(|u| u.played)})
}

pub async fn run(v: Value) -> Result<Value, String> {
    let action = required(&v, "action")?;
    let store = Arc::new(AppSessionStore(required(&v, "store")?.into()));
    let identity = ClientIdentity::new(
        "Quest Jellyfin 3D",
        "Meta Quest",
        required(&v, "device")?,
        env!("CARGO_PKG_VERSION"),
    )
    .map_err(|e| e.to_string())?;
    let client =
        JellyfinClient::new(required(&v, "server")?, identity, store).map_err(|e| e.to_string())?;
    match action {
        "status" => Ok(
            json!({"signed_in": client.session().is_some(), "user":client.session().map(|s| s.user_name)}),
        ),
        "signout" => {
            client.clear_session().map_err(|e| e.to_string())?;
            Ok(json!({}))
        }
        "initiate" => {
            let challenge = client
                .initiate_quick_connect()
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({"code": challenge.code, "secret": challenge.secret}))
        }
        "poll" => match client
            .poll_quick_connect(required(&v, "secret")?)
            .await
            .map_err(|e| e.to_string())?
        {
            QuickConnectPoll::Pending { .. } => Ok(json!({"authorized":false})),
            QuickConnectPoll::Authorized(_) => Ok(json!({"authorized":true})),
        },
        "libraries" => {
            let page = client.library_views().await.map_err(|e| e.to_string())?;
            Ok(json!({"items":page.items.iter().map(item_json).collect::<Vec<_>>()}))
        }
        "resume" => {
            let page = client.resume_items().await.map_err(|e| e.to_string())?;
            Ok(json!({"items":page.items.iter().map(item_json).collect::<Vec<_>>()}))
        }
        "details" => {
            let item = client
                .get_item(required(&v, "item")?)
                .await
                .map_err(|e| e.to_string())?;
            Ok(item_json(&item))
        }
        "favorite" => {
            client
                .set_favorite(
                    required(&v, "item")?,
                    v["favorite"].as_bool().ok_or("Missing favorite flag")?,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({}))
        }
        "browse" => {
            let page = client
                .list_items(&ItemsQuery {
                    start_index: v["start"].as_u64().unwrap_or(0).min(u32::MAX as u64) as u32,
                    limit: v["limit"].as_u64().unwrap_or(50).clamp(1, 50) as u32,
                    recursive: true,
                    parent_id: v["parent"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned),
                    is_favorite: v["favorite"].as_bool(),
                    sort_by: Some(
                        match (v["kind"].as_str(), v["sort"].as_str()) {
                            (Some("episodes"), _) => "ParentIndexNumber,IndexNumber,SortName",
                            (_, Some("latest")) => "DateCreated",
                            _ => "SortName",
                        }
                        .into(),
                    ),
                    descending: v["kind"].as_str() != Some("episodes")
                        && v["sort"].as_str() == Some("latest"),
                    extra_fields: vec!["Overview".into(), "PrimaryImageAspectRatio".into()],
                    search_term: v["search"]
                        .as_str()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned),
                    include_item_types: match v["kind"].as_str() {
                        Some("movies") => vec!["Movie".into()],
                        Some("shows") => vec!["Series".into()],
                        Some("episodes") => vec!["Episode".into()],
                        Some("favorites") => vec![
                            "Movie".into(),
                            "Series".into(),
                            "Episode".into(),
                            "Video".into(),
                        ],
                        _ if v["search"].as_str().is_some_and(|s| !s.trim().is_empty()) => vec![
                            "Movie".into(),
                            "Series".into(),
                            "Episode".into(),
                            "Video".into(),
                        ],
                        _ => vec!["Movie".into(), "Series".into(), "Video".into()],
                    },
                    ..Default::default()
                })
                .await
                .map_err(|e| e.to_string())?;
            let items: Vec<_> = page.items.iter().map(item_json).collect();
            Ok(json!({"items":items,"next":page.next_start_index()}))
        }
        "image" => {
            let image = client
                .primary_image(required(&v, "item")?)
                .await
                .map_err(|e| e.to_string())?;
            Ok(
                json!({"image": image.map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes))}),
            )
        }
        "prepare" => {
            let id = required(&v, "item")?;
            let item = client.get_item(id).await.map_err(|e| e.to_string())?;
            let max_bitrate = v["max_bitrate"]
                .as_u64()
                .map(|n| n.clamp(4_000_000, 120_000_000));
            let allow_transcoding = v["allow_transcoding"].as_bool().unwrap_or(true);
            let info = client
                .playback_info_with_policy(id, None, max_bitrate, allow_transcoding)
                .await
                .map_err(|e| e.to_string())?;
            if let Some(err) = info.error_code {
                return Err(format!("Jellyfin rejected playback: {err}"));
            }
            let resolved = jellyfin_client::resolve_playback(client.base_url(), id, &info)
                .map_err(|e| e.to_string())?;
            if !allow_transcoding && resolved.method == jellyfin_client::PlayMethod::Transcode {
                return Err("This source requires server transcoding. Enable it in Playback settings or choose another source.".into());
            }
            let source = info
                .media_sources
                .iter()
                .find(|s| s.id == resolved.source_id)
                .ok_or("Selected source missing")?;
            let url = &resolved.url;
            let token = client.access_token().ok_or("Not signed in")?;
            let authorization =
                jellyfin_client::authorization_header(client.identity(), Some(&token));
            let stream = source
                .media_streams
                .iter()
                .find(|s| s.stream_type == Some(jellyfin_client::MediaStreamType::Video))
                .or_else(|| {
                    if info.media_sources.len() == 1 {
                        item.video_stream()
                    } else {
                        None
                    }
                });
            let width = stream.and_then(|s| s.width).filter(|v| *v > 0);
            let height = stream.and_then(|s| s.height).filter(|v| *v > 0);
            let mut source_item = item.clone();
            if !source.media_streams.is_empty() {
                source_item.media_streams = source.media_streams.clone();
            }
            if let Some(path) = &source.path {
                source_item.path = Some(path.clone());
            }
            let format = quest_jellyfin_player::detect_item_format(&source_item);
            let resume = item
                .user_data
                .as_ref()
                .map_or(0, |d| d.playback_position_ticks.max(0));
            Ok(
                json!({"title":item.name,"url":url.as_str(),"authorization":authorization,"method":format!("{:?}",resolved.method),"source":source.id,"session":info.play_session_id,"resume":resume,"format":format!("{:?}",format.format),"eye_order":format!("{:?}",format.eye_order),"projection":format!("{:?}",format.projection),"width":width,"height":height}),
            )
        }
        "start" | "progress" | "stop" => {
            let id = required(&v, "item")?.to_string();
            let session = v["session"].as_str().map(str::to_owned);
            let source = v["source"].as_str().map(str::to_owned);
            let ticks = v["ticks"].as_u64().unwrap_or(0);
            let method = match v["method"].as_str().unwrap_or("DirectPlay") {
                "DirectPlay" => jellyfin_client::PlayMethod::DirectPlay,
                "DirectStream" => jellyfin_client::PlayMethod::DirectStream,
                "Transcode" => jellyfin_client::PlayMethod::Transcode,
                _ => return Err("Invalid playback method".into()),
            };
            match action {
                "start" => {
                    let mut req = PlaybackStartRequest::direct_play(id, session, ticks);
                    req.media_source_id = source;
                    req.play_method = method;
                    client.report_playing(&req).await
                }
                "progress" => {
                    let mut req = PlaybackProgressRequest::time_update(
                        id,
                        session,
                        ticks,
                        v["paused"].as_bool().unwrap_or(false),
                    );
                    req.media_source_id = source;
                    req.play_method = method;
                    client.report_progress(&req).await
                }
                _ => {
                    let mut req = PlaybackStopRequest::new(id, session, ticks);
                    req.media_source_id = source;
                    client.report_stopped(&req).await
                }
            }
            .map_err(|e| e.to_string())?;
            Ok(json!({}))
        }
        _ => Err("Unknown action".into()),
    }
}
