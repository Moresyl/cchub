use super::*;
use crate::db::schema;
use crate::usage_alerts::engine::tests::fixture;
use rusqlite::Connection;
use serde_json::json;
use std::sync::Mutex as StdMutex;
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;

fn app(url: &str, enabled: bool) -> tauri::App<MockRuntime> {
    let (mut state, mut profile) = fixture();
    profile.config_snapshot =
        json!({"env":{"ANTHROPIC_BASE_URL":url,"ANTHROPIC_AUTH_TOKEN":"fixture-token"}})
            .to_string();
    let mut settings = state.rules[&profile.id].settings.clone();
    settings.enabled = enabled;
    settings.system_notifications = false;
    storage::set_rule(&mut state, &profile, settings).unwrap();
    let conn = Connection::open_in_memory().unwrap();
    schema::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO config_profiles(id,name,tool_id,config_snapshot) VALUES(?1,?2,?3,?4)",
        rusqlite::params![
            profile.id,
            profile.name,
            profile.tool_id,
            profile.config_snapshot
        ],
    )
    .unwrap();
    storage::save(&conn, &state).unwrap();
    mock_builder()
        .manage(DbState(StdMutex::new(conn)))
        .manage(Arc::new(Runtime::default()))
        .build(mock_context(noop_assets()))
        .unwrap()
}

async fn request(socket: &mut tokio::net::TcpStream) -> String {
    let mut bytes = Vec::new();
    loop {
        let mut buffer = [0u8; 1024];
        let count = socket.read(&mut buffer).await.unwrap();
        assert!(count > 0);
        bytes.extend_from_slice(&buffer[..count]);
        assert!(bytes.len() < 32768);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(bytes).unwrap()
}

async fn respond(socket: &mut tokio::net::TcpStream) {
    let body = "{\"remaining\":1,\"unit\":\"USD\"}";
    socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).as_bytes()).await.unwrap();
    socket.shutdown().await.unwrap();
}

#[tokio::test]
async fn real_query_records_one_durable_alert_and_repeated_round_does_not_duplicate() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = request(&mut socket).await.to_ascii_lowercase();
            assert!(request.starts_with("get /usage "));
            assert!(request.contains("authorization: bearer fixture-token"));
            respond(&mut socket).await;
        }
    });
    let app = app(&url, true);
    for _ in 0..2 {
        check(app.handle()).await.unwrap();
    }
    let db = app.state::<DbState>();
    let state = storage::load(&db.0.lock().unwrap()).unwrap();
    assert_eq!(state.events.len(), 1);
    assert_eq!(state.events[0].event.value, 1.0);
    assert_eq!(state.events[0].event.system_status, "off");
    assert_eq!(state.rules["profile"].status, "ok");
    assert!(!app.state::<Arc<Runtime>>().polling());
    server.await.unwrap();
}

#[tokio::test]
async fn in_flight_round_does_not_hold_db_lock_and_drops_changed_account_results() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (started, observed) = oneshot::channel();
    let (resume, gate) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        request(&mut socket).await;
        started.send(()).unwrap();
        gate.await.unwrap();
        respond(&mut socket).await;
    });
    let app = app(&url, true);
    let handle = app.handle().clone();
    let round = tokio::spawn(async move { check(&handle).await });
    tokio::time::timeout(std::time::Duration::from_secs(5), observed)
        .await
        .unwrap()
        .unwrap();
    assert!(app.state::<Arc<Runtime>>().polling());
    assert!(check(app.handle())
        .await
        .unwrap_err()
        .contains("already running"));
    {
        let db = app.state::<DbState>();
        let conn =
            db.0.try_lock()
                .expect("network request must not hold the DB lock");
        conn.execute(
            "UPDATE config_profiles SET config_snapshot=?1 WHERE id='profile'",
            [
                json!({"env":{"ANTHROPIC_BASE_URL":url,"ANTHROPIC_AUTH_TOKEN":"new-account"}})
                    .to_string(),
            ],
        )
        .unwrap();
    }
    resume.send(()).unwrap();
    round.await.unwrap().unwrap();
    let db = app.state::<DbState>();
    let state = storage::load(&db.0.lock().unwrap()).unwrap();
    assert!(state.events.is_empty());
    assert_eq!(state.rules["profile"].checked_at, None);
    assert!(!app.state::<Arc<Runtime>>().polling());
    server.await.unwrap();
}

#[tokio::test]
async fn disabled_rules_do_not_query_and_failed_storage_leaves_polling_clear() {
    let app = app("http://127.0.0.1:9", false);
    check(app.handle()).await.unwrap();
    let db = app.state::<DbState>();
    {
        let conn = db.0.lock().unwrap();
        let state = storage::load(&conn).unwrap();
        assert!(state.events.is_empty());
        assert_eq!(state.rules["profile"].checked_at, None);
        conn.execute(
            "UPDATE app_settings SET value='corrupt' WHERE key='usage_alert_state'",
            [],
        )
        .unwrap();
    }
    assert!(check(app.handle()).await.is_err());
    assert!(!app.state::<Arc<Runtime>>().polling());
}

#[tokio::test]
async fn cancellation_releases_round_and_resets_polling() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (started, observed) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        request(&mut socket).await;
        started.send(()).unwrap();
        std::future::pending::<()>().await;
    });
    let app = app(&url, true);
    let handle = app.handle().clone();
    let round = tokio::spawn(async move { check(&handle).await });
    tokio::time::timeout(std::time::Duration::from_secs(5), observed)
        .await
        .unwrap()
        .unwrap();
    assert!(app.state::<Arc<Runtime>>().polling());
    round.abort();
    assert!(round.await.unwrap_err().is_cancelled());
    let runtime = app.state::<Arc<Runtime>>();
    assert!(!runtime.polling());
    assert!(runtime.round.try_lock().is_ok());
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn script_results_are_normalized_without_turning_failures_or_cached_data_into_alerts() {
    for payload in [
        json!({"success":false,"remaining":1,"unit":"USD"}),
        json!({"success":true,"asOf":"previous","data":[{"remaining":1,"unit":"USD"}]}),
        json!({"success":true,"data":[{"utilization":90,"accountId":"user","stale":true}]}),
    ] {
        let (mut state, mut profile) = fixture();
        profile.config_snapshot = json!({"env":{},"metadata":{"usageScript":{"enabled":true,"code":format!("console.log(JSON.stringify({payload}))")}}}).to_string();
        let settings = state.rules[&profile.id].settings.clone();
        storage::set_rule(&mut state, &profile, settings).unwrap();
        let result = query_profile_usage(&profile).await.unwrap();
        assert_eq!(result["success"], payload["success"]);
        assert!(result["data"].is_array());
        assert_eq!(engine::observe(&mut state, &profile, &result, 1000), 0);
    }
}
