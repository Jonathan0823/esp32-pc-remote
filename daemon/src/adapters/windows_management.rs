use crate::{
    config::AppConfig,
    domain::{ActionId, CustomAction},
    management::{
        self, DaemonSnapshot, DiagnosticSnapshot, Envelope, ManagementError, Request, Response,
    },
};
use std::{
    process::{Child, Command, Stdio},
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};
use tokio::runtime::Handle;

const RPC_TIMEOUT: Duration = Duration::from_secs(30);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

struct Call {
    request: Envelope<Request>,
    reply: SyncSender<Result<Envelope<Response>, String>>,
}

pub struct SettingsClient {
    state: RwLock<DaemonSnapshot>,
    calls: SyncSender<Call>,
    next_id: AtomicU64,
}

impl SettingsClient {
    pub fn connect() -> Result<Arc<Self>, String> {
        use std::io::IsTerminal;
        if std::io::stdin().is_terminal() || std::io::stdout().is_terminal() {
            return Err("Settings can only connect to its background process".into());
        }
        let (calls, receiver) = mpsc::sync_channel(management::MAX_PENDING_REQUESTS);
        thread::Builder::new()
            .name("daemon-settings-ipc".into())
            .spawn(move || client_worker(receiver))
            .map_err(|_| "Could not start the local management channel".to_owned())?;
        let client = Arc::new(Self {
            state: RwLock::new(DaemonSnapshot {
                config: AppConfig::default(),
                diagnostics: Vec::new(),
                uploaded_diagnostics: Vec::new(),
                startup_error: false,
            }),
            calls,
            next_id: AtomicU64::new(1),
        });
        let response = client.call_blocking(Request::Snapshot, STARTUP_TIMEOUT)?;
        let Response::Snapshot { state } = response else {
            return Err("Background process did not send initial Settings state".into());
        };
        client.replace_state(state);
        Ok(client)
    }

    pub fn config(&self) -> AppConfig {
        self.state
            .read()
            .map(|state| state.config.clone())
            .unwrap_or_default()
    }

    pub fn diagnostics(&self) -> Vec<DiagnosticSnapshot> {
        self.state
            .read()
            .map(|state| state.diagnostics.clone())
            .unwrap_or_default()
    }

    pub fn uploaded_diagnostics(&self) -> Vec<crate::diagnostics::Event> {
        self.state
            .read()
            .map(|state| state.uploaded_diagnostics.clone())
            .unwrap_or_default()
    }

    pub fn startup_error(&self) -> bool {
        self.state.read().is_ok_and(|state| state.startup_error)
    }

    pub async fn refresh(&self) -> Result<(), String> {
        match self.call(Request::Snapshot).await {
            Ok(Response::Snapshot { .. }) => Ok(()),
            Err(error) if error == "Settings request queue is full" => Ok(()),
            Ok(_) => Err("Background process returned an unexpected response".into()),
            Err(error) => Err(error),
        }
    }

    pub async fn set_listener(&self, enabled: bool) -> Result<(), String> {
        self.applied(Request::SetListener { enabled }).await
    }

    pub async fn save_actions(&self, actions: Vec<CustomAction>) -> Result<(), String> {
        self.applied(Request::SaveActions { actions }).await
    }

    pub async fn apply_voice_config(&self) -> Result<(), String> {
        self.applied(Request::ApplyVoiceConfig).await
    }

    pub async fn test_action(&self, action_id: &ActionId) -> Result<(), ManagementError> {
        action_test_result(
            self.call_raw(Request::TestAction {
                action_id: action_id.clone(),
            })
            .await,
        )
    }

    pub async fn set_start_after_login(&self, enabled: bool) -> Result<(), String> {
        self.applied(Request::SetStartup { enabled }).await
    }

    pub async fn reconfigure(
        &self,
        bind_address: std::net::IpAddr,
        port: u16,
    ) -> Result<(), String> {
        self.applied(Request::Reconfigure { bind_address, port })
            .await
    }

