use crate::{
    adapters::secret_store,
    application::{ApiToken, Dispatcher},
    config::{self, AppConfig},
    http::{HttpState, ListenerTask},
    security,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
    ptr,
    sync::{Arc, RwLock},
};
use tempfile::NamedTempFile;
use tokio::sync::Mutex;
use windows::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, GetLastError,
            HANDLE,
        },
        System::{
            Registry::{
                HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
                RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegSetValueExW,
            },
            Threading::CreateMutexW,
        },
    },
    core::PCWSTR,
};
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
struct Secrets {
    token: String,
    host: String,
    certificate_pem: Vec<u8>,
    server_key_pem: Vec<u8>,
    ca_certificate_pem: Vec<u8>,
    ca_key_pem: Vec<u8>,
}

pub struct WindowsRuntime {
    directory: PathBuf,
    dispatcher: Arc<Dispatcher>,
    config: Arc<RwLock<AppConfig>>,
    secrets: RwLock<Secrets>,
    listener: Mutex<Option<ListenerTask>>,
    lifecycle: Arc<Mutex<()>>,
}

pub struct SingleInstance(HANDLE);

impl Drop for SingleInstance {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) }.ok();
    }
}

pub fn acquire_single_instance() -> Result<Option<SingleInstance>, RuntimeError> {
    let name: Vec<u16> = "Local\\Esp32MediaDaemon.Instance\0"
        .encode_utf16()
        .collect();
    let handle = unsafe { CreateMutexW(None, false, PCWSTR::from_raw(name.as_ptr())) }
        .map_err(|_| RuntimeError)?;
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe { CloseHandle(handle) }.ok();
        super::windows_host::activate_existing_instance();
        return Ok(None);
    }
    Ok(Some(SingleInstance(handle)))
}

impl WindowsRuntime {
    pub fn load() -> Result<Arc<Self>, RuntimeError> {
        let directory = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or(RuntimeError)?
            .join("Esp32MediaDaemon");
        fs::create_dir_all(&directory).map_err(|_| RuntimeError)?;
        let config_path = directory.join("config.json");
        let config_exists = config_path.exists();
        let mut app_config = match config::load(&config_path) {
            Ok(config) => config,
            Err(config::ConfigError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                AppConfig::default()
            }
            Err(_) => AppConfig::default(),
        };
        if app_config.validate().is_err() {
            app_config = AppConfig::default();
        }
        let secrets_path = directory.join("secrets.dpapi");
        let mut secrets = if secrets_path.exists() {
            match read_secrets(&secrets_path) {
                Ok(secrets) => secrets,
                Err(_) => new_secrets(&app_config)?,
            }
        } else {
            new_secrets(&app_config)?
        };
        if ApiToken::parse(&secrets.token).is_none() {
            secrets = new_secrets(&app_config)?;
        } else if secrets.host != app_config.bind_address.to_string() {
            rotate_tls(&mut secrets, app_config.bind_address)?;
        }
        let dispatcher = Arc::new(
            Dispatcher::new(
                Arc::new(super::windows_executor::WindowsExecutor::default()),
                app_config.clone(),
            )
            .map_err(|_| RuntimeError)?,
        );
        let runtime = Arc::new(Self {
            directory,
            dispatcher,
            config: Arc::new(RwLock::new(app_config.clone())),
            secrets: RwLock::new(secrets),
            listener: Mutex::new(None),
            lifecycle: Arc::new(Mutex::new(())),
        });
        if !config_exists {
            runtime.save_config(&app_config)?;
        }
        runtime.save_secrets()?;
        Ok(runtime)
    }

    pub fn diagnostics(&self) -> Vec<crate::application::DiagnosticEvent> {
        self.dispatcher.diagnostics()
    }

    pub fn config(&self) -> AppConfig {
        self.config
            .read()
            .map(|config| config.clone())
            .unwrap_or_default()
    }

    pub fn token_for_pairing(&self) -> Result<String, RuntimeError> {
        self.secrets
            .read()
            .map(|secrets| secrets.token.clone())
            .map_err(|_| RuntimeError)
    }

