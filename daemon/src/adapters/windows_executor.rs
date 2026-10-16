use crate::{
    application::{ActionExecutor, ExecutorError},
    domain::{self, ConfiguredAction, MediaCommand},
};
use process_wrap::tokio::{CommandWrap, JobObject, KillOnDrop};
use std::{future::Future, path::Path, pin::Pin, process::Stdio, sync::Arc, time::Duration};
use tokio::sync::Semaphore;
#[cfg(test)]
use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;
use windows::Win32::{
    Media::Audio::{
        Endpoints::IAudioEndpointVolume, MMDeviceEnumerator, eMultimedia as E_MULTIMEDIA,
        eRender as E_RENDER,
    },
    System::Com::{
        CLSCTX_ALL, CO_MTA_USAGE_COOKIE, CoCreateInstance, CoDecrementMTAUsage, CoIncrementMTAUsage,
    },
    System::{
        Threading::CREATE_NO_WINDOW,
        WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
    },
    UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
        VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK,
    },
};
#[cfg(test)]
use windows_future::{AsyncStatus, IAsyncOperation};

const SCRIPT_TIMEOUT: Duration = Duration::from_secs(60);
#[cfg(test)]
const MEDIA_OPERATION_TIMEOUT: Duration = Duration::from_secs(1);

pub(super) struct MtaLifetime(CO_MTA_USAGE_COOKIE);

impl MtaLifetime {
    pub(super) fn new() -> windows::core::Result<Self> {
        unsafe { CoIncrementMTAUsage() }.map(Self)
    }
}

impl Drop for MtaLifetime {
    fn drop(&mut self) {
        let _ = unsafe { CoDecrementMTAUsage(self.0) };
    }
}

pub struct WindowsExecutor {
    running_scripts: Arc<Semaphore>,
    native_action: Arc<Semaphore>,
}

impl Default for WindowsExecutor {
    fn default() -> Self {
        Self {
            running_scripts: Arc::new(Semaphore::new(1)),
            native_action: Arc::new(Semaphore::new(1)),
        }
    }
}

impl ActionExecutor for WindowsExecutor {
    fn execute<'a>(
        &'a self,
        action: &'a ConfiguredAction,
    ) -> Pin<Box<dyn Future<Output = Result<(), ExecutorError>> + Send + 'a>> {
        let action = action.clone();
        let scripts = Arc::clone(&self.running_scripts);
        let native_action = Arc::clone(&self.native_action);
        Box::pin(async move {
            match action {
                ConfiguredAction::Media { command } => {
                    let permit = native_action
                        .try_acquire_owned()
                        .map_err(|_| ExecutorError::Busy)?;
                    tokio::task::spawn_blocking(move || {
                        let _permit = permit;
                        execute_media(command)
                    })
                    .await
                    .map_err(|_| ExecutorError::Failed)?
                }
                ConfiguredAction::Launch {
                    program,
                    args,
                    working_dir,
                } => {
                    let mut process = base_command(&program, &args, working_dir.as_deref());
                    process.spawn().map_err(|_| ExecutorError::Failed)?;
                    Ok(())
                }
                action @ (ConfiguredAction::Batch { .. } | ConfiguredAction::PowerShell { .. }) => {
                    let permit = scripts
                        .try_acquire_owned()
                        .map_err(|_| ExecutorError::Busy)?;
                    let command = script_command(&action)?;
                    let mut wrapped = CommandWrap::from(command);
                    wrapped.wrap(JobObject).wrap(KillOnDrop);
                    let mut child = wrapped.spawn().map_err(|_| ExecutorError::Failed)?;
                    tokio::spawn(async move {
                        match tokio::time::timeout(SCRIPT_TIMEOUT, child.wait()).await {
                            Ok(Ok(status)) if status.success() => {
                                tracing::debug!("configured script completed");
                            }
                            Ok(Ok(_)) => {
                                tracing::warn!("configured script returned a failure status")
                            }
                            Ok(Err(_)) => {
                                tracing::warn!("configured script could not be monitored")
                            }
                            Err(_) => {
                                tracing::warn!(
                                    "configured script exceeded its time limit; terminating process tree"
                                );
                                if Pin::from(child.kill()).await.is_err() {
                                    tracing::error!(
                                        "configured script process-tree cleanup failed"
                                    );
                                }
                            }
                        }
                        drop(permit);
                    });
                    Ok(())
                }
            }
        })
    }
}

