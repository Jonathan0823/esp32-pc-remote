use crate::{
    application::{ApiToken, DispatchError, DispatchStatus, Dispatcher},
    domain::{self, CustomActionRequest, MediaRequest},
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use axum_server::{accept::Accept, tls_rustls::RustlsAcceptor};
use serde::Serialize;
use std::{
    future::{Ready, ready},
    io,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
    sync::{OwnedSemaphorePermit, Semaphore},
};
use tower_http::timeout::TimeoutLayer;

#[derive(Clone)]
pub struct HttpState {
    pub dispatcher: Arc<Dispatcher>,
    pub token: ApiToken,
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
        DispatchError::InvalidTime => (StatusCode::GONE, "expired"),
        DispatchError::Duplicate => (StatusCode::CONFLICT, "duplicate"),
        DispatchError::Busy | DispatchError::ExecutionBusy => {
            (StatusCode::TOO_MANY_REQUESTS, "busy")
        }
        DispatchError::UnknownAction => (StatusCode::NOT_FOUND, "unknown_action"),
        DispatchError::DisabledAction => (StatusCode::FORBIDDEN, "disabled_action"),
        DispatchError::Execution | DispatchError::Configuration => {
            (StatusCode::INTERNAL_SERVER_ERROR, "server_error")
        }
    };
    error(status, label)
}

fn json<T: Serialize>(status: StatusCode, value: &T) -> Response {
    let body = match serde_json::to_vec(value) {
        Ok(body) if body.len() <= domain::BODY_LIMIT => body,
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
    let acceptor = RustlsAcceptor::new(tls)
        .handshake_timeout(Duration::from_secs(2))
        .acceptor(LimitedAcceptor::new(8));
    axum_server::Server::from_tcp(listener)
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
    async fn media_endpoint_authenticates_and_correlates_execution() {
        let token = ApiToken::parse(&"a".repeat(64)).unwrap();
        let state = HttpState {
            dispatcher: Arc::new(Dispatcher::new(Arc::new(Noop), AppConfig::default()).unwrap()),
            token,
        };
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
}
