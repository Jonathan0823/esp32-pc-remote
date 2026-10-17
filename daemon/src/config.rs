use crate::domain::{CustomAction, MAX_VOICE_ACTIONS};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::{self, Write},
    net::IpAddr,
    path::Path,
};
use thiserror::Error;

const CONFIG_VERSION: u32 = 1;
const MAX_ACTIONS: usize = 128;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    pub version: u32,
    pub listener_enabled: bool,
    pub bind_address: IpAddr,
    pub port: u16,
    pub start_after_login: bool,
    pub actions: Vec<CustomAction>,
    #[serde(default)]
    pub actions_revision: u64,
    #[serde(default)]
    pub published_voice: Option<crate::domain::PublishedVoiceConfig>,
    #[serde(default)]
    pub acknowledged_voice_revision: Option<u64>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            listener_enabled: false,
            bind_address: IpAddr::from([127, 0, 0, 1]),
            port: 8443,
            start_after_login: false,
            actions: Vec::new(),
            actions_revision: 0,
            published_voice: None,
            acknowledged_voice_revision: None,
        }
    }
}

impl AppConfig {
    pub fn publish_voice_config(&mut self) -> Result<(), ConfigError> {
        let revision = self
            .published_voice
            .as_ref()
            .map_or(Some(1), |published| published.revision.checked_add(1))
            .ok_or(ConfigError::Invalid("voice revision exhausted"))?;
        let commands: Vec<_> = self
            .actions
            .iter()
            .filter(|action| action.enabled)
            .filter_map(|action| {
                action
                    .phrase
                    .as_ref()
                    .map(|phrase| crate::domain::VoiceCommand {
                        phrase: phrase.clone(),
                        action_id: action.id.clone(),
                    })
            })
            .collect();
        crate::domain::validate_voice_commands(&commands)
            .map_err(|_| ConfigError::Invalid("invalid voice command"))?;
        self.published_voice = Some(crate::domain::PublishedVoiceConfig {
            schema: crate::domain::VOICE_CONFIG_SCHEMA,
            revision,
            actions_revision: self.actions_revision,
            commands,
        });
        self.validate()
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.version != CONFIG_VERSION
            || self.port == 0
            || self.actions.len() > MAX_ACTIONS
            || !matches!(self.bind_address, IpAddr::V4(address) if address.is_private() || address.is_loopback() || address.is_link_local())
        {
            return Err(ConfigError::Invalid("unsupported config version or limits"));
        }
        if self
            .actions
            .iter()
            .filter(|action| action.enabled && action.phrase.is_some())
            .count()
            > MAX_VOICE_ACTIONS
        {
            return Err(ConfigError::Invalid("too many enabled voice commands"));
        }
        let mut ids = HashSet::with_capacity(self.actions.len());
        let voice_commands: Vec<_> = self
            .actions
            .iter()
            .filter_map(|action| {
                action
                    .phrase
                    .as_ref()
                    .map(|phrase| crate::domain::VoiceCommand {
                        phrase: phrase.clone(),
                        action_id: action.id.clone(),
                    })
            })
            .collect();
        crate::domain::validate_voice_command_drafts(&voice_commands)
            .map_err(|_| ConfigError::Invalid("invalid voice command"))?;
        if let Some(published) = &self.published_voice {
            if published.schema != crate::domain::VOICE_CONFIG_SCHEMA
                || published.revision == 0
                || published.actions_revision > self.actions_revision
            {
                return Err(ConfigError::Invalid(
                    "invalid published voice configuration",
                ));
            }
            crate::domain::validate_voice_commands(&published.commands)
                .map_err(|_| ConfigError::Invalid("invalid published voice configuration"))?;
        }
        if self.acknowledged_voice_revision.is_some_and(|revision| {
            revision == 0
                || self
                    .published_voice
                    .as_ref()
                    .is_none_or(|published| revision > published.revision)
        }) {
            return Err(ConfigError::Invalid("invalid voice acknowledgment"));
        }
        for action in &self.actions {
            if action.name.trim().is_empty()
                || action.name.len() > 80
                || !ids.insert(action.id.as_str())
            {
                return Err(ConfigError::Invalid("invalid or duplicate action"));
            }
            match &action.action {
                crate::domain::ConfiguredAction::Media { .. } => {}
                crate::domain::ConfiguredAction::Launch { program, args, .. } => {
                    if !program.is_absolute()
                        || args.len() > 64
                        || args.iter().any(|arg| arg.len() > 1024)
                    {
                        return Err(ConfigError::Invalid("program path must be absolute"));
                    }
                }
                crate::domain::ConfiguredAction::Batch { script, args, .. } => {
                    if args.len() > 64
                        || args.iter().any(|arg| arg.len() > 1024)
                        || !script.is_absolute()
                        || !script
                            .to_str()
                            .is_some_and(crate::domain::is_safe_batch_literal)
                        || !args
                            .iter()
                            .all(|argument| crate::domain::is_safe_batch_literal(argument))
                        || !matches!(
                            script
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(str::to_ascii_lowercase)
                                .as_deref(),
                            Some("bat" | "cmd")
                        )
                    {
                        return Err(ConfigError::Invalid(
                            "batch action needs an absolute .bat or .cmd path",
                        ));
                    }
                }
                crate::domain::ConfiguredAction::PowerShell {
                    interpreter,
                    script,
                    args,
                    ..
                } => {
                    if !interpreter.is_absolute()
                        || !interpreter
                            .file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| {
                                name.eq_ignore_ascii_case("powershell.exe")
                                    || name.eq_ignore_ascii_case("pwsh.exe")
                            })
                        || !script.is_absolute()
                        || args.len() > 64
                        || args.iter().any(|arg| arg.len() > 1024)
                        || !matches!(
                            script
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(str::to_ascii_lowercase)
                                .as_deref(),
                            Some("ps1")
                        )
                    {
                        return Err(ConfigError::Invalid(
                            "PowerShell action needs absolute interpreter and .ps1 paths",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid configuration: {0}")]
    Invalid(&'static str),
    #[error("configuration I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("configuration JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn load(path: &Path) -> Result<AppConfig, ConfigError> {
    let config: AppConfig = serde_json::from_slice(&fs::read(path)?)?;
    config.validate()?;
    Ok(config)
}

/// Validate first, then replace configuration via a flushed sibling temp file.
pub fn save(path: &Path, config: &AppConfig) -> Result<(), ConfigError> {
    config.validate()?;
    let parent = path
        .parent()
        .ok_or(ConfigError::Invalid("config path needs a parent directory"))?;
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut temporary, config)?;
    temporary.write_all(b"\n")?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map_err(|error| ConfigError::Io(error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_action_config_loads_without_voice_fields() {
        let legacy = br#"{"version":1,"listener_enabled":false,"bind_address":"127.0.0.1","port":8443,"start_after_login":false,"actions":[{"id":"music","name":"Music","enabled":true,"action":{"type":"media","command":"play"}}]}"#;
        let config: AppConfig = serde_json::from_slice(legacy).unwrap();
        assert!(config.validate().is_ok());
        assert_eq!(config.actions[0].phrase, None);
        assert_eq!(config.actions_revision, 0);
    }

    #[test]
    fn disabled_phrase_drafts_do_not_count_toward_voice_limit_but_are_validated() {
        let actions = (0..=MAX_VOICE_ACTIONS)
            .map(|index| CustomAction {
                id: crate::domain::ActionId::parse(format!("action_{index}")).unwrap(),
                name: format!("Action {index}"),
                enabled: index < MAX_VOICE_ACTIONS,
                phrase: Some(format!("open app {}", (b'a' + index as u8) as char)),
                action: crate::domain::ConfiguredAction::Media {
                    command: crate::domain::MediaCommand::Play,
                },
            })
            .collect();
        let mut config = AppConfig {
            actions,
            ..AppConfig::default()
        };
        assert!(config.validate().is_ok());

        config.actions[MAX_VOICE_ACTIONS].enabled = true;
        assert!(config.validate().is_err());
        config.actions[MAX_VOICE_ACTIONS].enabled = false;
        config.actions[MAX_VOICE_ACTIONS].phrase = Some("bad  phrase".into());
        assert!(config.validate().is_err());
        config.actions[MAX_VOICE_ACTIONS].phrase = Some(config.actions[0].phrase.clone().unwrap());
        assert!(config.validate().is_err());
    }

    #[test]
    fn publish_captures_only_enabled_phrases_and_preserves_previous_snapshot_until_apply() {
        let id = |value| crate::domain::ActionId::parse(value).unwrap();
        let mut config = AppConfig {
            actions: vec![
                CustomAction {
                    id: id("first"),
                    name: "First".into(),
                    enabled: true,
                    phrase: Some("open first".into()),
                    action: crate::domain::ConfiguredAction::Media {
                        command: crate::domain::MediaCommand::Play,
                    },
                },
                CustomAction {
                    id: id("second"),
                    name: "Second".into(),
                    enabled: false,
                    phrase: Some("open second".into()),
                    action: crate::domain::ConfiguredAction::Media {
                        command: crate::domain::MediaCommand::Pause,
                    },
                },
            ],
            ..AppConfig::default()
        };
        config.validate().unwrap();
        assert!(
            config.published_voice.is_none(),
            "saving drafts must not publish"
        );
        config.publish_voice_config().unwrap();
        let published = config.published_voice.as_ref().unwrap();
        assert_eq!(published.revision, 1);
        assert_eq!(published.commands.len(), 1);
        assert_eq!(published.commands[0].action_id.as_str(), "first");
        config.actions[0].phrase = Some("launch first".into());
        config.actions_revision += 1;
        assert_eq!(config.published_voice.as_ref().unwrap().revision, 1);
        assert_eq!(
            config.published_voice.as_ref().unwrap().commands[0].phrase,
            "open first"
        );
        assert_ne!(
            config.published_voice.as_ref().unwrap().actions_revision,
            config.actions_revision
        );
        config.publish_voice_config().unwrap();
        assert_eq!(config.published_voice.as_ref().unwrap().revision, 2);
        assert_eq!(
            config.published_voice.as_ref().unwrap().commands[0].phrase,
            "launch first"
        );
    }

    #[test]
    fn default_is_disabled_and_loopback_only() {
        let config = AppConfig::default();
        assert!(!config.listener_enabled);
        assert_eq!(config.bind_address.to_string(), "127.0.0.1");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn wildcard_public_and_ipv6_bind_addresses_are_rejected() {
        for address in ["0.0.0.0", "::1", "8.8.8.8"] {
            let config = AppConfig {
                bind_address: address.parse().unwrap(),
                ..AppConfig::default()
            };
            assert!(config.validate().is_err());
        }
        let private_address = AppConfig {
            bind_address: "192.168.1.50".parse().unwrap(),
            ..AppConfig::default()
        };
        assert!(private_address.validate().is_ok());
    }

    #[test]
    fn invalid_configuration_is_not_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let invalid = AppConfig {
            port: 0,
            ..AppConfig::default()
        };
        assert!(save(&path, &invalid).is_err());
        assert!(!path.exists());
    }
}