fn base_command(
    program: &Path,
    args: &[String],
    working_dir: Option<&Path>,
) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(working_dir) = working_dir {
        command.current_dir(working_dir);
    }
    use std::os::windows::process::CommandExt;
    command.as_std_mut().creation_flags(CREATE_NO_WINDOW.0);
    command
}

fn script_command(action: &ConfiguredAction) -> Result<tokio::process::Command, ExecutorError> {
    match action {
        ConfiguredAction::Batch {
            script,
            args,
            working_dir,
        } => {
            let script = script.to_str().ok_or(ExecutorError::Failed)?;
            let script = cmd_literal(script)?;
            let arguments = args
                .iter()
                .map(|argument| cmd_literal(argument))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(|argument| format!(" \"{argument}\""))
                .collect::<String>();
            let command_line = format!("call \"{script}\"{arguments}");
            let mut command = tokio::process::Command::new("cmd.exe");
            command.args(["/D", "/V:OFF", "/S", "/C"]);
            use std::os::windows::process::CommandExt;
            // Preserve cmd.exe's /C quoting rules instead of applying CRT argument escaping.
            command.as_std_mut().raw_arg(format!(" \"{command_line}\""));
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            if let Some(working_dir) = working_dir {
                command.current_dir(working_dir);
            }
            command.as_std_mut().creation_flags(CREATE_NO_WINDOW.0);
            Ok(command)
        }
        ConfiguredAction::PowerShell {
            interpreter,
            script,
            args,
            working_dir,
        } => {
            if !script.is_absolute()
                || !interpreter.is_absolute()
                || !script
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("ps1"))
            {
                return Err(ExecutorError::Failed);
            }
            let mut command = base_command(interpreter, &[], working_dir.as_deref());
            command
                .args(["-NoProfile", "-NonInteractive", "-File"])
                .arg(script)
                .args(args);
            Ok(command)
        }
        _ => Err(ExecutorError::Failed),
    }
}

/// cmd.exe expands `%` and treats quotes as syntax; reject both instead of interpreting user data.
fn cmd_literal(value: &str) -> Result<&str, ExecutorError> {
    if domain::is_safe_batch_literal(value) {
        Ok(value)
    } else {
        Err(ExecutorError::Failed)
    }
}

fn execute_media(command: MediaCommand) -> Result<(), ExecutorError> {
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(|_| ExecutorError::Failed)?;
    let _apartment = ComApartment;
    match command {
        MediaCommand::Mute | MediaCommand::Unmute => {
            set_default_output_mute(command == MediaCommand::Mute)
        }
        _ => send_media_key(media_key_for_command(command).ok_or(ExecutorError::Failed)?),
    }
}

fn media_key_for_command(
    command: MediaCommand,
) -> Option<windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY> {
    match command {
        MediaCommand::Play | MediaCommand::Pause => Some(VK_MEDIA_PLAY_PAUSE),
        MediaCommand::Next => Some(VK_MEDIA_NEXT_TRACK),
        MediaCommand::Previous => Some(VK_MEDIA_PREV_TRACK),
        MediaCommand::Mute | MediaCommand::Unmute => None,
    }
}

fn send_media_key(
    key: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY,
) -> Result<(), ExecutorError> {
    let input = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                dwFlags: flags,
                ..Default::default()
            },
        },
    };
    let inputs = [input(KEYBD_EVENT_FLAGS(0)), input(KEYEVENTF_KEYUP)];
    if unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) } == inputs.len() as u32 {
        Ok(())
    } else {
        Err(ExecutorError::Failed)
    }
}

#[cfg(test)]
fn wait_media_operation<T: windows::core::RuntimeType + 'static>(
    operation: IAsyncOperation<T>,
) -> Result<T, ExecutorError> {
    let deadline = std::time::Instant::now() + MEDIA_OPERATION_TIMEOUT;
    loop {
        match operation.Status().map_err(|_| ExecutorError::Failed)? {
            AsyncStatus::Completed => {
                return operation.GetResults().map_err(|_| ExecutorError::Failed);
            }
            AsyncStatus::Canceled | AsyncStatus::Error => return Err(ExecutorError::Failed),
            AsyncStatus::Started if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            AsyncStatus::Started => {
                let _ = operation.Cancel();
                return Err(ExecutorError::Failed);
            }
            _ => return Err(ExecutorError::Failed),
        }
    }
}

