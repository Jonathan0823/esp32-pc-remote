use crate::{
    application::{ActionExecutor, ExecutorError},
    domain::{self, ConfiguredAction, MediaCommand},
};
#[cfg(test)]
use process_wrap::tokio::KillOnDrop;
use process_wrap::tokio::{CommandWrap, JobObject};
use std::{
    ffi::OsStr, future::Future, os::windows::ffi::OsStrExt, path::Path, pin::Pin, process::Stdio,
    sync::Arc, time::Duration,
};
use tokio::sync::Semaphore;
#[cfg(test)]
use windows::Media::Control::GlobalSystemMediaTransportControlsSessionManager;
use windows::Win32::{
    Foundation::{CloseHandle, ERROR_CANCELLED},
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
    UI::{
        Input::KeyboardAndMouse::{
            INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
            SendInput, VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK,
        },
        Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW},
        WindowsAndMessaging::SW_SHOWNORMAL,
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
                    run_as_admin,
                } => {
                    if run_as_admin {
                        tokio::task::spawn_blocking(move || {
                            launch_elevated(&program, &args, working_dir.as_deref())
                        })
                        .await
                        .map_err(|_| ExecutorError::Failed)?
                    } else {
                        let mut process = base_command(&program, &args, working_dir.as_deref());
                        process.spawn().map_err(|_| ExecutorError::Failed)?;
                        Ok(())
                    }
                }
                action @ (ConfiguredAction::Batch { .. } | ConfiguredAction::PowerShell { .. }) => {
                    let permit = scripts
                        .try_acquire_owned()
                        .map_err(|_| ExecutorError::Busy)?;
                    let command = script_command(&action)?;
                    let mut child = script_process(command)
                        .spawn()
                        .map_err(|_| ExecutorError::Failed)?;
                    tokio::spawn(async move {
                        match tokio::time::timeout(SCRIPT_TIMEOUT, child.wait()).await {
                            Ok(Ok(status)) if status.success() => {
                                tracing::debug!("configured script completed");
                            }
                            Ok(Ok(_)) => {
                                tracing::warn!("configured script returned a failure status")
                            }
                            Ok(Err(_)) => {
                                tracing::warn!("configured script could not be monitored");
                                if Pin::from(child.kill()).await.is_err() {
                                    tracing::error!(
                                        "configured script process-tree cleanup failed"
                                    );
                                }
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

fn launch_elevated(
    program: &Path,
    args: &[String],
    working_dir: Option<&Path>,
) -> Result<(), ExecutorError> {
    if program
        .extension()
        .and_then(OsStr::to_str)
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("exe"))
        || args.iter().any(|argument| argument.contains('\0'))
    {
        return Err(ExecutorError::Failed);
    }
    let file = wide_os_string(program.as_os_str())?;
    let parameters = if args.is_empty() {
        None
    } else {
        let value = args
            .iter()
            .map(|argument| quote_windows_argument(argument))
            .collect::<Vec<_>>()
            .join(" ");
        Some(wide_string(&value))
    };
    let directory = working_dir
        .map(|path| wide_os_string(path.as_os_str()))
        .transpose()?;
    let verb = wide_string("runas");
    let mut execution = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: windows::core::PCWSTR::from_raw(verb.as_ptr()),
        lpFile: windows::core::PCWSTR::from_raw(file.as_ptr()),
        lpParameters: parameters
            .as_ref()
            .map_or(windows::core::PCWSTR::null(), |value| {
                windows::core::PCWSTR::from_raw(value.as_ptr())
            }),
        lpDirectory: directory
            .as_ref()
            .map_or(windows::core::PCWSTR::null(), |value| {
                windows::core::PCWSTR::from_raw(value.as_ptr())
            }),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    if let Err(error) = unsafe { ShellExecuteExW(&mut execution) } {
        return Err(map_runas_error(error));
    }
    if !execution.hProcess.is_invalid() {
        let _ = unsafe { CloseHandle(execution.hProcess) };
    }
    Ok(())
}

fn map_runas_error(error: windows::core::Error) -> ExecutorError {
    if error.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
        ExecutorError::Cancelled
    } else {
        ExecutorError::Failed
    }
}

fn wide_os_string(value: &OsStr) -> Result<Vec<u16>, ExecutorError> {
    let mut wide: Vec<_> = value.encode_wide().collect();
    if wide.contains(&0) {
        return Err(ExecutorError::Failed);
    }
    wide.push(0);
    Ok(wide)
}

fn wide_string(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn quote_windows_argument(argument: &str) -> String {
    let mut quoted = String::from("\"");
    let mut backslashes = 0;
    for character in argument.chars() {
        match character {
            '\\' => backslashes += 1,
            '"' => {
                quoted.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.extend(std::iter::repeat_n('\\', backslashes));
                quoted.push(character);
                backslashes = 0;
            }
        }
    }
    quoted.extend(std::iter::repeat_n('\\', backslashes * 2));
    quoted.push('"');
    quoted
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

fn script_process(command: tokio::process::Command) -> CommandWrap {
    let mut wrapped = CommandWrap::from(command);
    wrapped.wrap(JobObject);
    wrapped
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
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                ])
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

    #[test]
    fn elevated_argument_quoting_preserves_windows_argument_boundaries() {
        assert_eq!(quote_windows_argument(""), "\"\"");
        assert_eq!(quote_windows_argument("two words"), "\"two words\"");
        assert_eq!(quote_windows_argument("a\"b"), "\"a\\\"b\"");
        assert_eq!(quote_windows_argument(r#"a\"b"#), r#""a\\\"b""#);
        assert_eq!(quote_windows_argument("trailing\\"), "\"trailing\\\\\"");
    }

    #[test]
    fn only_windows_uac_cancellation_is_classified_as_cancelled() {
        assert_eq!(
            map_runas_error(windows::core::Error::from_hresult(
                windows::core::HRESULT::from_win32(ERROR_CANCELLED.0),
            )),
            ExecutorError::Cancelled
        );
        assert_eq!(
            map_runas_error(windows::core::Error::from_hresult(
                windows::Win32::Foundation::E_ACCESSDENIED,
            )),
            ExecutorError::Failed
        );
    }

    async fn finish(command: tokio::process::Command) {
        let mut wrapped = script_process(command);
        wrapped.wrap(KillOnDrop);
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

    #[tokio::test]
    async fn normal_script_completion_does_not_kill_surviving_descendants() {
        let temp = tempfile::tempdir().unwrap();
        let parent_script = temp.path().join("parent.ps1");
        let child_script = temp.path().join("child.ps1");
        let marker = temp.path().join("child-finished.txt");
        fs::write(
            &child_script,
            "Start-Sleep -Milliseconds 750; [IO.File]::WriteAllText($args[0], 'done')",
        )
        .unwrap();
        let quote = |path: &Path| path.to_string_lossy().replace('\'', "''");
        fs::write(
            &parent_script,
            format!(
                "Start-Process -FilePath (Join-Path $PSHOME 'powershell.exe') -ArgumentList '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"{}\" \"{}\"' -WindowStyle Hidden",
                quote(&child_script),
                quote(&marker),
            ),
        )
        .unwrap();
        let interpreter = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap()
            .join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
        let action = ConfiguredAction::PowerShell {
            interpreter,
            script: parent_script,
            args: vec![],
            working_dir: Some(temp.path().to_path_buf()),
        };
        let mut child = script_process(script_command(&action).unwrap())
            .spawn()
            .unwrap();
        let status = tokio::time::timeout(Duration::from_secs(10), child.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(status.success());
        drop(child);
        tokio::time::timeout(Duration::from_secs(5), async {
            while !marker.exists() {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(fs::read_to_string(marker).unwrap(), "done");
    }
}