    pub async fn token_for_pairing(&self) -> Result<String, String> {
        match self.call(Request::RevealPairingToken).await? {
            Response::PairingToken { token } => Ok(token),
            _ => Err("Background process returned an unexpected response".into()),
        }
    }

    pub async fn export_ca(&self) -> Result<std::path::PathBuf, String> {
        match self.call(Request::ExportCa).await? {
            Response::CaExported { path } => Ok(path),
            _ => Err("Background process returned an unexpected response".into()),
        }
    }

    pub async fn rotate_credentials(&self) -> Result<(), String> {
        self.applied(Request::RotateCredentials).await
    }

    async fn applied(&self, request: Request) -> Result<(), String> {
        match self.call(request).await? {
            Response::Applied { .. } => Ok(()),
            _ => Err("Background process rejected the Settings operation".into()),
        }
    }

    async fn call(&self, request: Request) -> Result<Response, String> {
        match self.call_raw(request).await? {
            Response::Error { code } => Err(error_message(code).into()),
            payload => Ok(payload),
        }
    }

    async fn call_raw(&self, request: Request) -> Result<Response, String> {
        let calls = self.calls.clone();
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let envelope = Envelope::new(id, request);
        let response =
            tokio::task::spawn_blocking(move || call_worker(&calls, envelope, RPC_TIMEOUT))
                .await
                .map_err(|_| "Local management request failed".to_owned())??;
        if response.version != management::PROTOCOL_VERSION || response.id != id {
            return Err("Background process returned a mismatched response".into());
        }
        match response.payload {
            payload @ (Response::Snapshot { .. }
            | Response::Applied { .. }
            | Response::ActionCancelled { .. }) => {
                let state = match &payload {
                    Response::Snapshot { state }
                    | Response::Applied { state }
                    | Response::ActionCancelled { state } => state.clone(),
                    _ => unreachable!(),
                };
                self.replace_state(state);
                Ok(payload)
            }
            payload => Ok(payload),
        }
    }

    fn call_blocking(&self, request: Request, timeout: Duration) -> Result<Response, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let response = call_worker(&self.calls, Envelope::new(id, request), timeout)?;
        if response.version != management::PROTOCOL_VERSION || response.id != id {
            return Err("Background process returned a mismatched response".into());
        }
        match response.payload {
            Response::Snapshot { state } => {
                self.replace_state(state.clone());
                Ok(Response::Snapshot { state })
            }
            Response::Error { code } => Err(error_message(code).into()),
            payload => Ok(payload),
        }
    }

    fn replace_state(&self, state: DaemonSnapshot) {
        if let Ok(mut current) = self.state.write() {
            *current = state;
        }
    }
}

fn action_test_result(response: Result<Response, String>) -> Result<(), ManagementError> {
    match response {
        Ok(Response::Applied { .. }) => Ok(()),
        Ok(Response::ActionCancelled { .. }) => Err(ManagementError::Cancelled),
        Ok(Response::Error { code }) => Err(code),
        _ => Err(ManagementError::Failed),
    }
}

fn call_worker(
    calls: &SyncSender<Call>,
    request: Envelope<Request>,
    timeout: Duration,
) -> Result<Envelope<Response>, String> {
    let (reply, receiver) = mpsc::sync_channel(1);
    calls
        .try_send(Call { request, reply })
        .map_err(|error| match error {
            mpsc::TrySendError::Full(_) => "Settings request queue is full".to_owned(),
            mpsc::TrySendError::Disconnected(_) => "Background process is disconnected".to_owned(),
        })?;
    receiver
        .recv_timeout(timeout)
        .map_err(|_| "Background process did not respond".to_owned())?
}

