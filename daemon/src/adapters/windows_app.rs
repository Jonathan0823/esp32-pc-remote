use std::{
    net::IpAddr,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use eframe::egui;
use tokio::runtime::Handle;
use tray_icon::{
    MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};
use winit::event_loop::EventLoop;

use super::{
    windows_executor::MtaLifetime,
    windows_file_picker::{self, FileKind},
    windows_host, windows_icon,
    windows_management::{SettingsClient, SettingsSupervisor},
    windows_runtime::{WindowsRuntime, acquire_single_instance},
};
use crate::domain::{ActionId, ConfiguredAction, CustomAction};

enum UiEvent {
    ListenerState(bool),
    ConfigState(bool),
    ActionTestResult(bool),
    ActionsSaved(bool),
    VoiceConfigApplied(bool),
    StartupState(bool),
    CredentialsRotated(bool),
    PairingToken(Result<String, String>),
    CaExported(Result<PathBuf, String>),
    SnapshotUpdated,
    Disconnected,
}

struct UiSignals {
    context: Mutex<Option<egui::Context>>,
    sender: mpsc::Sender<UiEvent>,
    receiver: Mutex<mpsc::Receiver<UiEvent>>,
}

impl UiSignals {
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            context: Mutex::new(None),
            sender,
            receiver: Mutex::new(receiver),
        }
    }

    fn next_update(&self) -> Option<UiEvent> {
        self.receiver.lock().ok()?.try_recv().ok()
    }

    fn wake(&self, event: UiEvent) {
        let _ = self.sender.send(event);
        if let Ok(context) = self.context.lock()
            && let Some(context) = context.as_ref()
        {
            context.request_repaint();
        }
    }
}

#[derive(Clone, Copy)]
enum DaemonEvent {
    OpenSettings,
    ToggleListener,
    Quit,
}

struct DaemonSignals {
    sender: mpsc::Sender<DaemonEvent>,
    thread_id: u32,
}

impl DaemonSignals {
    fn send(&self, event: DaemonEvent) {
        let _ = self.sender.send(event);
        windows_host::wake_thread(self.thread_id);
    }

    fn menu_event(&self, id: &str) {
        match id {
            "settings" => self.send(DaemonEvent::OpenSettings),
            "listener" => self.send(DaemonEvent::ToggleListener),
            "quit" => self.send(DaemonEvent::Quit),
            _ => {}
        }
    }

    fn tray_event(&self, event: TrayIconEvent) {
        if matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
        ) {
            self.send(DaemonEvent::OpenSettings);
        }
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let Some(_instance) = acquire_single_instance()? else {
        return Ok(());
    };
    // Outlive the runtime so cached WinRT factories survive worker apartment teardown.
    let _mta = MtaLifetime::new()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let service = WindowsRuntime::load()?;
    let startup_error =
        service.config().listener_enabled && runtime.block_on(service.set_listener(true)).is_err();
    if startup_error {
        tracing::warn!("saved listener could not be started; it remains paused");
    }
    let _entered = runtime.enter();
    let _host = windows_host::MessageHost::new()?;
    let (sender, receiver) = mpsc::channel();
    let signals = Arc::new(DaemonSignals {
        sender,
        thread_id: windows_host::current_thread_id(),
    });
    let _tray = create_tray(Arc::clone(&signals)).map_err(|error| {
        windows_host::show_error(&format!("Could not create the system tray: {error}"));
        std::io::Error::other(error)
    })?;
    let supervisor = SettingsSupervisor::start(
        Arc::clone(&service),
        runtime.handle().clone(),
        startup_error,
    )?;
    let mut quitting = false;
    loop {
        if windows_host::take_open_request() {
            supervisor.open();
        }
        while let Ok(event) = receiver.try_recv() {
            match event {
                DaemonEvent::OpenSettings => supervisor.open(),
                DaemonEvent::ToggleListener => {
                    let service = Arc::clone(&service);
                    runtime.spawn(async move {
                        let enabled = !service.config().listener_enabled;
                        let _ = service.set_listener(enabled).await;
                    });
                }
                DaemonEvent::Quit => quitting = true,
            }
        }
        if quitting || !windows_host::dispatch_message()? {
            break;
        }
    }
    supervisor.quit();
    runtime.block_on(service.shutdown())?;
    Ok(())
}

