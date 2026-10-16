use media_daemon::{
    application::{ActionExecutor, ApiToken, Dispatcher, ExecutorError},
    config::AppConfig,
    domain::ConfiguredAction,
    http::{HttpState, serve_tls},
};
use rcgen::generate_simple_self_signed;
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
async fn verified_localhost_https_executes_and_rejects_hostname_mismatch() {
    let certified = generate_simple_self_signed(vec!["localhost".to_owned()]).unwrap();
    let cert_pem = certified.cert.pem();
    let key_pem = certified.key_pair.serialize_pem();
    let root = reqwest::Certificate::from_pem(cert_pem.as_bytes()).unwrap();
    let client = reqwest::Client::builder()
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
        cert_pem.into_bytes(),
        key_pem.into_bytes(),
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
        .get(format!("https://127.0.0.1:{}/v1/media", address.port()))
        .send()
        .await;
    assert!(
        wrong_host.is_err(),
        "certificate without the IP SAN must not validate for the IP URL"
    );

    let now = now_ms();
    let body = serde_json::json!({
        "id": "0123456789abcdef0123456789abcdef",
        "cmd": "pause",
        "issued_at_ms": now,
        "expires_at_ms": now + 5000,
    });
    let response = client
        .post(format!("https://localhost:{}/v1/media", address.port()))
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
