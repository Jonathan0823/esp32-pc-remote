use crate::{
    application::{ApiToken, DispatchError, DispatchStatus, Dispatcher},
    config::{self, AppConfig},
    domain::{self, CustomActionRequest, MediaRequest, VoiceConfigAckRequest},
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use axum_server::{accept::Accept, tls_rustls::RustlsAcceptor};
use hyper_util::rt::TokioTimer;
use serde::Serialize;
use std::{
    future::{Ready, ready},
    io,
    net::SocketAddr,
    path::PathBuf,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
    sync::{Mutex as AsyncMutex, OwnedSemaphorePermit, Semaphore},
};
use tower_http::timeout::TimeoutLayer;

#[derive(Clone)]
pub struct HttpState {
    pub dispatcher: Arc<Dispatcher>,
    pub token: ApiToken,
    pub config: Arc<std::sync::RwLock<AppConfig>>,
    pub config_path: Option<PathBuf>,
    pub lifecycle: Arc<AsyncMutex<()>>,
}

#[derive(Serialize)]
struct Reply<'a> {
    id: &'a str,
    status: &'a str,
}

#[derive(Serialize)]
struct ErrorReply {
    status: &'static str,
}

pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/v1/media", post(media))
        .route("/v1/actions", post(custom_action))
        .route("/v1/voice-config", get(voice_config))
        .route("/v1/voice-config/ack", post(voice_config_ack))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(2),
        ))
        .layer(DefaultBodyLimit::max(domain::BODY_LIMIT))
        .with_state(state)
}

async fn media(State(state): State<HttpState>, headers: HeaderMap, body: Bytes) -> Response {
    if !authorized(&state, &headers) {
        return error(StatusCode::UNAUTHORIZED, "unauthorized");
    }
    if !json_content_type(&headers) {
        return error(StatusCode::BAD_REQUEST, "content_type");
    }
    if !valid_framing(&headers, &body) {
        return error(StatusCode::BAD_REQUEST, "invalid_framing");
    }
    let request: MediaRequest = match domain::parse_request(&body) {
        Ok(request) => request,
        Err(domain::ValidationError::TooLarge) => {
            return error(StatusCode::PAYLOAD_TOO_LARGE, "too_large");
        }
        Err(_) => return error(StatusCode::BAD_REQUEST, "invalid_request"),
    };
    let id = request.id.clone();
    match state.dispatcher.dispatch_media(request, now_ms()).await {
        Ok(DispatchStatus::Executed) => json(
            StatusCode::OK,
            &Reply {
                id: id.as_str(),
                status: "executed",
            },
        ),
        Err(err) => dispatch_error(err),
        _ => error(StatusCode::INTERNAL_SERVER_ERROR, "server_error"),
    }
}

async fn custom_action(
    State(state): State<HttpState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !authorized(&state, &headers) {
        return error(StatusCode::UNAUTHORIZED, "unauthorized");
    }
    if !json_content_type(&headers) {
        return error(StatusCode::BAD_REQUEST, "content_type");
    }
    if !valid_framing(&headers, &body) {
        return error(StatusCode::BAD_REQUEST, "invalid_framing");
    }
    let request: CustomActionRequest = match domain::parse_request(&body) {
        Ok(request) => request,
        Err(domain::ValidationError::TooLarge) => {
            return error(StatusCode::PAYLOAD_TOO_LARGE, "too_large");
        }
        Err(_) => return error(StatusCode::BAD_REQUEST, "invalid_request"),
    };
    let id = request.id.clone();
    match state.dispatcher.dispatch_custom(request, now_ms()).await {
        Ok(DispatchStatus::Dispatched) => json(
            StatusCode::OK,
            &Reply {
                id: id.as_str(),
                status: "dispatched",
            },
        ),
        Err(err) => dispatch_error(err),
        _ => error(StatusCode::INTERNAL_SERVER_ERROR, "server_error"),
    }
}

async fn voice_config(State(state): State<HttpState>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return error(StatusCode::UNAUTHORIZED, "unauthorized");
    }
    let config = match state.config.read() {
        Ok(config) => config.clone(),
        Err(_) => return error(StatusCode::INTERNAL_SERVER_ERROR, "server_error"),
    };
    let Some(published) = config.published_voice else {
        return error(StatusCode::NOT_FOUND, "not_applied");
    };
    if published.actions_revision != config.actions_revision {
        return error(StatusCode::CONFLICT, "apply_required");
    }
    json_limited(StatusCode::OK, &published, domain::VOICE_CONFIG_LIMIT)
}

#[derive(Serialize)]
struct VoiceAckReply {
    revision: u64,
    status: &'static str,
}