pub fn run_settings_mode() -> Result<(), Box<dyn std::error::Error>> {
    let service = SettingsClient::connect().map_err(std::io::Error::other)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let signals = Arc::new(UiSignals::new());
    let mut event_loop = EventLoop::with_user_event().build()?;
    let startup_error = service.startup_error();
    let mut settings = open_settings(
        signals,
        service,
        runtime.handle().clone(),
        startup_error,
        &event_loop,
    );
    let mut result = Ok(());
    loop {
        if let eframe::EframePumpStatus::Exit(code) =
            settings.pump_eframe_app(&mut event_loop, None)
        {
            if code != 0 {
                result =
                    Err(std::io::Error::other(format!("Settings exited with code {code}")).into());
            }
            break;
        }
    }
    drop(settings);
    runtime.shutdown_timeout(Duration::from_millis(250));
    result
}

fn open_settings(
    signals: Arc<UiSignals>,
    service: Arc<SettingsClient>,
    handle: Handle,
    startup_error: bool,
    event_loop: &EventLoop<eframe::UserEvent>,
) -> eframe::EframeWinitApplication<'static> {
    let options = eframe::NativeOptions {
        run_and_return: true,
        viewport: egui::ViewportBuilder::default()
            .with_title("ESP32 Media Daemon")
            .with_icon(windows_icon::settings_icon())
            .with_inner_size([760.0, 680.0])
            .with_min_inner_size([620.0, 480.0]),
        ..Default::default()
    };
    eframe::create_native(
        "ESP32 Media Daemon",
        options,
        Box::new(move |creation| {
            Ok(Box::new(SettingsApp::new(
                creation,
                signals,
                service,
                handle,
                startup_error,
            )))
        }),
        event_loop,
    )
}

fn create_tray(signals: Arc<DaemonSignals>) -> Result<tray_icon::TrayIcon, String> {
    let menu = Menu::new();
    let open = MenuItem::with_id("settings", "Settings", true, None);
    let toggle = MenuItem::with_id("listener", "Pause / resume listener", true, None);
    let quit = MenuItem::with_id("quit", "Quit", true, None);
    menu.append(&open).map_err(|error| error.to_string())?;
    menu.append(&toggle).map_err(|error| error.to_string())?;
    menu.append(&quit).map_err(|error| error.to_string())?;
    let menu_signals = Arc::clone(&signals);
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        menu_signals.menu_event(event.id.0.as_str());
    }));
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        signals.tray_event(event);
    }));

    let icon = windows_icon::tray_icon()?;
    TrayIconBuilder::new()
        .with_tooltip("ESP32 Media Daemon")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .map_err(|error| error.to_string())
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum SettingsTab {
    #[default]
    Overview,
    Actions,
    Connection,
}

struct SettingsApp {
    tab: SettingsTab,
    logo: egui::TextureHandle,
    signals: Arc<UiSignals>,
    service: Arc<SettingsClient>,
    handle: Handle,
    listener_enabled: bool,
    listener_pending: bool,
    actions_pending: bool,
    startup_pending: bool,
    status: Option<String>,
    bind_address: String,
    port: String,
    pairing_token: Option<String>,
    start_after_login: bool,
    action_id: String,
    action_name: String,
    action_path: String,
    interpreter_path: String,
    action_args_json: String,
    action_phrase: String,
    phrase_drafts: std::collections::HashMap<String, String>,
    voice_apply_pending: bool,
    action_kind: usize,
    confirm_action: Option<String>,
    confirm_rotation: bool,
    refresh_pending: bool,
    last_refresh: Instant,
}