    pub async fn test_action(
        &self,
        action_id: &crate::domain::ActionId,
    ) -> Result<(), RuntimeError> {
        self.dispatcher
            .test_action(action_id)
            .await
            .map_err(|_| RuntimeError)
    }

    pub async fn save_actions(
        &self,
        actions: Vec<crate::domain::CustomAction>,
    ) -> Result<(), RuntimeError> {
        let _lifecycle = self.lifecycle.lock().await;
        let mut candidate = self.config();
        candidate.actions = actions;
        candidate.actions_revision = candidate
            .actions_revision
            .checked_add(1)
            .ok_or(RuntimeError)?;
        candidate.validate().map_err(|_| RuntimeError)?;
        self.save_config(&candidate)?;
        self.dispatcher
            .replace_config(candidate.clone())
            .await
            .map_err(|_| RuntimeError)?;
        *self.config.write().map_err(|_| RuntimeError)? = candidate;
        Ok(())
    }

    pub async fn apply_voice_config(&self) -> Result<(), RuntimeError> {
        let _lifecycle = self.lifecycle.lock().await;
        let mut candidate = self.config();
        candidate.publish_voice_config().map_err(|_| RuntimeError)?;
        self.save_config(&candidate)?;
        self.dispatcher
            .replace_config(candidate.clone())
            .await
            .map_err(|_| RuntimeError)?;
        *self.config.write().map_err(|_| RuntimeError)? = candidate;
        Ok(())
    }

    pub async fn set_start_after_login(&self, enabled: bool) -> Result<(), RuntimeError> {
        let _lifecycle = self.lifecycle.lock().await;
        let mut candidate = self.config();
        let previous = candidate.start_after_login;
        candidate.start_after_login = enabled;
        configure_startup(enabled)?;
        if let Err(error) = self.save_config(&candidate) {
            let _ = configure_startup(previous);
            return Err(error);
        }
        self.dispatcher
            .replace_config(candidate.clone())
            .await
            .map_err(|_| RuntimeError)?;
        *self.config.write().map_err(|_| RuntimeError)? = candidate;
        Ok(())
    }

    pub fn export_ca(&self) -> Result<PathBuf, RuntimeError> {
        let path = self.directory.join("trusted-ca.pem");
        let pem = self
            .secrets
            .read()
            .map_err(|_| RuntimeError)?
            .ca_certificate_pem
            .clone();
        atomic_write(&path, &pem)?;
        Ok(path)
    }

    pub async fn rotate_credentials(&self) -> Result<(), RuntimeError> {
        let _lifecycle = self.lifecycle.lock().await;
        let config = self.config();
        let previous_listener = self.listener.lock().await.take();
        let was_running = previous_listener.is_some();
        if let Some(listener) = previous_listener {
            listener.stop().await.map_err(|_| RuntimeError)?;
        }
        let replacement = match new_secrets(&config) {
            Ok(replacement) => replacement,
            Err(_) => {
                if was_running {
                    *self.listener.lock().await = Some(self.start_listener(&config).await?);
                }
                return Err(RuntimeError);
            }
        };
        let previous = {
            let mut secrets = self.secrets.write().map_err(|_| RuntimeError)?;
            std::mem::replace(&mut *secrets, replacement)
        };
        if self.save_secrets().is_err() {
            *self.secrets.write().map_err(|_| RuntimeError)? = previous;
            if was_running {
                *self.listener.lock().await = Some(self.start_listener(&config).await?);
            }
            return Err(RuntimeError);
        }
        if was_running {
            match self.start_listener(&config).await {
                Ok(listener) => *self.listener.lock().await = Some(listener),
                Err(_) => {
                    let mut paused = config.clone();
                    paused.listener_enabled = false;
                    self.save_config(&paused)?;
                    self.dispatcher
                        .replace_config(paused.clone())
                        .await
                        .map_err(|_| RuntimeError)?;
                    *self.config.write().map_err(|_| RuntimeError)? = paused;
                    return Err(RuntimeError);
                }
            }
        }
        Ok(())
    }

