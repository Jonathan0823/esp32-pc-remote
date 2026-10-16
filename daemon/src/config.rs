use crate::domain::CustomAction;
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
        }
    }
}

impl AppConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.version != CONFIG_VERSION
            || self.port == 0
            || self.actions.len() > MAX_ACTIONS
            || !matches!(self.bind_address, IpAddr::V4(address) if address.is_private() || address.is_loopback() || address.is_link_local())
        {
            return Err(ConfigError::Invalid("unsupported config version or limits"));
        }
        let mut ids = HashSet::with_capacity(self.actions.len());
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
