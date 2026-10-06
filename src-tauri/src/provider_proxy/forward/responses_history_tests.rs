use super::*;
use serde_json::Value;
use sha2::{Digest, Sha256};

async fn search_history_server() -> (Upstream, Arc<Mutex<Vec<(String, Value)>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let (seen, saved) = (hits.clone(), requests.clone());
    let router = Router::new().fallback(any(move |request: Request<Body>| {
        let (seen, saved) = (seen.clone(), saved.clone());
        async move {
            seen.fetch_add(1, Ordering::SeqCst);
            let path = request.uri().to_string();
            let value: Value = serde_json::from_slice(&to_bytes(request.into_body(), 65536).await.unwrap()).unwrap();
            let valid = value["input"].as_array().unwrap().iter().all(|item| {
                (item["type"] != "tool_search_call" || item["id"].as_str().is_none_or(|id| id.starts_with("tsc_")))
                    && item["call_id"].as_str().is_none_or(|id| id.chars().count() <= 64)
            });
            saved.lock().unwrap().push((path, value));
            Response::builder().status(if valid { StatusCode::OK } else { StatusCode::BAD_REQUEST })
                .header("content-type", "application/json")
                .body(Body::from(if valid { r#"{"id":"resp_history","model":"fixture-model","status":"completed","output":[]}"# }
                    else { r#"{"error":{"message":"Invalid tool search item ID","type":"invalid_request_error"}}"# })).unwrap()
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (Upstream { url, hits, task }, requests)
}

#[tokio::test]
async fn responses_search_history_is_repaired_for_native_turns_and_compaction_after_overrides() {
    for (path, stripping, long_links) in [
        ("v1/responses", false, false),
        ("v1/responses/compact", false, false),
        ("responses", true, false),
        ("v1/responses", false, true),
        ("v1/responses/compact", false, true),
        ("responses", true, true),
    ] {
        let (upstream, requests) = search_history_server().await;
        let app = app(
            &[("p1", &upstream.url, vec![])],
            OptimizerConfig {
                codex_field_stripping: stripping,
                ..Default::default()
            },
        );
        let search_link = if long_links {
            "search-original-".repeat(6)
        } else {
            "search-original".into()
        };
        let shell_link = if long_links {
            "shell-original-".repeat(6)
        } else {
            "shell-original".into()
        };
        let input = json!([
            {"type":"message","role":"user","content":[{"type":"input_text","text":"find tools"}]},
            {"type":"tool_search_call","id":"fc_history","call_id":search_link,"status":"completed","execution":"client","arguments":{"query":"calendar"}},
            {"type":"tool_search_output","call_id":search_link,"status":"completed","execution":"client","tools":[{"type":"function","name":"calendar","parameters":{"type":"object"}}]},
            {"type":"tool_search_call","id":"tsc_existing","call_id":"search-other","arguments":{"query":"mail"}},
            {"type":"function_call","id":"fc_shell","call_id":shell_link,"name":"shell","arguments":"{}"},
            {"type":"function_call_output","call_id":shell_link,"output":"ok"},
            {"type":"reasoning","id":"rs_original","encrypted_content":"ciphertext+opaque=="}
        ]);
        {
            let db = app.state::<DbState>();
            let conn = db.0.lock().unwrap();
            conn.execute(
                "UPDATE config_profiles SET tool_id='codex',config_snapshot=?1 WHERE id='p1'",
                [json!({
                "config":format!("base_url = \"{}\"\n",upstream.url),"auth":{"OPENAI_API_KEY":"fixture-token"},"metadata":{
                    "localProxyModelAliases":[{"model":"fixture-model","upstream":"wire-model"}],
                    "localProxyRequestOverrides":{"body":{"input":input}}
                }})
                .to_string()],
            )
            .unwrap();
            conn.execute(
                "UPDATE app_settings SET value=?1 WHERE key='local_provider_proxy_settings'",
                [json!({"enabled_apps":["codex"],"port":34567}).to_string()],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO app_settings(key,value) VALUES('current_profile_codex','p1')",
                [],
            )
            .unwrap();
        }
        let request = Request::builder()
            .method("POST")
            .uri(format!("/proxy/codex/{path}?fixture=1"))
            .header("content-type", "application/json")
            .body(Body::from(
                json!({"model":"fixture-model","input":[],"stream":false,"store":false})
                    .to_string(),
            ))
            .unwrap();
        let response = tokio::time::timeout(
            Duration::from_secs(5),
            forward_proxy_request_with_client(
                app.handle().clone(),
                "codex".into(),
                path.into(),
                request,
                Some(reqwest::Client::builder().no_proxy().build().unwrap()),
            ),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let _ = to_bytes(response.into_body(), 65536).await.unwrap();
        assert_eq!(upstream.hits.load(Ordering::SeqCst), 1);
        let saved = requests.lock().unwrap();
        assert!(saved[0].0.ends_with("?fixture=1"));
        assert_eq!(saved[0].1["model"], "wire-model");
        let mut expected = input;
        expected[1]["id"] = json!("tsc_history");
        if long_links {
            for index in [1, 2] {
                expected[index]["call_id"] =
                    json!(format!("{:x}", Sha256::digest(search_link.as_bytes())));
            }
            for index in [4, 5] {
                expected[index]["call_id"] =
                    json!(format!("{:x}", Sha256::digest(shell_link.as_bytes())));
            }
        }
        assert_eq!(saved[0].1["input"], expected);
    }
}