async fn voice_config_ack(
    State(state): State<HttpState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !authorized(&state, &headers) {
        return error(StatusCode::UNAUTHORIZED, "unauthorized");
    }
    if !json_content_type(&headers) || !valid_framing(&headers, &body) {
        return error(StatusCode::BAD_REQUEST, "invalid_framing");
    }
    let request: VoiceConfigAckRequest = match domain::parse_request(&body) {
        Ok(request) => request,
        Err(_) => return error(StatusCode::BAD_REQUEST, "invalid_request"),
    };
    let _lifecycle = state.lifecycle.lock().await;
    let mut config = match state.config.read() {
        Ok(config) => config.clone(),
        Err(_) => return error(StatusCode::INTERNAL_SERVER_ERROR, "server_error"),
    };
    let Some(published) = config.published_voice.as_ref() else {
        return error(StatusCode::CONFLICT, "not_applied");
    };
    if published.revision != request.revision
        || published.actions_revision != config.actions_revision
    {
        return error(StatusCode::CONFLICT, "stale_revision");
    }
    if config.acknowledged_voice_revision == Some(request.revision) {
        return json(
            StatusCode::OK,
            &VoiceAckReply {
                revision: request.revision,
                status: "active",
            },
        );
    }
    config.acknowledged_voice_revision = Some(request.revision);
    let Some(path) = state.config_path.as_ref() else {
        return error(StatusCode::INTERNAL_SERVER_ERROR, "server_error");
    };
    if config::save(path, &config).is_err()
        || state
            .dispatcher
            .replace_config(config.clone())
            .await
            .is_err()
    {
        return error(StatusCode::INTERNAL_SERVER_ERROR, "server_error");
    }
    match state.config.write() {
        Ok(mut current) => *current = config,
        Err(_) => return error(StatusCode::INTERNAL_SERVER_ERROR, "server_error"),
    }
    json(
        StatusCode::OK,
        &VoiceAckReply {
            revision: request.revision,
            status: "active",
        },
    )
}

fn authorized(state: &HttpState, headers: &HeaderMap) -> bool {
    state.token.verify_header(
        headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok()),
    )
}

fn valid_framing(headers: &HeaderMap, body: &[u8]) -> bool {
    !headers.contains_key(header::TRANSFER_ENCODING)
        && headers
            .get(header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<usize>().ok())
            .is_some_and(|length| length == body.len() && length <= domain::BODY_LIMIT)
}

fn json_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
}

fn dispatch_error(error_value: DispatchError) -> Response {
    let (status, label) = match error_value {
        DispatchError::InvalidTime => (StatusCode::BAD_REQUEST, "invalid_time"),
        DispatchError::Expired => (StatusCode::GONE, "expired"),
        DispatchError::ClockSkew => (StatusCode::BAD_REQUEST, "clock_skew"),
        DispatchError::Duplicate => (StatusCode::CONFLICT, "duplicate"),
        DispatchError::Busy | DispatchError::ExecutionBusy => {
            (StatusCode::TOO_MANY_REQUESTS, "busy")
        }
        DispatchError::UnknownAction => (StatusCode::NOT_FOUND, "unknown_action"),
        DispatchError::StaleVoiceConfig => (StatusCode::CONFLICT, "sync_required"),
        DispatchError::DisabledAction => (StatusCode::FORBIDDEN, "disabled_action"),
        DispatchError::Execution | DispatchError::Configuration => {
            (StatusCode::INTERNAL_SERVER_ERROR, "server_error")
        }
    };
    error(status, label)
}

fn json<T: Serialize>(status: StatusCode, value: &T) -> Response {
    json_limited(status, value, domain::BODY_LIMIT)
}

fn json_limited<T: Serialize>(status: StatusCode, value: &T, limit: usize) -> Response {
    let body = match serde_json::to_vec(value) {
        Ok(body) if body.len() <= limit => body,
        _ => return error(StatusCode::INTERNAL_SERVER_ERROR, "server_error"),
    };
    let body_length = body.len();
    let mut response = (status, body).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    if let Ok(length) = HeaderValue::from_str(&body_length.to_string()) {
        response
            .headers_mut()
            .insert(header::CONTENT_LENGTH, length);
    }
    response
}

fn error(status: StatusCode, label: &'static str) -> Response {
    json(status, &ErrorReply { status: label })
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

#[derive(Clone)]
struct LimitedAcceptor {
    permits: Arc<Semaphore>,
}

impl LimitedAcceptor {
    fn new(max_connections: usize) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(max_connections)),
        }
    }
}

