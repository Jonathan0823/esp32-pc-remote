use media_daemon::{
    application::{ActionExecutor, ApiToken, Dispatcher, ExecutorError},
    config::{self, AppConfig},
    domain::{
        ActionId, ConfiguredAction, CustomAction, MediaCommand, PublishedVoiceConfig,
        VOICE_CONFIG_SCHEMA, VoiceCommand,
    },
    http::{HttpState, serve_tls},
    security::provision_tls,
};
use std::{
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::watch;

struct NoopExecutor;
impl ActionExecutor for NoopExecutor {
    fn execute<'a>(
        &'a self,
        _: &'a ConfiguredAction,
    ) -> Pin<Box<dyn Future<Output = Result<(), ExecutorError>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[tokio::test]
async fn provisioned_https_requires_matching_host_ca_and_token() {
    let tls = provision_tls("192.168.1.50".parse().unwrap()).unwrap();
    let root = reqwest::Certificate::from_pem(&tls.trusted_ca_certificate_pem).unwrap();
    let client = reqwest::Client::builder()
        .no_proxy()
        .resolve("wrong-host.invalid", "127.0.0.1:0".parse().unwrap())
        .add_root_certificate(root)
        .build()
        .unwrap();
    let token = ApiToken::parse(&"a".repeat(64)).unwrap();
    let action_id = ActionId::parse("music").unwrap();
    let app_config = AppConfig {
        actions: vec![CustomAction {
            id: action_id.clone(),
            name: "Music".into(),
            enabled: true,
            phrase: Some("play music".into()),
            action: ConfiguredAction::Media {
                command: MediaCommand::Play,
            },
        }],
        published_voice: Some(PublishedVoiceConfig {
            schema: VOICE_CONFIG_SCHEMA,
            revision: 1,
            actions_revision: 0,
            commands: vec![VoiceCommand {
                phrase: "play music".into(),
                action_id,
            }],
        }),
        ..AppConfig::default()
    };
    let config_directory = tempfile::tempdir().unwrap();
    let config_path = config_directory.path().join("config.json");
    let state = HttpState {
        dispatcher: Arc::new(Dispatcher::new(Arc::new(NoopExecutor), app_config.clone()).unwrap()),
        token,
        config: Arc::new(std::sync::RwLock::new(app_config)),
        config_path: Some(config_path.clone()),
        lifecycle: Arc::new(tokio::sync::Mutex::new(())),
    };
    let address: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = std::net::TcpListener::bind(address).unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let server = tokio::spawn(serve_tls(
        address,
        tls.certificate_pem,
        tls.server_private_key_pem,
        state,
        shutdown_rx,
    ));

    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if tokio::net::TcpStream::connect(address).await.is_ok() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let wrong_host = client
        .get(format!(
            "https://wrong-host.invalid:{}/v1/media",
            address.port()
        ))
        .send()
        .await;
    assert!(
        wrong_host.is_err(),
        "the provisioned CA must not bypass hostname verification"
    );
    let url = format!("https://127.0.0.1:{}/v1/media", address.port());
    let unpaired_client = reqwest::Client::builder().no_proxy().build().unwrap();
    assert!(
        unpaired_client.get(&url).send().await.is_err(),
        "a client without the exported CA must reject the daemon certificate"
    );
    assert!(
        unpaired_client
            .get(format!(
                "https://127.0.0.1:{}/v1/diagnostics",
                address.port()
            ))
            .send()
            .await
            .is_err(),
        "diagnostics must use the same validated HTTPS listener"
    );
    for authorization in [None, Some(format!("Bearer {}", "b".repeat(64)))] {
        let mut request = client.post(&url);
        if let Some(authorization) = authorization {
            request = request.header("Authorization", authorization);
        }
        assert_eq!(
            request.send().await.unwrap().status(),
            reqwest::StatusCode::UNAUTHORIZED,
            "CA trust alone must not authorize an unpaired client"
        );
    }

    let now = now_ms();
    let body = serde_json::json!({
        "id": "0123456789abcdef0123456789abcdef",
        "cmd": "pause",
        "issued_at_ms": now,
        "expires_at_ms": now + 5000,
    });
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", "a".repeat(64)))
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.headers()["connection"].to_str().unwrap(), "close");
    let reply: serde_json::Value =
        serde_json::from_slice(&response.bytes().await.unwrap()).unwrap();
    assert_eq!(reply["id"], "0123456789abcdef0123456789abcdef");
    assert_eq!(reply["status"], "executed");

    let voice_url = format!("https://127.0.0.1:{}/v1/voice-config", address.port());
    let response = client
        .get(&voice_url)
        .header("Authorization", format!("Bearer {}", "a".repeat(64)))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let voice_config: serde_json::Value =
        serde_json::from_slice(&response.bytes().await.unwrap()).unwrap();
    assert_eq!(voice_config["revision"], 1);
    assert_eq!(voice_config["commands"][0]["action_id"], "music");
    assert!(voice_config["commands"][0].get("action").is_none());

    let response = client
        .post(format!("{voice_url}/ack"))
        .header("Authorization", format!("Bearer {}", "a".repeat(64)))
        .header("Content-Type", "application/json")
        .body(r#"{"revision":1}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        config::load(&config_path)
            .unwrap()
            .acknowledged_voice_revision,
        Some(1)
    );

    let now = now_ms();
    let action_request = serde_json::json!({
        "id": "1123456789abcdef0123456789abcdef",
        "action_id": "music",
        "voice_revision": 1,
        "issued_at_ms": now,
        "expires_at_ms": now + 5000,
    });
    let response = client
        .post(format!("https://127.0.0.1:{}/v1/actions", address.port()))
        .header("Authorization", format!("Bearer {}", "a".repeat(64)))
        .header("Content-Type", "application/json")
        .body(action_request.to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let reply: serde_json::Value =
        serde_json::from_slice(&response.bytes().await.unwrap()).unwrap();
    assert_eq!(reply["status"], "dispatched");

    let diagnostics_url = format!("https://127.0.0.1:{}/v1/diagnostics", address.port());
    let event = serde_json::json!({"schema":1,"uptime_ms":1234,"unix_time_ms":null,"kind":"clock_state","category":"not_ready"});
    let diagnostics_body = serde_json::json!({"schema":1,"events":[event]});
    let unauthorized = client
        .post(&diagnostics_url)
        .header("Content-Type", "application/json")
        .body(diagnostics_body.to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), reqwest::StatusCode::UNAUTHORIZED);
    let malformed = client
        .post(&diagnostics_url)
        .header("Authorization", format!("Bearer {}", "a".repeat(64)))
        .header("Content-Type", "application/json")
        .body(r#"{"schema":2,"events":[]}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(malformed.status(), reqwest::StatusCode::BAD_REQUEST);
    let oversized = client
        .post(&diagnostics_url)
        .header("Authorization", format!("Bearer {}", "a".repeat(64)))
        .header("Content-Type", "application/json")
        .body(" ".repeat(4097))
        .send()
        .await
        .unwrap();
    assert_eq!(oversized.status(), reqwest::StatusCode::PAYLOAD_TOO_LARGE);
    let stored = client
        .post(&diagnostics_url)
        .header("Authorization", format!("Bearer {}", "a".repeat(64)))
        .header("Content-Type", "application/json")
        .body(diagnostics_body.to_string())
        .send()
        .await
        .unwrap();
    assert_eq!(stored.status(), reqwest::StatusCode::OK);
    let exported = client
        .get(&diagnostics_url)
        .header("Authorization", format!("Bearer {}", "a".repeat(64)))
        .send()
        .await
        .unwrap();
    assert_eq!(exported.status(), reqwest::StatusCode::OK);
    let evidence: serde_json::Value =
        serde_json::from_slice(&exported.bytes().await.unwrap()).unwrap();
    assert_eq!(evidence[0]["category"], "not_ready");
    assert!(evidence[0].get("message").is_none());

    let _ = shutdown_tx.send(true);
    assert!(server.await.unwrap().is_ok());
}