impl SettingsApp {
    fn new(
        creation: &eframe::CreationContext<'_>,
        signals: Arc<UiSignals>,
        service: Arc<SettingsClient>,
        handle: Handle,
        startup_error: bool,
    ) -> Self {
        if let Ok(mut context_slot) = signals.context.lock() {
            *context_slot = Some(creation.egui_ctx.clone());
        }
        let config = service.config();
        let mut style = (*creation.egui_ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        creation.egui_ctx.set_style(style);
        let icon = windows_icon::settings_icon();
        let logo = creation.egui_ctx.load_texture(
            "daemon-logo",
            egui::ColorImage::from_rgba_unmultiplied(
                [icon.width as usize, icon.height as usize],
                &icon.rgba,
            ),
            egui::TextureOptions::LINEAR,
        );
        Self {
            tab: SettingsTab::default(),
            logo,
            signals,
            service,
            handle,
            listener_enabled: config.listener_enabled,
            listener_pending: false,
            actions_pending: false,
            startup_pending: false,
            status: startup_error
                .then(|| "Saved listener could not start; check its address and port".into()),
            bind_address: config.bind_address.to_string(),
            port: config.port.to_string(),
            pairing_token: None,
            start_after_login: config.start_after_login,
            action_id: String::new(),
            action_name: String::new(),
            action_path: String::new(),
            interpreter_path: String::new(),
            action_args_json: "[]".into(),
            action_phrase: String::new(),
            phrase_drafts: config
                .actions
                .iter()
                .map(|action| {
                    (
                        action.id.as_str().to_owned(),
                        action.phrase.clone().unwrap_or_default(),
                    )
                })
                .collect(),
            voice_apply_pending: false,
            action_kind: 0,
            confirm_action: None,
            confirm_rotation: false,
            refresh_pending: false,
            last_refresh: Instant::now(),
        }
    }

    fn request_listener(&mut self, enabled: bool) {
        if self.listener_pending {
            return;
        }
        self.listener_pending = true;
        self.listener_enabled = enabled;
        let service = Arc::clone(&self.service);
        let signals = Arc::clone(&self.signals);
        self.handle.spawn(async move {
            let result = service.set_listener(enabled).await;
            let actual = if result.is_ok() {
                enabled
            } else {
                service.config().listener_enabled
            };
            signals.wake(UiEvent::ListenerState(actual));
        });
    }

    fn save_actions(&mut self, actions: Vec<CustomAction>) {
        if self.actions_pending {
            return;
        }
        self.actions_pending = true;
        let service = Arc::clone(&self.service);
        let signals = Arc::clone(&self.signals);
        self.handle.spawn(async move {
            signals.wake(UiEvent::ActionsSaved(
                service.save_actions(actions).await.is_ok(),
            ));
        });
    }

    fn apply_voice_config(&mut self) {
        if self.actions_pending || self.voice_apply_pending {
            return;
        }
        self.voice_apply_pending = true;
        let service = Arc::clone(&self.service);
        let signals = Arc::clone(&self.signals);
        self.handle.spawn(async move {
            signals.wake(UiEvent::VoiceConfigApplied(
                service.apply_voice_config().await.is_ok(),
            ));
        });
    }

    fn save_phrases(&mut self) {
        if self.actions_pending {
            return;
        }
        let mut actions = self.service.config().actions;
        for action in &mut actions {
            action.phrase = self
                .phrase_drafts
                .get(action.id.as_str())
                .map(|phrase| phrase.trim().to_owned())
                .filter(|phrase| !phrase.is_empty());
        }
        let mut candidate = self.service.config();
        candidate.actions = actions.clone();
        if candidate.validate().is_err() {
            self.status = Some("Invalid, duplicate, or reserved voice phrase".into());
            return;
        }
        self.save_actions(actions);
    }

    fn set_startup(&mut self, enabled: bool) {
        if self.startup_pending {
            return;
        }
        self.startup_pending = true;
        let service = Arc::clone(&self.service);
        let signals = Arc::clone(&self.signals);
        self.handle.spawn(async move {
            signals.wake(UiEvent::StartupState(
                service.set_start_after_login(enabled).await.is_ok(),
            ));
        });
    }

    fn add_action(&mut self) {
        let Ok(id) = ActionId::parse(self.action_id.trim()) else {
            self.status = Some("Action ID must match [a-z][a-z0-9_]{0,63}".into());
            return;
        };
        let Ok(args) = serde_json::from_str::<Vec<String>>(&self.action_args_json) else {
            self.status = Some("Arguments must be a JSON string array".into());
            return;
        };
        let path = PathBuf::from(&self.action_path);
        let action = match self.action_kind {
            0 => ConfiguredAction::Launch {
                program: path,
                args,
                working_dir: None,
            },
            1 => ConfiguredAction::Batch {
                script: path,
                args,
                working_dir: None,
            },
            _ => ConfiguredAction::PowerShell {
                interpreter: PathBuf::from(&self.interpreter_path),
                script: path,
                args,
                working_dir: None,
            },
        };
        let mut actions = self.service.config().actions;
        actions.push(CustomAction {
            id,
            name: self.action_name.trim().to_owned(),
            enabled: true,
            phrase: match self.action_phrase.trim() {
                "" => None,
                phrase => Some(phrase.to_owned()),
            },
            action,
        });
        let mut candidate = self.service.config();
        candidate.actions = actions.clone();
        if candidate.validate().is_err() {
            self.status = Some("Invalid action".into());
            return;
        }
        self.action_id.clear();
        self.action_name.clear();
        self.action_phrase.clear();
        self.action_path.clear();
        self.save_actions(actions);
    }

    fn overview(&mut self, ui: &mut egui::Ui) {
        ui.heading("Ready for your commands");
        ui.label(
            "This agent runs in the tray. Closing Settings keeps the HTTPS service available.",
        );
        ui.add_space(6.0);
        egui::Frame::group(ui.style())
            .inner_margin(14)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.strong("HTTPS listener");
                let changed = ui
                    .add_enabled(
                        !self.listener_pending,
                        egui::Checkbox::new(&mut self.listener_enabled, "Accept HTTPS commands"),
                    )
                    .changed();
                if changed {
                    self.request_listener(self.listener_enabled);
                }
                let config = self.service.config();
                ui.monospace(format!("https://{}:{}", config.bind_address, config.port));
                ui.weak("Paused means the port is closed. Enable it before testing in RESTerm.");
            });
        ui.add_space(6.0);
        egui::Frame::group(ui.style())
            .inner_margin(14)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.strong("Windows startup");
                if ui
                    .add_enabled(
                        !self.startup_pending,
                        egui::Checkbox::new(&mut self.start_after_login, "Start after login"),
                    )
                    .changed()
                {
                    self.set_startup(self.start_after_login);
                }
                ui.weak("Runs as your Windows user, without administrator privileges.");
            });
        ui.add_space(6.0);
        egui::CollapsingHeader::new("Recent activity")
            .default_open(true)
            .show(ui, |ui| {
                let events = self.service.diagnostics();
                if events.is_empty() {
                    ui.weak("No commands received yet.");
                }
                for event in events.iter().rev().take(10) {
                    ui.group(|ui| {
                        ui.strong(&event.category);
                        if let Some(id) = &event.request_id {
                            ui.small(format!("Request: {id}"));
                        }
                        if let Some(id) = &event.action_id {
                            ui.small(format!("Action: {id}"));
                        }
                    });
                }
            });
    }

    fn actions(&mut self, ui: &mut egui::Ui) {
        ui.heading("Configured actions");
        ui.label("Choose a file locally, then trigger it remotely using only its action ID.");
        let config = self.service.config();
        let actions = config.actions.clone();
        let phrases_dirty = actions.iter().any(|action| {
            self.phrase_drafts
                .get(action.id.as_str())
                .map(String::as_str)
                .unwrap_or("")
                != action.phrase.as_deref().unwrap_or("")
        });
        if actions.is_empty() {
            ui.weak("No custom actions yet. Add an application or script below.");
        }
        for action in actions {
            ui.push_id(action.id.as_str(), |ui| {
                egui::Frame::group(ui.style())
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.strong(&action.name);
                        ui.monospace(action.id.as_str());
                        let description = match &action.action {
                            ConfiguredAction::Media { command } => {
                                format!("Media: {}", command.as_str())
                            }
                            ConfiguredAction::Launch { program, .. } => {
                                format!("Application: {}", program.display())
                            }
                            ConfiguredAction::Batch { script, .. } => {
                                format!("Batch: {}", script.display())
                            }
                            ConfiguredAction::PowerShell { script, .. } => {
                                format!("PowerShell: {}", script.display())
                            }
                        };
                        ui.add(egui::Label::new(description).wrap());
                        ui.horizontal(|ui| {
                            ui.label("Voice phrase");
                            let phrase = self
                                .phrase_drafts
                                .entry(action.id.as_str().to_owned())
                                .or_default();
                            ui.add_enabled(
                                !self.actions_pending,
                                egui::TextEdit::singleline(phrase).hint_text("open Spotify"),
                            );
                        });
                        ui.horizontal(|ui| {
                            let mut enabled = action.enabled;
                            if ui
                                .add_enabled(
                                    !self.actions_pending,
                                    egui::Checkbox::new(&mut enabled, "Enabled"),
                                )
                                .changed()
                            {
                                let mut updated = self.service.config().actions;
                                if let Some(item) =
                                    updated.iter_mut().find(|item| item.id == action.id)
                                {
                                    item.enabled = enabled;
                                }
                                self.save_actions(updated);
                            }
                            if ui
                                .add_enabled(
                                    action.enabled && !self.actions_pending,
                                    egui::Button::new("Test…"),
                                )
                                .clicked()
                            {
                                self.confirm_action = Some(action.id.as_str().to_string());
                            }
                            if ui
                                .add_enabled(!self.actions_pending, egui::Button::new("Remove"))
                                .clicked()
                            {
                                let mut remaining = self.service.config().actions;
                                remaining.retain(|existing| existing.id != action.id);
                                self.save_actions(remaining);
                            }
                        });
                    });
            });
        }
        let voice_needs_apply = config
            .published_voice
            .as_ref()
            .is_none_or(|published| published.actions_revision != config.actions_revision);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    phrases_dirty && !self.actions_pending,
                    egui::Button::new("Save phrases"),
                )
                .clicked()
            {
                self.save_phrases();
            }
            if ui
                .add_enabled(
                    voice_needs_apply
                        && !phrases_dirty
                        && !self.actions_pending
                        && !self.voice_apply_pending,
                    egui::Button::new("Apply to ESP"),
                )
                .clicked()
            {
                self.apply_voice_config();
            }
        });
        match config.published_voice.as_ref() {
            Some(published) if published.actions_revision != config.actions_revision => ui.weak("Saved action/phrase changes are not applied. Save phrases if needed, then Apply to ESP."),
            Some(published) if config.acknowledged_voice_revision == Some(published.revision) => ui.weak(format!("ESP last confirmed voice settings revision {}.", published.revision)),
            Some(published) => ui.weak(format!("Voice settings revision {} is pending ESP sync/confirmation.", published.revision)),
            None => ui.weak("Voice settings have not been applied to the ESP."),
        };
        ui.weak("After Apply, say Jarvis, then sync. Existing built-in phrases cannot be edited.");
        ui.separator();
        ui.strong("Built-in voice commands (read-only)");
        ui.label("Jarvis → sync (fetch settings); wake (Wake-on-LAN); play; pause; next; back; mute; unmute.");
        ui.add_space(8.0);
        egui::CollapsingHeader::new("Add an action")
            .default_open(true)
            .show(ui, |ui| {
                ui.add_enabled_ui(!self.actions_pending, |ui| {
                    egui::Grid::new("action_identity")
                        .num_columns(2)
                        .spacing([12.0, 10.0])
                        .show(ui, |ui| {
                            ui.label("Action ID").on_hover_text(
                                "1–64 characters: lowercase letter first, then lowercase letters, digits, or underscores.",
                            );
                            ui.add(
                                egui::TextEdit::singleline(&mut self.action_id)
                                    .hint_text("open_spotify")
                                    .desired_width(360.0),
                            );
                            ui.end_row();
                            ui.label("Display name");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.action_name)
                                    .hint_text("Open Spotify")
                                    .desired_width(360.0),
                            );
                            ui.end_row();
                            ui.label("Voice phrase (optional)");
                            ui.add(egui::TextEdit::singleline(&mut self.action_phrase).hint_text("open Spotify").desired_width(360.0));
                            ui.end_row();
                        });
                    ui.horizontal(|ui| {
                        ui.label("Run type");
                        ui.selectable_value(&mut self.action_kind, 0, "Application");
                        ui.selectable_value(&mut self.action_kind, 1, "Batch");
                        ui.selectable_value(&mut self.action_kind, 2, "PowerShell");
                    });
                    let (label, kind) = match self.action_kind {
                        0 => ("Application (.exe)", FileKind::Executable),
                        1 => ("Batch script (.bat / .cmd)", FileKind::Batch),
                        _ => ("PowerShell script (.ps1)", FileKind::PowerShell),
                    };
                    ui.label(label);
                    if let Some(error) = file_path_field(ui, &mut self.action_path, kind) {
                        self.status = Some(error);
                    }
                    if self.action_kind == 2 {
                        ui.label("PowerShell interpreter (.exe)");
                        if let Some(error) =
                            file_path_field(ui, &mut self.interpreter_path, FileKind::Executable)
                        {
                            self.status = Some(error);
                        }
                        ui.weak("Choose Windows PowerShell (powershell.exe) or PowerShell 7 (pwsh.exe).");
                    }
                    ui.label("Arguments (optional JSON array)");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.action_args_json)
                            .desired_width(f32::INFINITY),
                    );
                    ui.weak("Leave [] for no arguments. Example: [\"--profile\", \"work\"]");
                    if ui.button("Add action").clicked() {
                        self.add_action();
                    }
                    ui.weak("Selecting or adding a file does not run it. Test requires confirmation.");
                });
            });
    }

    fn connection(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        ui.heading("Connection & security");
        egui::Frame::group(ui.style())
            .inner_margin(14)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.strong("Listener address");
                ui.weak("Pause the listener in Overview before changing its address.");
                ui.label("For ESP32 access, enter this PC's private LAN IPv4 (for example, 192.168.1.50).");
                ui.weak("127.0.0.1 is PC-only. Use a specific LAN IP, not 0.0.0.0; restrict the firewall to your ESP32 on the Private network profile.");
                ui.add_enabled_ui(!self.listener_enabled && !self.listener_pending, |ui| {
                    egui::Grid::new("listener_address")
                        .num_columns(2)
                        .spacing([12.0, 10.0])
                        .show(ui, |ui| {
                            ui.label("Bind IP");
                            ui.text_edit_singleline(&mut self.bind_address);
                            ui.end_row();
                            ui.label("Port");
                            ui.add(egui::TextEdit::singleline(&mut self.port).desired_width(90.0));
                            ui.end_row();
                        });
                    if ui.button("Apply address").clicked() {
                        match (
                            self.bind_address.parse::<IpAddr>(),
                            self.port.parse::<u16>(),
                        ) {
                            (Ok(address), Ok(port)) => {
                                let service = Arc::clone(&self.service);
                                let signals = Arc::clone(&self.signals);
                                self.handle.spawn(async move {
                                    let success = service.reconfigure(address, port).await.is_ok();
                                    signals.wake(UiEvent::ConfigState(success));
                                });
                            }
                            _ => self.status = Some("Enter a valid IP address and port".into()),
                        }
                    }
                });
                ui.weak("Changing the bind IP regenerates the CA. Pair clients again afterwards.");
            });
        ui.add_space(6.0);
        egui::Frame::group(ui.style()).inner_margin(14).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.strong("Client pairing");
            ui.label("The pairing token authorizes every enabled action. Keep it private.");
            ui.weak("ESP32 config.h: use the LAN IP as PC_MEDIA_HOST, the exported CA PEM as PC_MEDIA_CA_CERT, and this token as PC_MEDIA_API_TOKEN. Rebuild and flash after provisioning.");
            ui.weak("The ESP32 needs the public CA, not a client certificate or a TLS private key.");
            ui.horizontal(|ui| {
                if ui.button("Show pairing token").clicked() {
                    let service = Arc::clone(&self.service);
                    let signals = Arc::clone(&self.signals);
                    self.handle.spawn(async move {
                        signals.wake(UiEvent::PairingToken(service.token_for_pairing().await));
                    });
                }
                if ui.button("Export trusted CA PEM").clicked() {
                    let service = Arc::clone(&self.service);
                    let signals = Arc::clone(&self.signals);
                    self.handle.spawn(async move {
                        signals.wake(UiEvent::CaExported(service.export_ca().await));
                    });
                }
            });
            if let Some(token) = &self.pairing_token {
                ui.add(egui::Label::new(egui::RichText::new(token).monospace()).wrap());
                if ui.button("Copy token").clicked() {
                    context.copy_text(token.clone());
                }
            }
            ui.separator();
            ui.weak("Only rotate credentials to revoke paired clients. Existing clients will stop working.");
            if ui.button("Rotate token and TLS trust…").clicked() {
                self.confirm_rotation = true;
            }
        });
    }
}

