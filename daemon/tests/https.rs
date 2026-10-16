use media_daemon::{
    application::{ActionExecutor, ApiToken, Dispatcher, ExecutorError},
    config::AppConfig,
    domain::ConfiguredAction,
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
    let state = HttpState {
        dispatcher: Arc::new(
            Dispatcher::new(Arc::new(NoopExecutor), AppConfig::default()).unwrap(),
        ),
        token,
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
    let reply: serde_json::Value =
        serde_json::from_slice(&response.bytes().await.unwrap()).unwrap();
    assert_eq!(reply["id"], "0123456789abcdef0123456789abcdef");
    assert_eq!(reply["status"], "executed");

    let _ = shutdown_tx.send(true);
    assert!(server.await.unwrap().is_ok());
}