    async fn start_listener(&self, config: &AppConfig) -> Result<ListenerTask, RuntimeError> {
        let (certificate, key, token) = {
            let secrets = self.secrets.read().map_err(|_| RuntimeError)?;
            (
                secrets.certificate_pem.clone(),
                secrets.server_key_pem.clone(),
                ApiToken::parse(&secrets.token).ok_or(RuntimeError)?,
            )
        };
        ListenerTask::start(
            SocketAddr::new(config.bind_address, config.port),
            certificate,
            key,
            HttpState {
                dispatcher: Arc::clone(&self.dispatcher),
                token,
                config: Arc::clone(&self.config),
                config_path: Some(self.directory.join("config.json")),
                lifecycle: Arc::clone(&self.lifecycle),
            },
        )
        .await
        .map_err(|_| RuntimeError)
    }

    pub async fn shutdown(&self) -> Result<(), RuntimeError> {
        if let Some(listener) = self.listener.lock().await.take() {
            listener.stop().await.map_err(|_| RuntimeError)?;
        }
        Ok(())
    }

    pub async fn set_listener(&self, enabled: bool) -> Result<(), RuntimeError> {
        let _lifecycle = self.lifecycle.lock().await;
        let current = self.config();
        let is_running = self.listener.lock().await.is_some();
        if current.listener_enabled == enabled && is_running == enabled {
            return Ok(());
        }
        let mut candidate = current;
        candidate.listener_enabled = enabled;
        if enabled {
            let listener = self.start_listener(&candidate).await;
            let listener = match listener {
                Ok(listener) => listener,
                Err(_) => {
                    candidate.listener_enabled = false;
                    self.save_config(&candidate)?;
                    self.dispatcher
                        .replace_config(candidate.clone())
                        .await
                        .map_err(|_| RuntimeError)?;
                    *self.config.write().map_err(|_| RuntimeError)? = candidate;
                    return Err(RuntimeError);
                }
            };
            if let Err(error) = self.save_config(&candidate) {
                let _ = listener.stop().await;
                return Err(error);
            }
            *self.listener.lock().await = Some(listener);
        } else {
            if let Some(listener) = self.listener.lock().await.take() {
                listener.stop().await.map_err(|_| RuntimeError)?;
            }
            self.save_config(&candidate)?;
        }
        self.dispatcher
            .replace_config(candidate.clone())
            .await
            .map_err(|_| RuntimeError)?;
        *self.config.write().map_err(|_| RuntimeError)? = candidate;
        Ok(())
    }

    pub async fn reconfigure(
        &self,
        bind_address: std::net::IpAddr,
        port: u16,
    ) -> Result<(), RuntimeError> {
        let _lifecycle = self.lifecycle.lock().await;
        if self
            .listener
            .try_lock()
            .map_err(|_| RuntimeError)?
            .is_some()
        {
            return Err(RuntimeError);
        }
        let mut candidate = self.config();
        candidate.bind_address = bind_address;
        candidate.port = port;
        candidate.validate().map_err(|_| RuntimeError)?;
        let previous = self.secrets.read().map_err(|_| RuntimeError)?.clone();
        {
            let mut secrets = self.secrets.write().map_err(|_| RuntimeError)?;
            if secrets.host != bind_address.to_string() {
                rotate_tls(&mut secrets, bind_address)?;
            }
        }
        if let Err(error) = self.save_secrets() {
            *self.secrets.write().map_err(|_| RuntimeError)? = previous;
            return Err(error);
        }
        if let Err(error) = self.save_config(&candidate) {
            *self.secrets.write().map_err(|_| RuntimeError)? = previous;
            let _ = self.save_secrets();
            return Err(error);
        }
        self.dispatcher
            .replace_config(candidate.clone())
            .await
            .map_err(|_| RuntimeError)?;
        *self.config.write().map_err(|_| RuntimeError)? = candidate;
        Ok(())
    }

    fn save_config(&self, config: &AppConfig) -> Result<(), RuntimeError> {
        config::save(&self.directory.join("config.json"), config).map_err(|_| RuntimeError)
    }