struct LimitedStream {
    stream: TcpStream,
    _permit: OwnedSemaphorePermit,
}

impl<S> Accept<TcpStream, S> for LimitedAcceptor {
    type Stream = LimitedStream;
    type Service = S;
    type Future = Ready<io::Result<(Self::Stream, Self::Service)>>;

    fn accept(&self, stream: TcpStream, service: S) -> Self::Future {
        let result = self
            .permits
            .clone()
            .try_acquire_owned()
            .map(|permit| {
                (
                    LimitedStream {
                        stream,
                        _permit: permit,
                    },
                    service,
                )
            })
            .map_err(|_| io::Error::new(io::ErrorKind::WouldBlock, "connection limit reached"));
        ready(result)
    }
}

impl AsyncRead for LimitedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(context, buffer)
    }
}

impl AsyncWrite for LimitedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(context, buffer)
    }
    fn poll_flush(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(context)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(context)
    }
    fn is_write_vectored(&self) -> bool {
        self.stream.is_write_vectored()
    }
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffers: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write_vectored(context, buffers)
    }
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("TLS configuration failed")]
    Tls(#[source] std::io::Error),
    #[error("HTTPS server failed: {0}")]
    Serve(#[from] std::io::Error),
    #[error("HTTPS server task failed: {0}")]
    Join(String),
}

pub struct ListenerTask {
    shutdown: tokio::sync::watch::Sender<bool>,
    task: tokio::task::JoinHandle<Result<(), ServerError>>,
}

impl ListenerTask {
    pub async fn start(
        address: SocketAddr,
        cert_pem: Vec<u8>,
        key_pem: Vec<u8>,
        state: HttpState,
    ) -> Result<Self, ServerError> {
        let listener = tokio::net::TcpListener::bind(address).await?;
        let listener = listener.into_std()?;
        let tls = axum_server::tls_rustls::RustlsConfig::from_pem(cert_pem, key_pem)
            .await
            .map_err(ServerError::Tls)?;
        let (shutdown, receiver) = tokio::sync::watch::channel(false);
        let task = tokio::spawn(serve_listener(listener, tls, state, receiver));
        Ok(Self { shutdown, task })
    }

    pub fn request_shutdown(&self) {
        let _ = self.shutdown.send(true);
    }

    pub async fn stop(self) -> Result<(), ServerError> {
        self.request_shutdown();
        self.task
            .await
            .map_err(|error| ServerError::Join(error.to_string()))?
    }
}

pub async fn serve_tls(
    address: SocketAddr,
    cert_pem: Vec<u8>,
    key_pem: Vec<u8>,
    state: HttpState,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), ServerError> {
    let listener = tokio::net::TcpListener::bind(address).await?;
    serve_listener(
        listener.into_std()?,
        axum_server::tls_rustls::RustlsConfig::from_pem(cert_pem, key_pem)
            .await
            .map_err(ServerError::Tls)?,
        state,
        shutdown,
    )
    .await
}

async fn serve_listener(
    listener: std::net::TcpListener,
    tls: axum_server::tls_rustls::RustlsConfig,
    state: HttpState,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), ServerError> {
    let handle = axum_server::Handle::new();
    let shutdown_handle = handle.clone();
    tokio::spawn(async move {
        let mut shutdown = shutdown;
        if shutdown.changed().await.is_ok() && *shutdown.borrow() {
            shutdown_handle.graceful_shutdown(Some(std::time::Duration::from_secs(2)));
        }
    });
    let mut server = axum_server::Server::from_tcp(listener);
    server
        .http_builder()
        .http1()
        .timer(TokioTimer::new())
        .header_read_timeout(Duration::from_secs(5))
        .keep_alive(false);
    let acceptor = RustlsAcceptor::new(tls)
        .handshake_timeout(Duration::from_secs(2))
        .acceptor(LimitedAcceptor::new(8));
    server
        .acceptor(acceptor)
        .handle(handle)
        .serve(router(state).into_make_service())
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{ActionExecutor, ExecutorError},
        config::AppConfig,
        domain::ConfiguredAction,
    };
    use std::{future::Future, pin::Pin};

    struct Noop;
    impl ActionExecutor for Noop {
        fn execute<'a>(
            &'a self,
            _: &'a ConfiguredAction,
        ) -> Pin<Box<dyn Future<Output = Result<(), ExecutorError>> + Send + 'a>> {
            Box::pin(async { Ok(()) })
        }
    }

    fn published_config() -> AppConfig {
        let action_id = domain::ActionId::parse("music").unwrap();
        AppConfig {
            actions: vec![domain::CustomAction {
                id: action_id.clone(),
                name: "Music".into(),
                enabled: true,
                phrase: Some("play my music".into()),
                action: ConfiguredAction::Media {
                    command: domain::MediaCommand::Play,
                },
            }],
            published_voice: Some(domain::PublishedVoiceConfig {
                schema: domain::VOICE_CONFIG_SCHEMA,
                revision: 1,
                actions_revision: 0,
                commands: vec![domain::VoiceCommand {
                    phrase: "play my music".into(),
                    action_id,
                }],
            }),
            ..AppConfig::default()
        }
    }

    fn test_state(config: AppConfig, config_path: Option<PathBuf>) -> HttpState {
        HttpState {
            dispatcher: Arc::new(Dispatcher::new(Arc::new(Noop), config.clone()).unwrap()),
            token: ApiToken::parse(&"a".repeat(64)).unwrap(),
            config: Arc::new(std::sync::RwLock::new(config)),
            config_path,
            lifecycle: Arc::new(AsyncMutex::new(())),
        }
    }

    fn paired_headers(body: &[u8]) -> HeaderMap {
        HeaderMap::from_iter([
            (
                header::AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", "a".repeat(64))).unwrap(),
            ),
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            ),
            (
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&body.len().to_string()).unwrap(),
            ),
        ])
    }

    #[test]
    fn framing_requires_bounded_content_length_and_rejects_transfer_encoding() {
        let body = b"{}";
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from_static("2"));
        assert!(valid_framing(&headers, body));
        headers.insert(
            header::TRANSFER_ENCODING,
            HeaderValue::from_static("chunked"),
        );
        assert!(!valid_framing(&headers, body));
        headers.remove(header::TRANSFER_ENCODING);
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from_static("513"));
        assert!(!valid_framing(&headers, body));
    }

    #[tokio::test]
    async fn clock_skew_and_expiry_have_distinct_responses() {
        for (error_value, expected_status, expected_label) in [
            (
                DispatchError::ClockSkew,
                StatusCode::BAD_REQUEST,
                "clock_skew",
            ),
            (DispatchError::Expired, StatusCode::GONE, "expired"),
        ] {
            let response = dispatch_error(error_value);
            assert_eq!(response.status(), expected_status);
            let body = axum::body::to_bytes(response.into_body(), domain::BODY_LIMIT)
                .await
                .unwrap();
            let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(body["status"], expected_label);
        }
    }

    #[tokio::test]
    async fn media_endpoint_authenticates_and_correlates_execution() {
        let state = test_state(AppConfig::default(), None);
        let now = now_ms();
        let body = format!(
            r#"{{"id":"0123456789abcdef0123456789abcdef","cmd":"pause","issued_at_ms":{now},"expires_at_ms":{}}}"#,
            now + domain::REQUEST_LIFETIME_MS
        );
        let body_length = body.len().to_string();
        let response = media(
            State(state.clone()),
            HeaderMap::from_iter([
                (
                    header::AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {}", "a".repeat(64))).unwrap(),
                ),
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                ),
                (
                    header::CONTENT_LENGTH,
                    HeaderValue::from_str(&body_length).unwrap(),
                ),
            ]),
            Bytes::from(body),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key(header::CONTENT_LENGTH));
        assert!(
            response.headers()[header::CONTENT_LENGTH]
                .to_str()
                .unwrap()
                .parse::<usize>()
                .unwrap()
                <= domain::BODY_LIMIT
        );
        let unauthorized = media(State(state), HeaderMap::new(), Bytes::new()).await;
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn voice_config_requires_auth_and_returns_only_published_phrase_ids() {
        let state = test_state(published_config(), None);
        let response = voice_config(State(state.clone()), paired_headers(b"")).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), domain::VOICE_CONFIG_LIMIT)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["commands"][0]["phrase"], "play my music");
        assert_eq!(value["commands"][0]["action_id"], "music");
        assert!(value.get("action").is_none());
        assert_eq!(
            voice_config(State(state), HeaderMap::new()).await.status(),
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn voice_ack_is_authenticated_revision_bound_and_persisted() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.json");
        let state = test_state(published_config(), Some(path.clone()));
        let body = br#"{"revision":1}"#;
        let response = voice_config_ack(
            State(state.clone()),
            paired_headers(body),
            Bytes::from_static(body),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            config::load(&path).unwrap().acknowledged_voice_revision,
            Some(1)
        );
        let stale = br#"{"revision":2}"#;
        assert_eq!(
            voice_config_ack(
                State(state.clone()),
                paired_headers(stale),
                Bytes::from_static(stale)
            )
            .await
            .status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            voice_config_ack(State(state), HeaderMap::new(), Bytes::from_static(body))
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
}