struct ComApartment;
impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe { RoUninitialize() };
    }
}

fn set_default_output_mute(muted: bool) -> Result<(), ExecutorError> {
    let enumerator: windows::Win32::Media::Audio::IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
            .map_err(|_| ExecutorError::Failed)?;
    let device = unsafe { enumerator.GetDefaultAudioEndpoint(E_RENDER, E_MULTIMEDIA) }
        .map_err(|_| ExecutorError::Failed)?;
    let endpoint: IAudioEndpointVolume =
        unsafe { device.Activate(CLSCTX_ALL, None) }.map_err(|_| ExecutorError::Failed)?;
    let event_context = windows::core::GUID::zeroed();
    unsafe { endpoint.SetMute(muted, &event_context) }.map_err(|_| ExecutorError::Failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    async fn finish(command: tokio::process::Command) {
        let mut wrapped = CommandWrap::from(command);
        wrapped.wrap(JobObject).wrap(KillOnDrop);
        let mut child = wrapped.spawn().unwrap();
        let status = tokio::time::timeout(Duration::from_secs(10), child.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn cached_media_factory_survives_worker_apartment_teardown() {
        let _mta = MtaLifetime::new().unwrap();
        for _ in 0..8 {
            std::thread::spawn(|| {
                unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.unwrap();
                let _apartment = ComApartment;
                let request = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
                    .expect("media-session factory activation must succeed");
                // Completion may fail on a headless host; only cached factory reuse is under test.
                let _ = wait_media_operation(request);
            })
            .join()
            .unwrap();
        }
    }

    #[test]
    fn playback_commands_use_global_media_keys() {
        assert_eq!(
            media_key_for_command(MediaCommand::Play),
            media_key_for_command(MediaCommand::Pause)
        );
        assert_eq!(
            media_key_for_command(MediaCommand::Next),
            Some(VK_MEDIA_NEXT_TRACK)
        );
        assert_eq!(
            media_key_for_command(MediaCommand::Previous),
            Some(VK_MEDIA_PREV_TRACK)
        );
        assert_eq!(media_key_for_command(MediaCommand::Mute), None);
    }

    #[test]
    fn batch_rejects_shell_syntax_before_spawning() {
        let action = ConfiguredAction::Batch {
            script: Path::new(r"C:\Actions\safe.cmd").to_path_buf(),
            args: vec!["x&echo injected".into()],
            working_dir: None,
        };
        assert!(script_command(&action).is_err());
    }

    #[tokio::test]
    async fn batch_preserves_quoted_arguments_in_a_path_with_spaces() {
        let temp = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let directory = temp.path().join("with spaces");
        fs::create_dir(&directory).unwrap();
        let script = directory.join("fixture.cmd");
        assert!(
            domain::is_safe_batch_literal(script.to_str().unwrap()),
            "fixture path is not safe for cmd.exe: {script:?}"
        );
        let output = directory.join("args.txt");
        let script_text = format!(
            "@echo off\r\n> \"{}\" echo %~1\r\n>> \"{}\" echo %~2\r\n",
            output.display(),
            output.display()
        );
        fs::write(&script, script_text).unwrap();
        let action = ConfiguredAction::Batch {
            script,
            args: vec!["first arg".into(), "(second)".into()],
            working_dir: Some(directory.clone()),
        };
        finish(script_command(&action).unwrap()).await;
        assert_eq!(
            fs::read_to_string(output).unwrap(),
            "first arg\r\n(second)\r\n"
        );
    }

    #[tokio::test]
    async fn powershell_passes_arguments_without_shell_interpretation() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("with spaces");
        fs::create_dir(&directory).unwrap();
        let script = directory.join("fixture.ps1");
        let output = directory.join("args.txt");
        fs::write(
            &script,
            "[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'args.txt'), $args[0] + '|' + $args[1], [Text.UTF8Encoding]::new($false))",
        )
        .unwrap();
        let interpreter = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap()
            .join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
        let action = ConfiguredAction::PowerShell {
            interpreter,
            script,
            args: vec!["left & right".into(), "%PATH%".into()],
            working_dir: Some(directory),
        };
        finish(script_command(&action).unwrap()).await;
        assert_eq!(fs::read_to_string(output).unwrap(), "left & right|%PATH%");
    }
}
