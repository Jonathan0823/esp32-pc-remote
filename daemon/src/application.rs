use crate::{
    config::AppConfig,
    domain::{
        self, ActionId, ConfiguredAction, CustomActionRequest, MediaCommand, MediaRequest,
        RequestId,
    },
};
use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, RwLock},
};
use subtle::{Choice, ConstantTimeEq};
use thiserror::Error;
use tokio::sync::RwLock as AsyncRwLock;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub trait ActionExecutor: Send + Sync + 'static {
    fn execute<'a>(
        &'a self,
        action: &'a ConfiguredAction,
    ) -> Pin<Box<dyn Future<Output = Result<(), ExecutorError>> + Send + 'a>>;
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ExecutorError {
    #[error("action dispatch failed")]
    Failed,
    #[error("action executor is busy")]
    Busy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticEvent {
    pub request_id: Option<String>,
    pub action_id: Option<String>,
    pub category: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchStatus {
    Executed,
    Dispatched,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum DispatchError {
    #[error("request time is invalid or expired")]
    InvalidTime,
    #[error("request id was already used")]
    Duplicate,
    #[error("request cache is full")]
    Busy,
    #[error("custom action was not found")]
    UnknownAction,
    #[error("custom action is disabled")]
    DisabledAction,
    #[error("voice configuration is stale; sync the latest settings")]
    StaleVoiceConfig,
    #[error("action dispatch failed")]
    Execution,
    #[error("action executor is busy")]
    ExecutionBusy,
    #[error("configuration is invalid")]
    Configuration,
}

#[derive(Clone, Debug)]
struct Reservation {
    id: RequestId,
    expires_at_ms: u64,
}

#[derive(Default)]
struct DuplicateCache {
    entries: VecDeque<Reservation>,
}

impl DuplicateCache {
    fn reserve(
        &mut self,
        id: RequestId,
        expires_at_ms: u64,
        now_ms: u64,
    ) -> Result<(), DispatchError> {
        self.entries.retain(|entry| entry.expires_at_ms > now_ms);
        if self.entries.iter().any(|entry| entry.id == id) {
            return Err(DispatchError::Duplicate);
        }
        if self.entries.len() >= domain::MAX_REMEMBERED_IDS {
            return Err(DispatchError::Busy);
        }
        self.entries.push_back(Reservation { id, expires_at_ms });
        Ok(())
    }
}

pub struct Dispatcher {
    executor: Arc<dyn ActionExecutor>,
    config: Arc<RwLock<AppConfig>>,
    seen: Mutex<DuplicateCache>,
    diagnostics: Mutex<VecDeque<DiagnosticEvent>>,
    lifecycle: AsyncRwLock<()>,
}

impl Dispatcher {
    pub fn new(
        executor: Arc<dyn ActionExecutor>,
        config: AppConfig,
    ) -> Result<Self, DispatchError> {
        config
            .validate()
            .map_err(|_| DispatchError::Configuration)?;
        Ok(Self {
            executor,
            config: Arc::new(RwLock::new(config)),
            seen: Mutex::new(DuplicateCache::default()),
            diagnostics: Mutex::new(VecDeque::with_capacity(128)),
            lifecycle: AsyncRwLock::new(()),
        })
    }

    pub async fn replace_config(&self, config: AppConfig) -> Result<(), DispatchError> {
        config
            .validate()
            .map_err(|_| DispatchError::Configuration)?;
        let _lifecycle = self.lifecycle.write().await;
        *self
            .config
            .write()
            .map_err(|_| DispatchError::Configuration)? = config;
        Ok(())
    }

    pub async fn dispatch_media(
        &self,
        request: MediaRequest,
        now_ms: u64,
    ) -> Result<DispatchStatus, DispatchError> {
        validate_request_time(request.issued_at_ms, request.expires_at_ms, now_ms)?;
        let _lifecycle = self.lifecycle.try_read().map_err(|_| DispatchError::Busy)?;
        let request_id = request.id.clone();
        self.reserve(request.id, request.expires_at_ms, now_ms)?;
        let result = self
            .execute(&ConfiguredAction::Media {
                command: request.cmd,
            })
            .await;
        self.record(
            Some(request_id.as_str()),
            None,
            if result.is_ok() {
                "media_executed"
            } else {
                "media_failed"
            },
        );
        result?;
        Ok(DispatchStatus::Executed)
    }

    pub async fn test_action(&self, action_id: &domain::ActionId) -> Result<(), DispatchError> {
        let _lifecycle = self.lifecycle.try_read().map_err(|_| DispatchError::Busy)?;
        let action = {
            let config = self
                .config
                .read()
                .map_err(|_| DispatchError::Configuration)?;
            let action = config
                .actions
                .iter()
                .find(|action| &action.id == action_id)
                .ok_or(DispatchError::UnknownAction)?;
            if !action.enabled {
                return Err(DispatchError::DisabledAction);
            }
            action.action.clone()
        };
        let result = self.execute(&action).await;
        self.record(
            None,
            Some(action_id.as_str()),
            if result.is_ok() {
                "local_test_dispatched"
            } else {
                "local_test_failed"
            },
        );
        result
    }

    pub async fn dispatch_custom(
        &self,
        request: CustomActionRequest,
        now_ms: u64,
    ) -> Result<DispatchStatus, DispatchError> {
        validate_request_time(request.issued_at_ms, request.expires_at_ms, now_ms)?;
        let _lifecycle = self.lifecycle.try_read().map_err(|_| DispatchError::Busy)?;
        let action = {
            let config = self
                .config
                .read()
                .map_err(|_| DispatchError::Configuration)?;
            let published = config
                .published_voice
                .as_ref()
                .ok_or(DispatchError::StaleVoiceConfig)?;
            if published.revision != request.voice_revision
                || published.actions_revision != config.actions_revision
                || !published
                    .commands
                    .iter()
                    .any(|command| command.action_id == request.action_id)
            {
                return Err(DispatchError::StaleVoiceConfig);
            }
            let action = config
                .actions
                .iter()
                .find(|action| action.id == request.action_id)
                .ok_or(DispatchError::UnknownAction)?;
            if !action.enabled {
                return Err(DispatchError::DisabledAction);
            }
            action.action.clone()
        };
        let request_id = request.id.clone();
        self.reserve(request.id, request.expires_at_ms, now_ms)?;
        let result = self.execute(&action).await;
        self.record(
            Some(request_id.as_str()),
            Some(request.action_id.as_str()),
            if result.is_ok() {
                "action_dispatched"
            } else {
                "action_failed"
            },
        );
        result?;
        Ok(DispatchStatus::Dispatched)
    }

    pub fn diagnostics(&self) -> Vec<DiagnosticEvent> {
        self.diagnostics
            .lock()
            .map(|events| events.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn record(&self, request_id: Option<&str>, action_id: Option<&str>, category: &'static str) {
        if let Ok(mut events) = self.diagnostics.lock() {
            if events.len() == 128 {
                events.pop_front();
            }
            events.push_back(DiagnosticEvent {
                request_id: request_id.map(str::to_owned),
                action_id: action_id.map(str::to_owned),
                category,
            });
        }
    }

    fn reserve(&self, id: RequestId, expiry: u64, now: u64) -> Result<(), DispatchError> {
        self.seen
            .lock()
            .map_err(|_| DispatchError::Configuration)?
            .reserve(id, expiry, now)
    }

    async fn execute(&self, action: &ConfiguredAction) -> Result<(), DispatchError> {
        self.executor
            .execute(action)
            .await
            .map_err(|error| match error {
                ExecutorError::Failed => DispatchError::Execution,
                ExecutorError::Busy => DispatchError::ExecutionBusy,
            })
    }
}

fn validate_request_time(issued: u64, expires: u64, now: u64) -> Result<(), DispatchError> {
    domain::validate_time(issued, expires, now).map_err(|_| DispatchError::InvalidTime)
}

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct ApiToken([u8; 64]);

impl Clone for ApiToken {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl ApiToken {
    pub fn generate() -> Self {
        let random: [u8; 32] = rand::random();
        let mut token = [0u8; 64];
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for (index, byte) in random.iter().enumerate() {
            token[index * 2] = HEX[(byte >> 4) as usize];
            token[index * 2 + 1] = HEX[(byte & 0x0f) as usize];
        }
        Self(token)
    }

    pub fn parse(value: &str) -> Option<Self> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return None;
        }
        let mut token = [0; 64];
        token.copy_from_slice(value.as_bytes());
        Some(Self(token))
    }

    pub fn expose_for_pairing(&self) -> String {
        String::from_utf8(self.0.to_vec()).expect("generated lowercase hex")
    }

    pub fn verify_header(&self, header: Option<&str>) -> bool {
        let supplied = header
            .and_then(|value| value.strip_prefix("Bearer "))
            .unwrap_or("")
            .as_bytes();
        let mut padded = [0u8; 64];
        let copied = supplied.len().min(padded.len());
        padded[..copied].copy_from_slice(&supplied[..copied]);
        let length_matches = Choice::from(u8::from(supplied.len() == self.0.len()));
        bool::from(length_matches & padded.ct_eq(&self.0))
    }
}

impl std::fmt::Debug for ApiToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApiToken([REDACTED])")
    }
}

pub fn media_action(command: MediaCommand) -> ConfiguredAction {
    ConfiguredAction::Media { command }
}
pub fn find_action_id(value: &str) -> Option<ActionId> {
    ActionId::parse(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ActionId, CustomAction};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct FakeExecutor(AtomicUsize);
    impl ActionExecutor for FakeExecutor {
        fn execute<'a>(
            &'a self,
            _action: &'a ConfiguredAction,
        ) -> Pin<Box<dyn Future<Output = Result<(), ExecutorError>> + Send + 'a>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }
    }

    struct BlockingExecutor {
        started: tokio::sync::Notify,
        release: tokio::sync::Semaphore,
    }

    impl ActionExecutor for BlockingExecutor {
        fn execute<'a>(
            &'a self,
            _action: &'a ConfiguredAction,
        ) -> Pin<Box<dyn Future<Output = Result<(), ExecutorError>> + Send + 'a>> {
            Box::pin(async move {
                self.started.notify_one();
                self.release
                    .acquire()
                    .await
                    .map_err(|_| ExecutorError::Failed)?
                    .forget();
                Ok(())
            })
        }
    }

    fn request(id: &str) -> MediaRequest {
        MediaRequest {
            id: RequestId::parse(id).unwrap(),
            cmd: MediaCommand::Pause,
            issued_at_ms: 1000,
            expires_at_ms: 6000,
        }
    }

    #[tokio::test]
    async fn request_executes_once_and_duplicate_does_not_dispatch_twice() {
        let executor = Arc::new(FakeExecutor::default());
        let dispatcher = Dispatcher::new(executor.clone(), AppConfig::default()).unwrap();
        assert_eq!(
            dispatcher
                .dispatch_media(request("00000000000000000000000000000001"), 1000)
                .await,
            Ok(DispatchStatus::Executed)
        );
        assert_eq!(
            dispatcher
                .dispatch_media(request("00000000000000000000000000000001"), 1001)
                .await,
            Err(DispatchError::Duplicate)
        );
        assert_eq!(executor.0.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn diagnostics_ring_keeps_only_the_newest_128_events() {
        let dispatcher =
            Dispatcher::new(Arc::new(FakeExecutor::default()), AppConfig::default()).unwrap();
        for index in 0..129 {
            dispatcher.record(Some("request"), None, "media_executed");
            if index == 0 {
                dispatcher.record(Some("oldest"), None, "media_executed");
            }
        }
        let events = dispatcher.diagnostics();
        assert_eq!(events.len(), 128);
        assert_eq!(events[0].request_id.as_deref(), Some("request"));
    }

    #[tokio::test]
    async fn config_apply_waits_for_dispatch_and_rejects_new_work_during_apply() {
        let executor = Arc::new(BlockingExecutor {
            started: tokio::sync::Notify::new(),
            release: tokio::sync::Semaphore::new(0),
        });
        let dispatcher = Arc::new(Dispatcher::new(executor.clone(), AppConfig::default()).unwrap());
        let dispatch = {
            let dispatcher = Arc::clone(&dispatcher);
            tokio::spawn(async move {
                dispatcher
                    .dispatch_media(request("00000000000000000000000000000001"), 1000)
                    .await
            })
        };
        executor.started.notified().await;
        let update = {
            let dispatcher = Arc::clone(&dispatcher);
            tokio::spawn(async move { dispatcher.replace_config(AppConfig::default()).await })
        };
        tokio::task::yield_now().await;
        assert!(matches!(
            dispatcher
                .dispatch_media(request("00000000000000000000000000000002"), 1000)
                .await,
            Err(DispatchError::Busy)
        ));
        executor.release.add_permits(1);
        assert_eq!(dispatch.await.unwrap(), Ok(DispatchStatus::Executed));
        assert!(update.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn cache_rejects_saturation_instead_of_evicting_live_ids() {
        let mut cache = DuplicateCache::default();
        for n in 0..domain::MAX_REMEMBERED_IDS {
            let id = format!("{n:032x}");
            cache
                .reserve(RequestId::parse(id).unwrap(), 6000, 1000)
                .unwrap();
        }
        assert_eq!(
            cache.reserve(
                RequestId::parse("ffffffffffffffffffffffffffffffff").unwrap(),
                6000,
                1000
            ),
            Err(DispatchError::Busy)
        );
        assert_eq!(cache.entries.len(), domain::MAX_REMEMBERED_IDS);
    }

    #[tokio::test]
    async fn custom_action_uses_local_snapshot_only() {
        let action_id = ActionId::parse("music").unwrap();
        let config = AppConfig {
            actions: vec![CustomAction {
                id: action_id.clone(),
                name: "Music".into(),
                enabled: true,
                phrase: None,
                action: ConfiguredAction::Media {
                    command: MediaCommand::Play,
                },
            }],
            published_voice: Some(domain::PublishedVoiceConfig {
                schema: domain::VOICE_CONFIG_SCHEMA,
                revision: 1,
                actions_revision: 0,
                commands: vec![domain::VoiceCommand {
                    phrase: "play music".into(),
                    action_id: action_id.clone(),
                }],
            }),
            ..AppConfig::default()
        };
        let dispatcher = Dispatcher::new(Arc::new(FakeExecutor::default()), config).unwrap();
        let req = CustomActionRequest {
            id: RequestId::parse("00000000000000000000000000000002").unwrap(),
            action_id,
            voice_revision: 1,
            issued_at_ms: 1000,
            expires_at_ms: 6000,
        };
        assert_eq!(
            dispatcher.dispatch_custom(req, 1000).await,
            Ok(DispatchStatus::Dispatched)
        );
    }

    #[tokio::test]
    async fn custom_action_fails_closed_for_unpublished_action_changes() {
        let action_id = ActionId::parse("music").unwrap();
        let config = AppConfig {
            actions: vec![CustomAction {
                id: action_id.clone(),
                name: "Music".into(),
                enabled: true,
                phrase: Some("play music".into()),
                action: ConfiguredAction::Media {
                    command: MediaCommand::Play,
                },
            }],
            actions_revision: 2,
            published_voice: Some(domain::PublishedVoiceConfig {
                schema: domain::VOICE_CONFIG_SCHEMA,
                revision: 1,
                actions_revision: 1,
                commands: vec![domain::VoiceCommand {
                    phrase: "play music".into(),
                    action_id: action_id.clone(),
                }],
            }),
            ..AppConfig::default()
        };
        let dispatcher = Dispatcher::new(Arc::new(FakeExecutor::default()), config).unwrap();
        let request = CustomActionRequest {
            id: RequestId::parse("00000000000000000000000000000003").unwrap(),
            action_id,
            voice_revision: 1,
            issued_at_ms: 1000,
            expires_at_ms: 6000,
        };
        assert_eq!(
            dispatcher.dispatch_custom(request, 1000).await,
            Err(DispatchError::StaleVoiceConfig)
        );
    }

    #[test]
    fn token_header_authentication_is_fixed_width_and_redacted() {
        let token = ApiToken::parse(&"a".repeat(64)).unwrap();
        assert!(token.verify_header(Some(&format!("Bearer {}", "a".repeat(64)))));
        assert!(!token.verify_header(Some("Bearer short")));
        assert!(!format!("{token:?}").contains(&"a".repeat(16)));
    }
}