    fn save_secrets(&self) -> Result<(), RuntimeError> {
        let secrets = self.secrets.read().map_err(|_| RuntimeError)?;
        let mut plaintext = serde_json::to_vec(&*secrets).map_err(|_| RuntimeError)?;
        let encrypted = secret_store::protect(&plaintext);
        plaintext.zeroize();
        atomic_write(
            &self.directory.join("secrets.dpapi"),
            &encrypted.map_err(|_| RuntimeError)?,
        )
    }
}

fn new_secrets(config: &AppConfig) -> Result<Secrets, RuntimeError> {
    let tls = security::provision_tls(config.bind_address).map_err(|_| RuntimeError)?;
    Ok(Secrets {
        token: ApiToken::generate().expose_for_pairing(),
        host: config.bind_address.to_string(),
        certificate_pem: tls.certificate_pem,
        server_key_pem: tls.server_private_key_pem,
        ca_certificate_pem: tls.trusted_ca_certificate_pem,
        ca_key_pem: tls.ca_private_key_pem,
    })
}

fn rotate_tls(secrets: &mut Secrets, host: std::net::IpAddr) -> Result<(), RuntimeError> {
    let tls = security::provision_tls(host).map_err(|_| RuntimeError)?;
    secrets.host = host.to_string();
    secrets.certificate_pem = tls.certificate_pem;
    secrets.server_key_pem = tls.server_private_key_pem;
    secrets.ca_certificate_pem = tls.trusted_ca_certificate_pem;
    secrets.ca_key_pem = tls.ca_private_key_pem;
    Ok(())
}

fn read_secrets(path: &Path) -> Result<Secrets, RuntimeError> {
    let encrypted = fs::read(path).map_err(|_| RuntimeError)?;
    let mut plaintext = secret_store::unprotect(&encrypted).map_err(|_| RuntimeError)?;
    let secrets = serde_json::from_slice(&plaintext).map_err(|_| RuntimeError);
    plaintext.zeroize();
    secrets
}

fn configure_startup(enabled: bool) -> Result<(), RuntimeError> {
    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run\0"
        .encode_utf16()
        .collect();
    let value_name: Vec<u16> = "Esp32MediaDaemon\0".encode_utf16().collect();
    let mut key = HKEY(ptr::null_mut());
    let result = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(subkey.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
    };
    if result != ERROR_SUCCESS {
        return Err(RuntimeError);
    }
    let result = if enabled {
        std::env::current_exe()
            .map(|executable| {
                let value: Vec<u16> = format!("\"{}\"", executable.display())
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                let bytes = unsafe {
                    std::slice::from_raw_parts(value.as_ptr().cast::<u8>(), value.len() * 2)
                };
                unsafe {
                    RegSetValueExW(
                        key,
                        PCWSTR::from_raw(value_name.as_ptr()),
                        None,
                        REG_SZ,
                        Some(bytes),
                    )
                }
            })
            .unwrap_or(windows::Win32::Foundation::WIN32_ERROR(u32::MAX))
    } else {
        unsafe { RegDeleteValueW(key, PCWSTR::from_raw(value_name.as_ptr())) }
    };
    if unsafe { RegCloseKey(key) } != ERROR_SUCCESS {
        return Err(RuntimeError);
    }
    if result == ERROR_SUCCESS || (!enabled && result == ERROR_FILE_NOT_FOUND) {
        Ok(())
    } else {
        Err(RuntimeError)
    }
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), RuntimeError> {
    let mut temporary =
        NamedTempFile::new_in(path.parent().ok_or(RuntimeError)?).map_err(|_| RuntimeError)?;
    temporary.write_all(contents).map_err(|_| RuntimeError)?;
    temporary.as_file().sync_all().map_err(|_| RuntimeError)?;
    temporary.persist(path).map_err(|_| RuntimeError)?;
    Ok(())
}

impl Drop for WindowsRuntime {
    fn drop(&mut self) {
        if let Ok(listener) = self.listener.try_lock()
            && let Some(listener) = listener.as_ref()
        {
            listener.request_shutdown();
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("daemon configuration or listener operation failed")]
pub struct RuntimeError;
