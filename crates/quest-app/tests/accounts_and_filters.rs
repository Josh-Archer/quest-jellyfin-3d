use jellyfin_client::{authorization_header, ClientIdentity, StoredSession, TokenStore};
use quest_app::{bridge::run, AppSessionStore};
use serde_json::{json, Value};
use wiremock::{
    matchers::{body_json, method, path, query_param},
    Mock, MockServer, ResponseTemplate,
};

struct Fixture {
    server: MockServer,
    dir: std::path::PathBuf,
    base: Value,
}
impl Fixture {
    async fn new() -> Self {
        let server = MockServer::start().await;
        let dir = std::env::temp_dir().join(format!(
            "quest-accounts-{}-{}",
            std::process::id(),
            server.address().port()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let base = json!({"server":server.uri(),"device":"quest","store":dir.join("session.json")});
        let fixture = Self { server, dir, base };
        fixture.store().save(&fixture.session("a")).unwrap();
        fixture
    }
    fn store(&self) -> AppSessionStore {
        AppSessionStore(self.dir.join("session.json"))
    }
    fn session(&self, id: &str) -> StoredSession {
        StoredSession {
            server_url: self.server.uri(),
            device_id: "quest".into(),
            user_id: id.into(),
            user_name: format!("User {id}"),
            access_token: format!("token-{id}"),
            server_id: "server".into(),
        }
    }
    fn auth(&self, id: &str) -> impl wiremock::Match {
        let expected = authorization_header(
            &ClientIdentity::new(
                "Quest Jellyfin 3D",
                "Meta Quest",
                "quest",
                env!("CARGO_PKG_VERSION"),
            )
            .unwrap(),
            Some(&format!("token-{id}")),
        );
        move |request: &wiremock::Request| {
            request
                .headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                == Some(expected.as_str())
        }
    }
    async fn call(&self, action: &str, extra: Value) -> Result<Value, String> {
        let mut v = self.base.clone();
        v["action"] = json!(action);
        for (k, val) in extra.as_object().unwrap() {
            v[k] = val.clone();
        }
        run(v).await
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[tokio::test]
async fn legacy_session_migrates_without_losing_users_and_signout_only_removes_current() {
    let f = Fixture::new().await;
    std::fs::write(&f.store().0, serde_json::to_vec(&f.session("a")).unwrap()).unwrap();
    assert_eq!(f.store().load().unwrap(), Some(f.session("a")));
    f.store().save(&f.session("b")).unwrap();
    assert_eq!(
        f.store().sessions(&f.server.uri(), "quest").unwrap().len(),
        2
    );
    assert!(f
        .store()
        .sessions("http://other-server", "quest")
        .unwrap()
        .is_empty());
    assert!(f
        .store()
        .sessions(&f.server.uri(), "other-device")
        .unwrap()
        .is_empty());
    f.store().save(&f.session("a")).unwrap();
    f.store().clear().unwrap();
    assert!(f.store().load().unwrap().is_none());
    assert!(f
        .store()
        .find(&f.server.uri(), "quest", "a")
        .unwrap()
        .is_none());
    assert_eq!(
        f.store().find(&f.server.uri(), "quest", "b").unwrap(),
        Some(f.session("b"))
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(f.store().0).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[tokio::test]
async fn authenticate_remember_switch_and_bind_delayed_requests_to_original_user() {
    let f = Fixture::new().await;
    Mock::given(method("GET"))
        .and(path("/Users/Public"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!([{"Id":"b","Name":"User b","Secret":"never-return"}])),
        )
        .mount(&f.server)
        .await;
    assert_eq!(
        f.call("public_users", json!({})).await.unwrap(),
        json!({"users":[{"id":"b","name":"User b"}]})
    );
    Mock::given(method("POST"))
        .and(path("/Users/AuthenticateByName"))
        .and(body_json(
            json!({"Username":"User b","Pw":"secret-password"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"AccessToken":"token-b","ServerId":"server","User":{"Id":"b","Name":"User b"}}),
        ))
        .mount(&f.server)
        .await;
    let result = f
        .call(
            "login",
            json!({"username":"User b","password":"secret-password"}),
        )
        .await
        .unwrap();
    assert_eq!(result, json!({"user":"User b","user_id":"b"}));
    assert!(!std::fs::read_to_string(f.store().0)
        .unwrap()
        .contains("secret-password"));
    let profiles = f.call("profiles", json!({})).await.unwrap();
    assert_eq!(profiles["users"].as_array().unwrap().len(), 2);
    assert!(!profiles.to_string().contains("token-"));
    Mock::given(method("GET"))
        .and(path("/Users/Me"))
        .and(f.auth("a"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"Id":"a"})))
        .expect(1)
        .mount(&f.server)
        .await;
    f.call("switch_user", json!({"target_user":"a"}))
        .await
        .unwrap();
    assert_eq!(f.call("status", json!({})).await.unwrap()["user_id"], "a");
    Mock::given(method("GET"))
        .and(path("/Items"))
        .and(f.auth("b"))
        .and(query_param("UserId", "b"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"Items":[],"TotalRecordCount":0})),
        )
        .expect(1)
        .mount(&f.server)
        .await;
    f.call("browse", json!({"user_id":"b"})).await.unwrap();
    f.call("signout", json!({})).await.unwrap();
    assert_eq!(
        f.call("status", json!({})).await.unwrap()["signed_in"],
        false
    );
    assert!(f
        .store()
        .find(&f.server.uri(), "quest", "b")
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn failed_auth_and_expired_or_mismatched_switch_leave_active_user_untouched() {
    let f = Fixture::new().await;
    Mock::given(method("POST"))
        .and(path("/Users/AuthenticateByName"))
        .respond_with(ResponseTemplate::new(401).set_body_string("echo-secret-password token-a"))
        .mount(&f.server)
        .await;
    let error = f
        .call(
            "login",
            json!({"username":"nope","password":"echo-secret-password"}),
        )
        .await
        .unwrap_err();
    assert!(!error.contains("echo-secret-password") && !error.contains("token-a"));
    f.store().save(&f.session("b")).unwrap();
    f.store().save(&f.session("a")).unwrap();
    Mock::given(method("GET"))
        .and(path("/Users/Me"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&f.server)
        .await;
    assert!(f
        .call("switch_user", json!({"target_user":"b"}))
        .await
        .is_err());
    assert_eq!(f.store().load().unwrap().unwrap().user_id, "a");
    f.server.reset().await;
    Mock::given(method("GET"))
        .and(path("/Users/Me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"Id":"someone-else"})))
        .mount(&f.server)
        .await;
    assert!(f
        .call("switch_user", json!({"target_user":"b"}))
        .await
        .is_err());
    assert_eq!(f.store().load().unwrap().unwrap().user_id, "a");
}

#[tokio::test]
async fn stereo_filter_is_server_side_and_preserves_search_favorites_scope_and_paging() {
    let f = Fixture::new().await;
    Mock::given(method("GET")).and(path("/Items")).and(query_param("Is3D","true")).and(query_param("UserId","a"))
        .and(query_param("StartIndex","50")).and(query_param("Limit","2")).and(query_param("ParentId","library"))
        .and(query_param("IsFavorite","true")).and(query_param("SearchTerm","Avatar")).and(query_param("SortBy","DateCreated"))
        .and(query_param("SortOrder","Descending")).and(f.auth("a"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"Items":[{"Id":"one","Name":"One","Video3DFormat":"HalfSideBySide"},{"Id":"two","Name":"Two","Video3DFormat":"FullTopAndBottom"}],"TotalRecordCount":100}))).expect(1).mount(&f.server).await;
    let page=f.call("browse",json!({"three_d_only":true,"user_id":"a","start":50,"limit":2,"parent":"library","favorite":true,"search":" Avatar ","sort":"latest"})).await.unwrap();
    assert_eq!(page["next"], 52);
    assert_eq!(page["items"][1]["format"], "FullTopAndBottom");
}

#[tokio::test]
async fn stereo_home_resume_and_shows_query_playable_episodes_not_series_containers() {
    let f = Fixture::new().await;
    Mock::given(method("GET")).and(path("/Items")).and(query_param("Is3D","true")).and(query_param("Filters","IsResumable"))
        .and(query_param("SortBy","DatePlayed")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"Items":[{"Id":"resume","Name":"Resume","Video3DFormat":"HalfSideBySide"}],"TotalRecordCount":1}))).expect(1).mount(&f.server).await;
    assert_eq!(
        f.call("resume", json!({"three_d_only":true}))
            .await
            .unwrap()["items"][0]["id"],
        "resume"
    );
    Mock::given(method("GET"))
        .and(path("/Items"))
        .and(query_param("Is3D", "true"))
        .and(query_param("IncludeItemTypes", "Episode"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"Items":[],"TotalRecordCount":0})),
        )
        .expect(1)
        .mount(&f.server)
        .await;
    let page = f
        .call("browse", json!({"three_d_only":true,"kind":"shows"}))
        .await
        .unwrap();
    assert_eq!(page["items"], json!([]));
    assert!(page["next"].is_null());
}

#[tokio::test]
async fn new_auth_endpoints_preserve_server_subpaths_and_refuse_redirects() {
    let f = Fixture::new().await;
    let other = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/jellyfin/Users/AuthenticateByName"))
        .respond_with(
            ResponseTemplate::new(307).insert_header("Location", format!("{}/steal", other.uri())),
        )
        .expect(1)
        .mount(&f.server)
        .await;
    let mut request = f.base.clone();
    request["server"] = json!(format!("{}/jellyfin/", f.server.uri()));
    request["action"] = json!("login");
    request["username"] = json!("a");
    request["password"] = json!("private");
    let error = run(request).await.unwrap_err();
    assert!(!error.contains("private"));
    assert!(other.received_requests().await.unwrap().is_empty());
}
