use crate::AppSessionStore;
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
        "status" => Ok(json!({"signed_in": client.session().is_some()})),
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
        "browse" => {
            let page = client
                .list_items(&ItemsQuery {
                    start_index: v["start"].as_u64().unwrap_or(0).min(u32::MAX as u64) as u32,
                    limit: 50,
                    recursive: true,
                    search_term: v["search"]
                        .as_str()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned),
                    include_item_types: vec!["Movie".into(), "Episode".into(), "Video".into()],
                    ..Default::default()
                })
                .await
                .map_err(|e| e.to_string())?;
            let items: Vec<_> = page.items.iter().map(|item| json!({"id":item.id,"name":item.name,"type":item.item_type,"format":item.video_3d_format.as_ref().map(ToString::to_string)})).collect();
            Ok(json!({"items":items,"next":page.next_start_index()}))
        }
        "prepare" => {
            let id = required(&v, "item")?;
            let item = client.get_item(id).await.map_err(|e| e.to_string())?;
            let info = client
                .playback_info(id, None)
                .await
                .map_err(|e| e.to_string())?;
            if let Some(err) = info.error_code {
                return Err(format!("Jellyfin rejected playback: {err}"));
            }
            let resolved = jellyfin_client::resolve_playback(client.base_url(), id, &info)
                .map_err(|e| e.to_string())?;
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
            let format = quest_jellyfin_player::detect_item_format(&item);
            let resume = item
                .user_data
                .as_ref()
                .map_or(0, |d| d.playback_position_ticks.max(0));
            Ok(
                json!({"url":url.as_str(),"authorization":authorization,"method":format!("{:?}",resolved.method),"source":source.id,"session":info.play_session_id,"resume":resume,"format":format!("{:?}",format.format),"eye_order":format!("{:?}",format.eye_order),"projection":format!("{:?}",format.projection),"width":width,"height":height}),
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