fn file_path_field(ui: &mut egui::Ui, path: &mut String, kind: FileKind) -> Option<String> {
    let mut error = None;
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 105.0).max(120.0);
        ui.add(
            egui::TextEdit::singleline(path)
                .desired_width(width)
                .hint_text("Browse or enter an absolute path"),
        );
        if ui.button("Browse…").clicked() {
            match windows_file_picker::pick_file(kind) {
                Ok(Some(selected)) => *path = selected,
                Ok(None) => {}
                Err(reason) => error = Some(format!("Could not select a file: {reason}")),
            }
        }
    });
    error
}

impl Drop for SettingsApp {
    fn drop(&mut self) {
        if let Ok(mut context) = self.signals.context.lock() {
            *context = None;
        }
        while self.signals.next_update().is_some() {}
    }
}

impl eframe::App for SettingsApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        context.request_repaint_after(Duration::from_secs(2));
        if !self.refresh_pending && self.last_refresh.elapsed() >= Duration::from_secs(2) {
            self.refresh_pending = true;
            self.last_refresh = Instant::now();
            let service = Arc::clone(&self.service);
            let signals = Arc::clone(&self.signals);
            self.handle.spawn(async move {
                signals.wake(if service.refresh().await.is_ok() {
                    UiEvent::SnapshotUpdated
                } else {
                    UiEvent::Disconnected
                });
            });
        }
        while let Some(event) = self.signals.next_update() {
            match event {
                UiEvent::ListenerState(enabled) => {
                    self.listener_pending = false;
                    self.listener_enabled = enabled;
                    self.status = Some(
                        if enabled {
                            "Listener enabled"
                        } else {
                            "Listener paused"
                        }
                        .into(),
                    );
                }
                UiEvent::ConfigState(success) => {
                    self.status = Some(
                        if success {
                            "Configuration applied"
                        } else {
                            "Configuration was not applied"
                        }
                        .into(),
                    );
                }
                UiEvent::CredentialsRotated(success) => {
                    self.pairing_token = None;
                    self.listener_enabled = self.service.config().listener_enabled;
                    self.status = Some(
                        if success {
                            "Token and TLS trust rotated; pair clients again"
                        } else {
                            "Credential rotation failed"
                        }
                        .into(),
                    );
                }
                UiEvent::ActionsSaved(success) => {
                    self.actions_pending = false;
                    if success {
                        self.phrase_drafts = self
                            .service
                            .config()
                            .actions
                            .iter()
                            .map(|action| {
                                (
                                    action.id.as_str().to_owned(),
                                    action.phrase.clone().unwrap_or_default(),
                                )
                            })
                            .collect();
                    }
                    self.status = Some(
                        if success {
                            "Action configuration saved; Apply to publish voice changes"
                        } else {
                            "Invalid action or configuration save failed"
                        }
                        .into(),
                    );
                }
                UiEvent::VoiceConfigApplied(success) => {
                    self.voice_apply_pending = false;
                    self.status = Some(
                        if success {
                            "Voice settings published; say Jarvis, then sync on the ESP32"
                        } else {
                            "Could not publish voice settings"
                        }
                        .into(),
                    );
                }
                UiEvent::StartupState(success) => {
                    self.startup_pending = false;
                    self.start_after_login = self.service.config().start_after_login;
                    self.status = Some(
                        if success {
                            "Startup preference saved"
                        } else {
                            "Could not update startup preference"
                        }
                        .into(),
                    );
                }
                UiEvent::ActionTestResult(success) => {
                    self.status = Some(
                        if success {
                            "Test was dispatched"
                        } else {
                            "Test failed"
                        }
                        .into(),
                    );
                }
                UiEvent::PairingToken(result) => {
                    self.pairing_token = result.as_ref().ok().cloned();
                    self.status = Some(if result.is_ok() {
                        "Pairing token revealed".into()
                    } else {
                        "Could not retrieve the pairing token".into()
                    });
                }
                UiEvent::CaExported(result) => {
                    self.status = Some(match result {
                        Ok(path) => format!("CA exported to {}", path.display()),
                        Err(_) => "Could not export the trusted CA".into(),
                    });
                }
                UiEvent::SnapshotUpdated => {
                    self.refresh_pending = false;
                    let config = self.service.config();
                    if !self.listener_pending {
                        self.listener_enabled = config.listener_enabled;
                    }
                    if !self.startup_pending {
                        self.start_after_login = config.start_after_login;
                    }
                }
                UiEvent::Disconnected => {
                    self.status = Some("Background process disconnected; closing Settings".into());
                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        egui::TopBottomPanel::top("settings_header").show(context, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.image((self.logo.id(), egui::vec2(46.0, 46.0)));
                ui.vertical(|ui| {
                    ui.heading("ESP32 Media Daemon");
                    ui.weak("Local actions. Secure voice control.");
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (label, color) = if self.listener_pending {
                        ("Updating…", ui.visuals().weak_text_color())
                    } else if self.listener_enabled {
                        ("Listener enabled", ui.visuals().text_color())
                    } else {
                        ("Listener paused", ui.visuals().weak_text_color())
                    };
                    ui.colored_label(color, label);
                });
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, SettingsTab::Overview, "Overview");
                ui.selectable_value(&mut self.tab, SettingsTab::Actions, "Actions");
                ui.selectable_value(
                    &mut self.tab,
                    SettingsTab::Connection,
                    "Connection & security",
                );
            });
        });
        egui::TopBottomPanel::bottom("settings_footer").show(context, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Close Settings").clicked() {
                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                ui.add(
                    egui::Label::new(
                        self.status
                            .as_deref()
                            .unwrap_or("Settings closes; your tray agent keeps running."),
                    )
                    .wrap(),
                );
            });
        });
        egui::CentralPanel::default().show(context, |ui| {
            egui::ScrollArea::vertical()
                .id_salt(self.tab as u8)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    match self.tab {
                        SettingsTab::Overview => self.overview(ui),
                        SettingsTab::Actions => self.actions(ui),
                        SettingsTab::Connection => self.connection(ui, context),
                    }
                });
        });
        if let Some(action_id) = self.confirm_action.clone() {
            egui::Window::new("Confirm action test")
                .collapsible(false)
                .show(context, |ui| {
                    ui.label("This runs the configured action with your Windows user permissions.");
                    ui.horizontal(|ui| {
                        if ui.button("Run test").clicked() {
                            self.confirm_action = None;
                            if let Ok(id) = ActionId::parse(action_id) {
                                let service = Arc::clone(&self.service);
                                let signals = Arc::clone(&self.signals);
                                self.handle.spawn(async move {
                                    signals.wake(UiEvent::ActionTestResult(
                                        service.test_action(&id).await.is_ok(),
                                    ));
                                });
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.confirm_action = None;
                        }
                    });
                });
        }
        if self.confirm_rotation {
            egui::Window::new("Confirm credential rotation")
                    .collapsible(false)
                    .show(context, |ui| {
                        ui.label("If running, the listener briefly restarts. Rotation invalidates the current token and trusted CA; existing clients must be paired again.");
                        ui.horizontal(|ui| {
                            if ui.button("Rotate credentials").clicked() {
                                self.confirm_rotation = false;
                                let service = Arc::clone(&self.service);
                                let signals = Arc::clone(&self.signals);
                                self.handle.spawn(async move {
                                    signals.wake(UiEvent::CredentialsRotated(service.rotate_credentials().await.is_ok()));
                                });
                            }
                            if ui.button("Cancel").clicked() {
                                self.confirm_rotation = false;
                            }
                        });
                    });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendering_a_file_field_does_not_change_or_run_its_path() {
        let context = egui::Context::default();
        let mut path = String::from(r"C:\Apps\Music Player\player.exe");
        let original = path.clone();
        let _ = context.run(egui::RawInput::default(), |context| {
            egui::CentralPanel::default().show(context, |ui| {
                assert!(file_path_field(ui, &mut path, FileKind::Executable).is_none());
            });
        });
        assert_eq!(path, original);
    }

    #[test]
    fn settings_updates_are_queued_without_a_background_ui_context() {
        let signals = UiSignals::new();
        signals.wake(UiEvent::ListenerState(true));
        assert!(matches!(
            signals.next_update(),
            Some(UiEvent::ListenerState(true))
        ));
        assert!(signals.next_update().is_none());
        assert!(signals.context.lock().unwrap().is_none());
    }
}
