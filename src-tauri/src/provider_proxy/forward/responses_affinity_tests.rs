use super::*;
use bytes::Bytes;
use serde_json::Value;

type Captured = Arc<Mutex<Vec<Bytes>>>;

async fn native_server(profile: &'static str) -> (Upstream, Captured) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let (seen, saved) = (hits.clone(), requests.clone());
    let router = Router::new().fallback(any(move |request: Request<Body>| {
        let (seen, saved) = (seen.clone(), saved.clone());
        async move {
            seen.fetch_add(1, Ordering::SeqCst);
            let body = to_bytes(request.into_body(), 65536).await.unwrap();
            saved.lock().unwrap().push(body);
            Response::builder()
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"id":format!("resp_{profile}"),"model":"fixture-model",
                    "status":"completed","output":[],"served":profile})
                    .to_string(),
                ))
                .unwrap()
        }
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (Upstream { url, hits, task }, requests)
}

fn native_app(first: &Upstream, second: &Upstream) -> App<MockRuntime> {
    let app = app(
        &[("p1", &first.url, vec![]), ("p2", &second.url, vec![])],
        OptimizerConfig {
            codex_field_stripping: false,
            ..Default::default()
        },
    );
    {
        let db = app.state::<DbState>();
        let conn = db.0.lock().unwrap();
        for (id, url) in [("p1", &first.url), ("p2", &second.url)] {
            conn.execute(
                "UPDATE config_profiles SET tool_id='codex',config_snapshot=?1 WHERE id=?2",
                rusqlite::params![
                    json!({"config":format!("base_url = \"{url}\"\n"),
                    "auth":{"OPENAI_API_KEY":"fixture-token"}})
                    .to_string(),
                    id
                ],
            )
            .unwrap();
        }
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
    affinity_tests::configure_tool(&app, "codex", "turn", &["p1", "p2"]);
    app
}

async fn send(app: &App<MockRuntime>, body: Bytes) -> String {
    let request = Request::builder()
        .method("POST")
        .uri("/proxy/codex/v1/responses")
        .header("content-type", "application/json")
        .header("x-session-id", "native-turn")
        .body(Body::from(body))
        .unwrap();
    let response = tokio::time::timeout(
        Duration::from_secs(5),
        forward_proxy_request_with_client(
            app.handle().clone(),
            "codex".into(),
            "v1/responses".into(),
            request,
            Some(reqwest::Client::builder().no_proxy().build().unwrap()),
        ),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    serde_json::from_slice::<Value>(&bytes).unwrap()["served"]
        .as_str()
        .unwrap()
        .into()
}

fn body(input: Value) -> Bytes {
    Bytes::from(json!({"model":"fixture-model","stream":false,"input":input}).to_string())
}

fn prompt() -> Value {
    json!({"type":"message","role":"user","content":[{"type":"input_text","text":"find tools"}]})
}

#[tokio::test]
async fn native_function_custom_and_search_results_keep_the_successful_route() {
    for kind in [
        "function_call_output",
        "custom_tool_call_output",
        "tool_search_output",
    ] {
        let (first, first_requests) = native_server("p1").await;
        let (second, _) = native_server("p2").await;
        let app = native_app(&first, &second);
        assert_eq!(send(&app, body(json!([prompt()]))).await, "p1");
        let result =
            json!({"type":kind,"call_id":"original-call","output":"exact result","tools":[]});
        let continued = body(json!([prompt(), result]));
        for _ in 0..2 {
            assert_eq!(send(&app, continued.clone()).await, "p1", "{kind}");
        }
        // A call from a previous response need not be repeated in input.
        let partial = Bytes::from(
            json!({"model":"fixture-model","stream":false,
            "previous_response_id":"resp_p1","input":[result]})
            .to_string(),
        );
        assert_eq!(send(&app, partial.clone()).await, "p1", "{kind}");
        let saved = first_requests.lock().unwrap();
        assert_eq!(saved[1], continued);
        assert_eq!(saved[3], partial);
        assert_eq!(second.hits.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn native_notifications_release_a_paired_turn_without_rewriting_the_payload() {
    for kind in ["function_call_output", "custom_tool_call_output"] {
        for call_id in [None, Some("")] {
            let (first, _) = native_server("p1").await;
            let (second, requests) = native_server("p2").await;
            let app = native_app(&first, &second);
            assert_eq!(send(&app, body(json!([prompt()]))).await, "p1");
            let paired =
                json!({"type":"function_call_output","call_id":"original-call","output":"paired"});
            for _ in 0..2 {
                assert_eq!(send(&app, body(json!([prompt(), paired]))).await, "p1");
            }
            let mut notification = json!({"type":kind,"name":"send_message_to_thread",
                "namespace":"client","output":"independent notice"});
            if let Some(call_id) = call_id {
                notification["call_id"] = json!(call_id);
            }
            let request = body(json!([prompt(), paired, notification]));
            assert_eq!(
                send(&app, request.clone()).await,
                "p2",
                "{kind}/{call_id:?}"
            );
            assert_eq!(requests.lock().unwrap()[0], request);
        }
    }
}

#[tokio::test]
async fn native_custom_grammar_and_opaque_reasoning_keep_their_original_bytes() {
    let (first, requests) = native_server("p1").await;
    let (second, _) = native_server("p2").await;
    let app = native_app(&first, &second);
    let source = br#"{ "model": "fixture-model", "stream": false, "input": [
        {"type":"custom_tool_call","call_id":"custom","name":"patch","namespace":"workspace","input":"line1\nline2"},
        {"type":"custom_tool_call_output","call_id":"custom","output":[{"type":"input_text","text":"done"}]},
        {"type":"reasoning","encrypted_content":"ciphertext\u003d\u003d","signature":"opaque\\signature"},
        {"type":"tool_search_output","execution":"server","call_id":null,"tools":[]}
      ], "tools": [{"type":"custom","name":"patch","format":{"type":"grammar","syntax":"lark","definition":"start: WORD\nWORD: /[a-z]+/"}}],
      "vendor": {"large":184467440737095516160,"decimal":1.2300e+15} }"#;
    let request = Bytes::copy_from_slice(source);
    assert_eq!(send(&app, request.clone()).await, "p1");
    assert_eq!(requests.lock().unwrap()[0], request);
}