fn client_worker(receiver: Receiver<Call>) {
    let mut input = std::io::stdin();
    let mut output = std::io::stdout();
    while let Ok(call) = receiver.recv() {
        let result = management::write_message(&mut output, &call.request)
            .and_then(|()| {
                management::read_message::<Envelope<Response>>(&mut input).and_then(|response| {
                    response.ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "background disconnected",
                        )
                    })
                })
            })
            .map_err(|_| "Background process is disconnected".to_owned());
        let disconnected = result.is_err();
        let _ = call.reply.send(result);
        if disconnected {
            break;
        }
    }
}

pub struct SettingsSupervisor {
    commands: mpsc::Sender<SupervisorCommand>,
}

enum SupervisorCommand {
    Open,
    Quit(SyncSender<()>),
    ChildDisconnected(u32),
}

impl SettingsSupervisor {
    pub fn start(
        service: Arc<super::windows_runtime::WindowsRuntime>,
        handle: Handle,
        startup_error: bool,
    ) -> std::io::Result<Self> {
        let (commands, receiver) = mpsc::channel();
        let actor_commands = commands.clone();
        let startup_error = Arc::new(AtomicBool::new(startup_error));
        thread::Builder::new()
            .name("daemon-settings-supervisor".into())
            .spawn(move || {
                supervisor_loop(receiver, actor_commands, service, handle, startup_error)
            })?;
        Ok(Self { commands })
    }

    pub fn open(&self) {
        let _ = self.commands.send(SupervisorCommand::Open);
    }

    pub fn quit(&self) {
        let (reply, receiver) = mpsc::sync_channel(1);
        if self.commands.send(SupervisorCommand::Quit(reply)).is_ok() {
            let _ = receiver.recv_timeout(SHUTDOWN_TIMEOUT + Duration::from_secs(1));
        }
    }
}

fn supervisor_loop(
    commands: Receiver<SupervisorCommand>,
    command_sender: mpsc::Sender<SupervisorCommand>,
    service: Arc<super::windows_runtime::WindowsRuntime>,
    handle: Handle,
    startup_error: Arc<AtomicBool>,
) {
    let mut child: Option<Child> = None;
    let mut _job: Option<SettingsJob> = None;
    loop {
        let command = commands.recv().ok();
        match command {
            Some(SupervisorCommand::Open) => {
                if child
                    .as_mut()
                    .is_some_and(|child| child.try_wait().ok().flatten().is_none())
                {
                    if let Some(process) = child.as_ref() {
                        super::windows_host::reveal_settings(process.id());
                    }
                    continue;
                }
                if let Some(mut exited) = child.take() {
                    let _ = exited.wait();
                    _job = None;
                }
                match spawn_settings(
                    &service,
                    handle.clone(),
                    command_sender.clone(),
                    Arc::clone(&startup_error),
                ) {
                    Ok((process, process_job)) => {
                        child = Some(process);
                        _job = Some(process_job);
                    }
                    Err(_) => super::windows_host::show_error(
                        "Could not open Settings. The background service is still running.",
                    ),
                }
            }
            Some(SupervisorCommand::Quit(reply)) => {
                if let Some(process) = child.as_mut() {
                    super::windows_host::close_settings(process.id());
                    let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
                    loop {
                        if process.try_wait().ok().flatten().is_some() {
                            break;
                        }
                        if Instant::now() >= deadline {
                            let _ = process.kill();
                            let _ = process.wait();
                            break;
                        }
                        thread::sleep(Duration::from_millis(25));
                    }
                }
                _job = None;
                let _ = reply.send(());
                break;
            }
            Some(SupervisorCommand::ChildDisconnected(process_id)) => {
                if child
                    .as_ref()
                    .is_some_and(|process| process.id() == process_id)
                {
                    if let Some(mut process) = child.take() {
                        if process.try_wait().ok().flatten().is_none() {
                            let _ = process.kill();
                        }
                        let _ = process.wait();
                    }
                    _job = None;
                }
            }
            None => {
                if let Some(mut process) = child.take() {
                    let _ = process.kill();
                    let _ = process.wait();
                }
                break;
            }
        }
        if let Some(process) = child.as_mut()
            && process.try_wait().ok().flatten().is_some()
        {
            child = None;
            _job = None;
        }
    }
}

fn spawn_settings(
    service: &Arc<super::windows_runtime::WindowsRuntime>,
    handle: Handle,
    events: mpsc::Sender<SupervisorCommand>,
    startup_error: Arc<AtomicBool>,
) -> Result<(Child, SettingsJob), String> {
    let executable = std::env::current_exe().map_err(|_| "current executable unavailable")?;
    let mut command = Command::new(executable);
    command
        .arg("--settings")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    use std::os::windows::process::CommandExt;
    command.creation_flags(windows::Win32::System::Threading::CREATE_NO_WINDOW.0);
    let mut child = command
        .spawn()
        .map_err(|_| "Settings process could not be started")?;
    let job = match SettingsJob::assign(&child) {
        Ok(job) => job,
        Err(()) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Settings lifetime could not be secured".into());
        }
    };
    let process_id = child.id();
    let input = child
        .stdin
        .take()
        .ok_or("Settings input pipe unavailable")?;
    let output = child
        .stdout
        .take()
        .ok_or("Settings output pipe unavailable")?;
    let service = Arc::clone(service);
    thread::Builder::new()
        .name("daemon-settings-channel".into())
        .spawn(move || {
            let mut reader = std::io::BufReader::new(output);
            let mut writer = std::io::BufWriter::new(input);
            while let Ok(Some(request)) = management::read_message::<Envelope<Request>>(&mut reader)
            {
                let response = if request.version != management::PROTOCOL_VERSION {
                    Response::Error {
                        code: ManagementError::InvalidRequest,
                    }
                } else {
                    dispatch_request(request.payload, &service, &handle, &startup_error)
                };
                if management::write_message(&mut writer, &Envelope::new(request.id, response))
                    .is_err()
                {
                    break;
                }
            }
            let _ = events.send(SupervisorCommand::ChildDisconnected(process_id));
        })
        .map_err(|_| "Settings communication could not be started")?;
    Ok((child, job))
}

fn dispatch_request(
    request: Request,
    service: &super::windows_runtime::WindowsRuntime,
    handle: &Handle,
    startup_error: &AtomicBool,
) -> Response {
    let clears_startup_error = matches!(
        &request,
        Request::SetListener { .. } | Request::Reconfigure { .. } | Request::RotateCredentials
    );
    if let Request::TestAction { action_id } = &request {
        return match handle.block_on(service.test_action(action_id)) {
            Ok(()) => Response::Applied {
                state: snapshot(service, startup_error.load(Ordering::Relaxed)),
            },
            Err(crate::application::DispatchError::ExecutionCancelled) => {
                Response::ActionCancelled {
                    state: snapshot(service, startup_error.load(Ordering::Relaxed)),
                }
            }
            Err(
                crate::application::DispatchError::Busy
                | crate::application::DispatchError::ExecutionBusy,
            ) => Response::Error {
                code: ManagementError::Busy,
            },
            Err(_) => Response::Error {
                code: ManagementError::Failed,
            },
        };
    }
    let result = match request {
        Request::Snapshot => {
            return Response::Snapshot {
                state: snapshot(service, startup_error.load(Ordering::Relaxed)),
            };
        }
        Request::SetListener { enabled } => handle.block_on(service.set_listener(enabled)),
        Request::SaveActions { actions } => handle.block_on(service.save_actions(actions)),
        Request::ApplyVoiceConfig => handle.block_on(service.apply_voice_config()),
        Request::TestAction { .. } => unreachable!("TestAction handled above"),
        Request::SetStartup { enabled } => handle.block_on(service.set_start_after_login(enabled)),
        Request::Reconfigure { bind_address, port } => {
            handle.block_on(service.reconfigure(bind_address, port))
        }
        Request::RevealPairingToken => {
            return match service.token_for_pairing() {
                Ok(token) => Response::PairingToken { token },
                Err(_) => Response::Error {
                    code: ManagementError::Failed,
                },
            };
        }
        Request::ExportCa => {
            return match service.export_ca() {
                Ok(path) => Response::CaExported { path },
                Err(_) => Response::Error {
                    code: ManagementError::Failed,
                },
            };
        }
        Request::RotateCredentials => handle.block_on(service.rotate_credentials()),
    };
    match result {
        Ok(()) => {
            if clears_startup_error {
                startup_error.store(false, Ordering::Relaxed);
            }
            Response::Applied {
                state: snapshot(service, startup_error.load(Ordering::Relaxed)),
            }
        }
        Err(_) => Response::Error {
            code: ManagementError::Failed,
        },
    }
}

fn snapshot(
    service: &super::windows_runtime::WindowsRuntime,
    startup_error: bool,
) -> DaemonSnapshot {
    DaemonSnapshot {
        config: service.config(),
        diagnostics: service
            .diagnostics()
            .into_iter()
            .map(|event| DiagnosticSnapshot {
                request_id: event.request_id,
                action_id: event.action_id,
                category: event.category.into(),
                reason: event.reason,
            })
            .collect(),
        uploaded_diagnostics: service.uploaded_diagnostics(),
        startup_error,
    }
}

struct SettingsJob(windows::Win32::Foundation::HANDLE);

impl SettingsJob {
    fn assign(child: &Child) -> Result<Self, ()> {
        use windows::Win32::{
            Foundation::HANDLE,
            System::JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject,
            },
        };
        let job =
            unsafe { CreateJobObjectW(None, windows::core::PCWSTR::null()) }.map_err(|_| ())?;
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast::<core::ffi::c_void>(),
                std::mem::size_of_val(&info) as u32,
            )
            .and_then(|()| AssignProcessToJobObject(job, HANDLE(child.as_raw_handle() as _)))
        }
        .is_err()
        {
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(job);
            }
            return Err(());
        }
        Ok(Self(job))
    }
}

impl Drop for SettingsJob {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

use std::os::windows::io::AsRawHandle;

fn error_message(error: ManagementError) -> &'static str {
    match error {
        ManagementError::InvalidRequest => "Background process rejected the Settings request",
        ManagementError::Busy => "Background process is busy",
        ManagementError::Cancelled => "UAC prompt was cancelled",
        ManagementError::Failed => "Background process could not complete the operation",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_test_preserves_management_error_codes() {
        for code in [ManagementError::Busy, ManagementError::Failed] {
            assert_eq!(action_test_result(Ok(Response::Error { code })), Err(code));
        }
    }

    #[test]
    fn closing_the_settings_job_terminates_only_its_owned_child() {
        use std::os::windows::process::CommandExt;

        let mut child = Command::new("cmd.exe")
            .args(["/D", "/C", "ping -n 20 127.0.0.1 >NUL"])
            .creation_flags(windows::Win32::System::Threading::CREATE_NO_WINDOW.0)
            .spawn()
            .expect("Windows test command must start");
        let job = SettingsJob::assign(&child).expect("job object must own the test child");
        drop(job);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if child.try_wait().expect("query child status").is_some() {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "job close did not terminate its child"
            );
            thread::sleep(Duration::from_millis(25));
        }
    }

    #[test]
    fn a_full_request_queue_rejects_without_retrying_or_replacing_work() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let (reply, _response) = mpsc::sync_channel(1);
        assert!(
            sender
                .send(Call {
                    request: Envelope::new(1, Request::Snapshot),
                    reply,
                })
                .is_ok()
        );

        let error = call_worker(
            &sender,
            Envelope::new(2, Request::RotateCredentials),
            RPC_TIMEOUT,
        )
        .err()
        .expect("a full queue must reject the new operation");
        assert!(error.contains("queue is full"));
        assert_eq!(receiver.try_recv().unwrap().request.id, 1);
        assert!(receiver.try_recv().is_err());
    }
}
